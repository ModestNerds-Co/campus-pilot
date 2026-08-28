//! Implements tenant-scoped provider connection persistence and lifecycle writes.
//!
//! Every mutation and its actor-aware audit event share one transaction. Network
//! calls run outside database transactions and persist only safe outcome fields.
//!
//! Copyright (c) 2026 Codecraft Solutions. All rights reserved.

use std::str::FromStr;

use chrono::{DateTime, Utc};
use cp_audit::{
    AuditActor, AuditActorKind, AuditOutcome, AuditTarget, NewAuditEvent, RequestContext,
};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use sqlx::{FromRow, PgPool, Postgres, QueryBuilder};
use uuid::Uuid;

use crate::{
    client::ProviderHttpClient,
    crypto::{CredentialContext, CredentialKeyring, EncryptedCredential},
    types::{
        AiProviderConnection, AuthMethod, ConnectionModelSnapshot, ConnectionStatus,
        ConnectionTestOutcome, ConnectionTestResult, CreateConnectionCommand,
        DisconnectedConnection, ProviderKey, ProviderModel, RotateCredentialCommand, ServiceError,
        UpdateConnectionCommand, positive_version,
    },
};

const CREDENTIAL_DOMAIN: &str = "campus-pilot/ai-provider-credential";

const CONNECTION_PROJECTION: &str = r#"
    SELECT
        c.id,
        c.provider,
        CASE c.provider
            WHEN 'openai' THEN 'OpenAI'
            WHEN 'anthropic' THEN 'Anthropic'
            WHEN 'openrouter' THEN 'OpenRouter'
        END AS provider_label,
        c.auth_method,
        c.account_label,
        c.status,
        c.credential_fingerprint,
        c.credential_version,
        c.version,
        u.full_name AS configured_by_name,
        c.last_tested_at,
        c.last_test_status,
        c.last_failure_category,
        c.last_used_at,
        (
            SELECT COUNT(*)
            FROM ai_provider_models m
            WHERE m.tenant_id = c.tenant_id
              AND m.connection_id = c.id
              AND m.credential_version = c.credential_version
              AND m.catalog_version = c.model_catalog_version
              AND m.deleted_at IS NULL
        ) AS model_count,
        c.model_catalog_refreshed_at,
        c.created_at,
        c.updated_at
    FROM ai_provider_connections c
    JOIN users u ON u.id = c.configured_by AND u.tenant_id = c.tenant_id
"#;

/// Shared service used by HTTP routes and secret-free Agent read handlers.
#[derive(Debug, Clone)]
pub struct AiProviderOps {
    pool: PgPool,
    keyring: Option<CredentialKeyring>,
    provider_client: Option<ProviderHttpClient>,
}

impl AiProviderOps {
    #[must_use]
    pub fn new(
        pool: PgPool,
        keyring: Option<CredentialKeyring>,
        provider_client: ProviderHttpClient,
    ) -> Self {
        Self {
            pool,
            keyring,
            provider_client: Some(provider_client),
        }
    }

    /// Builds the same service boundary for secret-free catalogue and status reads.
    #[must_use]
    pub fn for_reads(pool: PgPool) -> Self {
        Self {
            pool,
            keyring: None,
            provider_client: None,
        }
    }

    /// Lists current connections without selecting credential storage columns.
    pub async fn list_connections(
        &self,
        tenant_id: Uuid,
    ) -> Result<Vec<AiProviderConnection>, ServiceError> {
        let query = format!(
            "{CONNECTION_PROJECTION} WHERE c.tenant_id = $1 AND c.deleted_at IS NULL ORDER BY c.provider, LOWER(c.account_label), c.id"
        );
        sqlx::query_as::<_, AiProviderConnection>(&query)
            .bind(tenant_id)
            .fetch_all(&self.pool)
            .await
            .map_err(ServiceError::from)
    }

    /// Reads one current connection without selecting credential storage columns.
    pub async fn read_connection(
        &self,
        tenant_id: Uuid,
        connection_id: Uuid,
    ) -> Result<AiProviderConnection, ServiceError> {
        fetch_connection(&self.pool, tenant_id, connection_id).await
    }

    /// Encrypts and creates a new app-managed provider connection.
    pub async fn create_connection(
        &self,
        tenant_id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        command: CreateConnectionCommand,
    ) -> Result<AiProviderConnection, ServiceError> {
        let configured_by = person_actor_id(actor)?;
        let connection_id = Uuid::new_v4();
        let credential_version = 1_i64;
        let encrypted = self.encrypt(
            tenant_id,
            connection_id,
            command.provider,
            command.auth_method,
            credential_version,
            &command.api_key,
        )?;
        let fingerprint = credential_fingerprint(command.provider, &command.api_key);

        let mut transaction = self.pool.begin().await?;
        let insert = sqlx::query(
            r#"
            INSERT INTO ai_provider_connections (
                id, tenant_id, provider, auth_method, account_label, status,
                credential_ciphertext, credential_nonce, credential_key_id,
                credential_envelope_version, credential_version,
                credential_fingerprint, configured_by
            )
            VALUES ($1, $2, $3, $4, $5, 'untested', $6, $7, $8, $9, $10, $11, $12)
            "#,
        )
        .bind(connection_id)
        .bind(tenant_id)
        .bind(command.provider.as_str())
        .bind(command.auth_method.as_str())
        .bind(command.account_label.as_str())
        .bind(encrypted.ciphertext)
        .bind(encrypted.nonce)
        .bind(encrypted.key_id)
        .bind(encrypted.envelope_version)
        .bind(credential_version)
        .bind(fingerprint)
        .bind(configured_by)
        .execute(&mut *transaction)
        .await;
        if let Err(error) = insert {
            return Err(map_write_error(error));
        }

        append_audit(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "administration.ai_providers.connections.create",
            connection_id,
            AuditOutcome::Succeeded,
            metadata([
                (
                    "provider",
                    Value::String(command.provider.as_str().to_owned()),
                ),
                (
                    "auth_method",
                    Value::String(command.auth_method.as_str().to_owned()),
                ),
                ("credential_version", Value::from(credential_version)),
            ]),
        )
        .await?;
        transaction.commit().await?;
        self.read_connection(tenant_id, connection_id).await
    }

    /// Renames a connection using optimistic concurrency.
    pub async fn update_connection(
        &self,
        tenant_id: Uuid,
        connection_id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        command: UpdateConnectionCommand,
    ) -> Result<AiProviderConnection, ServiceError> {
        let mut transaction = self.pool.begin().await?;
        let result = sqlx::query(
            r#"
            UPDATE ai_provider_connections
            SET account_label = $1, version = version + 1, updated_at = NOW()
            WHERE tenant_id = $2 AND id = $3 AND version = $4 AND deleted_at IS NULL
            "#,
        )
        .bind(command.account_label.as_str())
        .bind(tenant_id)
        .bind(connection_id)
        .bind(command.expected_version)
        .execute(&mut *transaction)
        .await;
        let result = match result {
            Ok(result) => result,
            Err(error) => return Err(map_write_error(error)),
        };
        require_updated(result.rows_affected(), &self.pool, tenant_id, connection_id).await?;
        append_audit(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "administration.ai_providers.connections.update",
            connection_id,
            AuditOutcome::Succeeded,
            Map::new(),
        )
        .await?;
        transaction.commit().await?;
        self.read_connection(tenant_id, connection_id).await
    }

    /// Rotates a credential and invalidates old test state and model snapshots.
    pub async fn rotate_credential(
        &self,
        tenant_id: Uuid,
        connection_id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        command: RotateCredentialCommand,
    ) -> Result<AiProviderConnection, ServiceError> {
        person_actor_id(actor)?;
        let existing = load_credential_row(&self.pool, tenant_id, connection_id).await?;
        if existing.version != command.expected_version {
            return Err(stale_version());
        }
        let provider = ProviderKey::from_str(&existing.provider)?;
        let auth_method = AuthMethod::from_str(&existing.auth_method)?;
        let credential_version = existing
            .credential_version
            .checked_add(1)
            .ok_or_else(stale_version)?;
        let encrypted = self.encrypt(
            tenant_id,
            connection_id,
            provider,
            auth_method,
            credential_version,
            &command.api_key,
        )?;
        let fingerprint = credential_fingerprint(provider, &command.api_key);

        let mut transaction = self.pool.begin().await?;
        let result = sqlx::query(
            r#"
            UPDATE ai_provider_connections
            SET credential_ciphertext = $1,
                credential_nonce = $2,
                credential_key_id = $3,
                credential_envelope_version = $4,
                credential_version = $5,
                credential_fingerprint = $6,
                status = 'untested',
                last_tested_at = NULL,
                last_test_status = NULL,
                last_failure_category = NULL,
                model_catalog_refreshed_at = NULL,
                version = version + 1,
                updated_at = NOW()
            WHERE tenant_id = $7 AND id = $8 AND version = $9 AND deleted_at IS NULL
            "#,
        )
        .bind(encrypted.ciphertext)
        .bind(encrypted.nonce)
        .bind(encrypted.key_id)
        .bind(encrypted.envelope_version)
        .bind(credential_version)
        .bind(fingerprint)
        .bind(tenant_id)
        .bind(connection_id)
        .bind(command.expected_version)
        .execute(&mut *transaction)
        .await;
        let result = match result {
            Ok(result) => result,
            Err(error) => return Err(map_write_error(error)),
        };
        require_updated(result.rows_affected(), &self.pool, tenant_id, connection_id).await?;
        append_audit(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "administration.ai_providers.credentials.rotate",
            connection_id,
            AuditOutcome::Succeeded,
            metadata([("credential_version", Value::from(credential_version))]),
        )
        .await?;
        transaction.commit().await?;
        self.read_connection(tenant_id, connection_id).await
    }

    /// Tests the current credential and records only a normalized outcome.
    pub async fn test_connection(
        &self,
        tenant_id: Uuid,
        connection_id: Uuid,
        expected_version: i64,
        actor: AuditActor,
        request_context: RequestContext,
    ) -> Result<ConnectionTestResult, ServiceError> {
        let expected_version = positive_version(expected_version)?;
        let loaded = self
            .load_open_credential(tenant_id, connection_id, expected_version)
            .await?;
        let outcome = self
            .provider_client
            .as_ref()
            .ok_or(ServiceError::CredentialStorageUnavailable)?
            .test_connection(loaded.provider, &loaded.api_key)
            .await;
        let tested_at = Utc::now();
        let failure_category = outcome
            .as_ref()
            .err()
            .map(|failure| failure.category.as_str().to_owned());
        let succeeded = outcome.is_ok();
        let connection = self
            .persist_test_outcome(
                tenant_id,
                connection_id,
                expected_version,
                loaded.credential_version,
                actor,
                request_context,
                tested_at,
                failure_category.as_deref(),
                "administration.ai_providers.connections.test",
            )
            .await?;
        Ok(ConnectionTestResult {
            connection,
            outcome: ConnectionTestOutcome {
                status: if succeeded { "succeeded" } else { "failed" }.to_owned(),
                failure_category,
                tested_at,
            },
        })
    }

    /// Lists the current cached immutable model snapshot.
    pub async fn list_models(
        &self,
        tenant_id: Uuid,
        connection_id: Uuid,
    ) -> Result<ConnectionModelSnapshot, ServiceError> {
        let header = sqlx::query_as::<_, ModelSnapshotHeader>(
            r#"
            SELECT id, provider, credential_version, model_catalog_version,
                   model_catalog_refreshed_at
            FROM ai_provider_connections
            WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL
            "#,
        )
        .bind(tenant_id)
        .bind(connection_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(ServiceError::NotFound)?;
        let models = if header.model_catalog_version == 0 {
            Vec::new()
        } else {
            sqlx::query_as::<_, ProviderModel>(
                r#"
                SELECT provider_model_id, display_name, context_window_tokens,
                       supports_tools, source
                FROM ai_provider_models
                WHERE tenant_id = $1 AND connection_id = $2
                  AND credential_version = $3 AND catalog_version = $4
                  AND deleted_at IS NULL
                ORDER BY provider_model_id
                "#,
            )
            .bind(tenant_id)
            .bind(connection_id)
            .bind(header.credential_version)
            .bind(header.model_catalog_version)
            .fetch_all(&self.pool)
            .await?
        };
        Ok(ConnectionModelSnapshot {
            connection_id: header.id,
            provider: header.provider,
            credential_version: header.credential_version,
            refreshed_at: header.model_catalog_refreshed_at,
            models,
        })
    }

    /// Refreshes and retains a new immutable provider model snapshot.
    pub async fn refresh_models(
        &self,
        tenant_id: Uuid,
        connection_id: Uuid,
        expected_version: i64,
        actor: AuditActor,
        request_context: RequestContext,
    ) -> Result<ConnectionModelSnapshot, ServiceError> {
        let expected_version = positive_version(expected_version)?;
        let loaded = self
            .load_open_credential(tenant_id, connection_id, expected_version)
            .await?;
        let models = match self
            .provider_client
            .as_ref()
            .ok_or(ServiceError::CredentialStorageUnavailable)?
            .fetch_models(loaded.provider, &loaded.api_key)
            .await
        {
            Ok(models) => models,
            Err(failure) => {
                self.persist_test_outcome(
                    tenant_id,
                    connection_id,
                    expected_version,
                    loaded.credential_version,
                    actor,
                    request_context,
                    Utc::now(),
                    Some(failure.category.as_str()),
                    "administration.ai_providers.models.refresh",
                )
                .await?;
                return Err(ServiceError::ProviderFailed(failure.category));
            }
        };

        let refreshed_at = Utc::now();
        let mut transaction = self.pool.begin().await?;
        let locked = sqlx::query_as::<_, (i64, i64, i64)>(
            r#"
            SELECT version, credential_version, model_catalog_version
            FROM ai_provider_connections
            WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL
            FOR UPDATE
            "#,
        )
        .bind(tenant_id)
        .bind(connection_id)
        .fetch_optional(&mut *transaction)
        .await?;
        let Some(locked) = locked else {
            append_audit(
                &mut transaction,
                tenant_id,
                actor,
                request_context,
                "administration.ai_providers.models.refresh",
                connection_id,
                AuditOutcome::Succeeded,
                completed_attempt_metadata(
                    loaded.credential_version,
                    None,
                    false,
                    Some(models.len()),
                ),
            )
            .await?;
            transaction.commit().await?;
            return Err(ServiceError::NotFound);
        };
        if locked.0 != expected_version || locked.1 != loaded.credential_version {
            append_audit(
                &mut transaction,
                tenant_id,
                actor,
                request_context,
                "administration.ai_providers.models.refresh",
                connection_id,
                AuditOutcome::Succeeded,
                completed_attempt_metadata(
                    loaded.credential_version,
                    None,
                    false,
                    Some(models.len()),
                ),
            )
            .await?;
            transaction.commit().await?;
            return Err(stale_version());
        }
        let catalog_version = next_catalog_version(locked.2)?;

        if !models.is_empty() {
            let mut builder = QueryBuilder::<Postgres>::new(
                "INSERT INTO ai_provider_models (tenant_id, connection_id, credential_version, catalog_version, provider_model_id, display_name, context_window_tokens, supports_tools, source, refreshed_at) ",
            );
            builder.push_values(&models, |mut row, model| {
                row.push_bind(tenant_id)
                    .push_bind(connection_id)
                    .push_bind(loaded.credential_version)
                    .push_bind(catalog_version)
                    .push_bind(&model.id)
                    .push_bind(&model.display_name)
                    .push_bind(model.context_window_tokens)
                    .push_bind(model.supports_tools)
                    .push_bind(&model.source)
                    .push_bind(refreshed_at);
            });
            builder.build().execute(&mut *transaction).await?;
        }

        let update = sqlx::query(
            r#"
            UPDATE ai_provider_connections
            SET model_catalog_version = $1,
                model_catalog_refreshed_at = $2,
                status = 'ready',
                last_tested_at = $2,
                last_test_status = 'succeeded',
                last_failure_category = NULL,
                version = version + 1,
                updated_at = NOW()
            WHERE tenant_id = $3 AND id = $4 AND version = $5
              AND credential_version = $6 AND deleted_at IS NULL
            "#,
        )
        .bind(catalog_version)
        .bind(refreshed_at)
        .bind(tenant_id)
        .bind(connection_id)
        .bind(expected_version)
        .bind(loaded.credential_version)
        .execute(&mut *transaction)
        .await?;
        if update.rows_affected() != 1 {
            return Err(stale_version());
        }
        append_audit(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "administration.ai_providers.models.refresh",
            connection_id,
            AuditOutcome::Succeeded,
            {
                let mut evidence = completed_attempt_metadata(
                    loaded.credential_version,
                    None,
                    true,
                    Some(models.len()),
                );
                evidence.insert("catalog_version".to_owned(), Value::from(catalog_version));
                evidence
            },
        )
        .await?;
        transaction.commit().await?;
        self.list_models(tenant_id, connection_id).await
    }

    /// Purges credential material and disconnects only an unreferenced connection.
    pub async fn disconnect(
        &self,
        tenant_id: Uuid,
        connection_id: Uuid,
        expected_version: i64,
        actor: AuditActor,
        request_context: RequestContext,
    ) -> Result<DisconnectedConnection, ServiceError> {
        person_actor_id(actor)?;
        let expected_version = positive_version(expected_version)?;
        let mut transaction = self.pool.begin().await?;
        let current_version = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT version
            FROM ai_provider_connections
            WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL
            FOR UPDATE
            "#,
        )
        .bind(tenant_id)
        .bind(connection_id)
        .fetch_optional(&mut *transaction)
        .await?
        .ok_or(ServiceError::NotFound)?;
        if current_version != expected_version {
            return Err(stale_version());
        }
        if connection_has_route_reference(&mut transaction, tenant_id, connection_id).await? {
            return Err(ServiceError::conflict(
                "connection_in_use",
                "Remove this connection from every AI route before disconnecting it",
            ));
        }
        let update = sqlx::query(
            r#"
            UPDATE ai_provider_connections
            SET status = 'disconnected',
                credential_ciphertext = NULL,
                credential_nonce = NULL,
                credential_key_id = NULL,
                credential_fingerprint = NULL,
                credential_version = credential_version + 1,
                version = version + 1,
                deleted_at = NOW(),
                updated_at = NOW()
            WHERE tenant_id = $1 AND id = $2 AND version = $3 AND deleted_at IS NULL
            "#,
        )
        .bind(tenant_id)
        .bind(connection_id)
        .bind(expected_version)
        .execute(&mut *transaction)
        .await?;
        if update.rows_affected() != 1 {
            return Err(stale_version());
        }
        append_audit(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "administration.ai_providers.connections.disconnect",
            connection_id,
            AuditOutcome::Succeeded,
            Map::new(),
        )
        .await?;
        transaction.commit().await?;
        Ok(DisconnectedConnection {
            disconnected_id: connection_id,
        })
    }

    fn encrypt(
        &self,
        tenant_id: Uuid,
        connection_id: Uuid,
        provider: ProviderKey,
        auth_method: AuthMethod,
        credential_version: i64,
        api_key: &crate::types::ApiKey,
    ) -> Result<EncryptedCredential, ServiceError> {
        self.keyring
            .as_ref()
            .ok_or(ServiceError::CredentialStorageUnavailable)?
            .encrypt(
                credential_context(
                    tenant_id,
                    connection_id,
                    provider,
                    auth_method,
                    credential_version,
                ),
                api_key,
            )
            .map_err(|_| ServiceError::CredentialUnavailable)
    }

    async fn load_open_credential(
        &self,
        tenant_id: Uuid,
        connection_id: Uuid,
        expected_version: i64,
    ) -> Result<OpenCredential, ServiceError> {
        let row = load_credential_row(&self.pool, tenant_id, connection_id).await?;
        if row.version != expected_version {
            return Err(stale_version());
        }
        let provider = ProviderKey::from_str(&row.provider)?;
        let auth_method = AuthMethod::from_str(&row.auth_method)?;
        let encrypted = EncryptedCredential {
            ciphertext: row
                .credential_ciphertext
                .ok_or(ServiceError::CredentialUnavailable)?,
            nonce: row
                .credential_nonce
                .ok_or(ServiceError::CredentialUnavailable)?,
            key_id: row
                .credential_key_id
                .ok_or(ServiceError::CredentialUnavailable)?,
            envelope_version: row.credential_envelope_version,
        };
        let api_key = self
            .keyring
            .as_ref()
            .ok_or(ServiceError::CredentialStorageUnavailable)?
            .decrypt(
                credential_context(
                    tenant_id,
                    connection_id,
                    provider,
                    auth_method,
                    row.credential_version,
                ),
                &encrypted,
            )
            .map_err(|_| ServiceError::CredentialUnavailable)?;
        Ok(OpenCredential {
            provider,
            credential_version: row.credential_version,
            api_key,
        })
    }

    #[allow(clippy::too_many_arguments)]
    async fn persist_test_outcome(
        &self,
        tenant_id: Uuid,
        connection_id: Uuid,
        expected_version: i64,
        credential_version: i64,
        actor: AuditActor,
        request_context: RequestContext,
        tested_at: DateTime<Utc>,
        failure_category: Option<&str>,
        action_key: &'static str,
    ) -> Result<AiProviderConnection, ServiceError> {
        let succeeded = failure_category.is_none();
        let mut transaction = self.pool.begin().await?;
        let update = sqlx::query(
            r#"
            UPDATE ai_provider_connections
            SET status = $1,
                last_tested_at = $2,
                last_test_status = $3,
                last_failure_category = $4,
                version = version + 1,
                updated_at = NOW()
            WHERE tenant_id = $5 AND id = $6 AND version = $7
              AND credential_version = $8 AND deleted_at IS NULL
            "#,
        )
        .bind(if succeeded {
            ConnectionStatus::Ready.as_str()
        } else {
            ConnectionStatus::Error.as_str()
        })
        .bind(tested_at)
        .bind(if succeeded { "succeeded" } else { "failed" })
        .bind(failure_category)
        .bind(tenant_id)
        .bind(connection_id)
        .bind(expected_version)
        .bind(credential_version)
        .execute(&mut *transaction)
        .await?;
        let state_persisted = update.rows_affected() == 1;
        append_audit(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            action_key,
            connection_id,
            if succeeded {
                AuditOutcome::Succeeded
            } else {
                AuditOutcome::Failed
            },
            completed_attempt_metadata(credential_version, failure_category, state_persisted, None),
        )
        .await?;
        transaction.commit().await?;
        if !state_persisted {
            return Err(stale_version());
        }
        self.read_connection(tenant_id, connection_id).await
    }
}

#[derive(Debug, FromRow)]
struct CredentialRow {
    provider: String,
    auth_method: String,
    credential_ciphertext: Option<Vec<u8>>,
    credential_nonce: Option<Vec<u8>>,
    credential_key_id: Option<String>,
    credential_envelope_version: i16,
    credential_version: i64,
    version: i64,
}

#[derive(Debug)]
struct OpenCredential {
    provider: ProviderKey,
    credential_version: i64,
    api_key: crate::types::ApiKey,
}

#[derive(Debug, FromRow)]
struct ModelSnapshotHeader {
    id: Uuid,
    provider: String,
    credential_version: i64,
    model_catalog_version: i64,
    model_catalog_refreshed_at: Option<DateTime<Utc>>,
}

async fn fetch_connection(
    pool: &PgPool,
    tenant_id: Uuid,
    connection_id: Uuid,
) -> Result<AiProviderConnection, ServiceError> {
    let query = format!(
        "{CONNECTION_PROJECTION} WHERE c.tenant_id = $1 AND c.id = $2 AND c.deleted_at IS NULL"
    );
    sqlx::query_as::<_, AiProviderConnection>(&query)
        .bind(tenant_id)
        .bind(connection_id)
        .fetch_optional(pool)
        .await?
        .ok_or(ServiceError::NotFound)
}

async fn load_credential_row(
    pool: &PgPool,
    tenant_id: Uuid,
    connection_id: Uuid,
) -> Result<CredentialRow, ServiceError> {
    sqlx::query_as::<_, CredentialRow>(
        r#"
        SELECT provider, auth_method, credential_ciphertext, credential_nonce,
               credential_key_id, credential_envelope_version,
               credential_version, version
        FROM ai_provider_connections
        WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL
        "#,
    )
    .bind(tenant_id)
    .bind(connection_id)
    .fetch_optional(pool)
    .await?
    .ok_or(ServiceError::NotFound)
}

fn credential_context(
    tenant_id: Uuid,
    connection_id: Uuid,
    provider: ProviderKey,
    auth_method: AuthMethod,
    credential_version: i64,
) -> CredentialContext<'static> {
    CredentialContext {
        tenant_id,
        connection_id,
        provider,
        auth_method,
        credential_version,
        domain: CREDENTIAL_DOMAIN,
    }
}

fn credential_fingerprint(provider: ProviderKey, api_key: &crate::types::ApiKey) -> String {
    let mut digest = Sha256::new();
    digest.update(provider.as_str().as_bytes());
    digest.update([0]);
    digest.update(api_key.expose().as_bytes());
    let digest = digest.finalize();
    let short = digest[..8]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!("sha256:{short}")
}

fn person_actor_id(actor: AuditActor) -> Result<Uuid, ServiceError> {
    if actor.kind() != AuditActorKind::Person {
        return Err(ServiceError::invalid(
            "human_workflow_required",
            "Provider credentials can only be managed directly by a person",
        ));
    }
    actor.user_id().ok_or_else(|| {
        ServiceError::invalid(
            "human_workflow_required",
            "Provider credentials can only be managed directly by a person",
        )
    })
}

fn stale_version() -> ServiceError {
    ServiceError::conflict(
        "stale_connection",
        "This provider connection changed; reload it before trying again",
    )
}

async fn require_updated(
    rows_affected: u64,
    pool: &PgPool,
    tenant_id: Uuid,
    connection_id: Uuid,
) -> Result<(), ServiceError> {
    if rows_affected == 1 {
        return Ok(());
    }
    let exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM ai_provider_connections WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL)",
    )
    .bind(tenant_id)
    .bind(connection_id)
    .fetch_one(pool)
    .await?;
    if exists {
        Err(stale_version())
    } else {
        Err(ServiceError::NotFound)
    }
}

fn map_write_error(error: sqlx::Error) -> ServiceError {
    if error
        .as_database_error()
        .is_some_and(|database_error| database_error.code().as_deref() == Some("23505"))
    {
        ServiceError::conflict(
            "connection_exists",
            "A connection with this provider label or credential already exists",
        )
    } else {
        ServiceError::Storage(error)
    }
}

fn metadata<const N: usize>(entries: [(&str, Value); N]) -> Map<String, Value> {
    entries
        .into_iter()
        .map(|(key, value)| (key.to_owned(), value))
        .collect()
}

fn completed_attempt_metadata(
    credential_version: i64,
    failure_category: Option<&str>,
    state_persisted: bool,
    model_count: Option<usize>,
) -> Map<String, Value> {
    let mut evidence = metadata([
        ("credential_version", Value::from(credential_version)),
        (
            "failure_category",
            failure_category.map_or(Value::Null, |value| Value::String(value.to_owned())),
        ),
        ("state_persisted", Value::from(state_persisted)),
        ("stale_after_upstream", Value::from(!state_persisted)),
    ]);
    if let Some(model_count) = model_count {
        evidence.insert("model_count".to_owned(), Value::from(model_count as u64));
    }
    evidence
}

fn next_catalog_version(current: i64) -> Result<i64, ServiceError> {
    current.checked_add(1).ok_or_else(stale_version)
}

#[allow(clippy::too_many_arguments)]
async fn append_audit(
    transaction: &mut sqlx::Transaction<'_, Postgres>,
    tenant_id: Uuid,
    actor: AuditActor,
    request_context: RequestContext,
    action_key: &'static str,
    connection_id: Uuid,
    outcome: AuditOutcome,
    metadata: Map<String, Value>,
) -> Result<(), ServiceError> {
    let event = NewAuditEvent::new(tenant_id, actor, action_key, outcome, request_context)
        .with_target(AuditTarget::new("ai_provider_connection", connection_id))
        .with_redacted_metadata(metadata);
    cp_audit::append(&mut **transaction, &event).await?;
    Ok(())
}

async fn connection_has_route_reference(
    transaction: &mut sqlx::Transaction<'_, Postgres>,
    tenant_id: Uuid,
    connection_id: Uuid,
) -> Result<bool, ServiceError> {
    let route_table_exists =
        sqlx::query_scalar::<_, bool>("SELECT TO_REGCLASS('public.ai_task_routes') IS NOT NULL")
            .fetch_one(&mut **transaction)
            .await?;
    if !route_table_exists {
        return Ok(false);
    }
    sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM ai_task_routes WHERE tenant_id = $1 AND connection_id = $2 AND deleted_at IS NULL)",
    )
    .bind(tenant_id)
    .bind(connection_id)
    .fetch_one(&mut **transaction)
    .await
    .map_err(ServiceError::from)
}

#[cfg(test)]
mod tests {
    use cp_audit::AuditActor;
    use serde_json::Value;
    use uuid::Uuid;

    use crate::types::{ApiKey, ProviderKey};

    use super::{
        CONNECTION_PROJECTION, completed_attempt_metadata, credential_fingerprint,
        next_catalog_version, person_actor_id,
    };

    const PROVIDER_MIGRATION: &str =
        include_str!("../../../../migrations/078_create_ai_provider_connections.sql");

    #[test]
    fn public_projection_never_selects_credential_storage_columns() {
        for forbidden in [
            "credential_ciphertext",
            "credential_nonce",
            "credential_key_id",
            "credential_envelope_version",
        ] {
            assert!(!CONNECTION_PROJECTION.contains(forbidden));
        }
    }

    #[test]
    fn fingerprint_is_deterministic_provider_bound_and_not_a_key_suffix() {
        let key = ApiKey::parse("secret-key-material-123").unwrap();
        let first = credential_fingerprint(ProviderKey::OpenAi, &key);
        assert_eq!(first, credential_fingerprint(ProviderKey::OpenAi, &key));
        assert_ne!(first, credential_fingerprint(ProviderKey::Anthropic, &key));
        assert!(first.starts_with("sha256:"));
        assert!(!first.contains("l-123"));
    }

    #[test]
    fn credential_and_disconnect_actor_gate_accepts_only_a_person() {
        let person_id = Uuid::new_v4();
        assert_eq!(
            person_actor_id(AuditActor::person(person_id)).unwrap_or_else(|_| unreachable!()),
            person_id
        );
        for actor in [AuditActor::agent(person_id), AuditActor::system()] {
            let error = person_actor_id(actor)
                .err()
                .unwrap_or_else(|| unreachable!());
            assert_eq!(error.code(), "human_workflow_required");
        }
    }

    #[test]
    fn tenant_ownership_is_enforced_by_composite_foreign_keys() {
        assert!(PROVIDER_MIGRATION.contains(
            "FOREIGN KEY (configured_by, tenant_id)\n        REFERENCES users(id, tenant_id)"
        ));
        assert!(PROVIDER_MIGRATION.contains(
            "FOREIGN KEY (connection_id, tenant_id)\n        REFERENCES ai_provider_connections(id, tenant_id)"
        ));
    }

    #[test]
    fn model_snapshot_identity_never_reuses_or_mixes_credentials() {
        assert!(CONNECTION_PROJECTION.contains(
            "m.credential_version = c.credential_version\n              AND m.catalog_version = c.model_catalog_version"
        ));
        assert!(PROVIDER_MIGRATION.contains(
            "UNIQUE (connection_id, credential_version, catalog_version, provider_model_id)"
        ));
        assert_eq!(
            next_catalog_version(7).unwrap_or_else(|_| unreachable!()),
            8
        );
        assert!(!PROVIDER_MIGRATION.contains("model_catalog_version = 0"));
    }

    #[test]
    fn stale_upstream_attempts_keep_only_reduced_audit_evidence() {
        let evidence = completed_attempt_metadata(3, Some("authentication"), false, Some(17));
        assert_eq!(evidence.get("credential_version"), Some(&Value::from(3)));
        assert_eq!(
            evidence.get("failure_category"),
            Some(&Value::String("authentication".to_owned()))
        );
        assert_eq!(evidence.get("state_persisted"), Some(&Value::Bool(false)));
        assert_eq!(
            evidence.get("stale_after_upstream"),
            Some(&Value::Bool(true))
        );
        assert_eq!(evidence.get("model_count"), Some(&Value::from(17_u64)));
        assert_eq!(evidence.len(), 5);
    }
}

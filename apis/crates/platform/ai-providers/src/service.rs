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

pub(crate) const CREDENTIAL_DOMAIN: &str = "campus-pilot/ai-provider-credential";

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
        approval.id AS provider_data_approval_id,
        approval.approval_version AS provider_data_approval_version,
        approval.approval_class AS provider_data_approval_class,
        'external_managed'::TEXT AS execution_environment_class,
        c.created_at,
        c.updated_at
    FROM ai_provider_connections c
    JOIN users u ON u.id = c.configured_by AND u.tenant_id = c.tenant_id
    JOIN LATERAL (
        SELECT current_approval.id, current_approval.approval_version,
               current_approval.approval_class
        FROM ai_provider_data_approval_versions current_approval
        WHERE current_approval.tenant_id = c.tenant_id
          AND current_approval.connection_id = c.id
        ORDER BY current_approval.approval_version DESC
        LIMIT 1
    ) approval ON TRUE
"#;

/// Shared service used by HTTP routes and secret-free Agent read handlers.
#[derive(Debug, Clone)]
pub struct AiProviderOps {
    pub(crate) pool: PgPool,
    pub(crate) keyring: Option<CredentialKeyring>,
    pub(crate) provider_client: Option<ProviderHttpClient>,
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

        sqlx::query(
            r#"
            INSERT INTO ai_provider_data_approval_versions (
                id, tenant_id, connection_id, approval_version, approval_class,
                change_source, changed_by, change_reason
            )
            VALUES ($1, $2, $3, 1, 'unapproved', 'system_default', NULL, $4)
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(tenant_id)
        .bind(connection_id)
        .bind("Initial unapproved provider data eligibility.")
        .execute(&mut *transaction)
        .await
        .map_err(map_write_error)?;

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

    /// Appends one human-owned data approval version with optimistic concurrency.
    pub async fn set_data_approval(
        &self,
        tenant_id: Uuid,
        connection_id: Uuid,
        actor: AuditActor,
        request_context: RequestContext,
        command: crate::types::SetProviderDataApprovalCommand,
    ) -> Result<crate::types::ProviderDataApproval, ServiceError> {
        let changed_by = person_actor_id(actor)?;
        let mut transaction = self.pool.begin().await?;
        let current = sqlx::query_as::<_, CurrentDataApprovalRow>(
            r#"
            SELECT approval_version
            FROM ai_provider_data_approval_versions
            WHERE tenant_id = $1 AND connection_id = $2
            ORDER BY approval_version DESC
            LIMIT 1
            FOR UPDATE
            "#,
        )
        .bind(tenant_id)
        .bind(connection_id)
        .fetch_optional(&mut *transaction)
        .await?
        .ok_or(ServiceError::NotFound)?;
        if current.approval_version != command.expected_approval_version {
            return Err(ServiceError::conflict(
                "stale_provider_data_approval",
                "Provider data approval changed; reload before saving",
            ));
        }
        let next_version = current
            .approval_version
            .checked_add(1)
            .filter(|version| *version <= 9_007_199_254_740_991)
            .ok_or_else(|| {
                ServiceError::conflict(
                    "provider_data_approval_version_exhausted",
                    "Provider data approval cannot advance further",
                )
            })?;
        let approval_id = Uuid::new_v4();
        let inserted = sqlx::query(
            r#"
            INSERT INTO ai_provider_data_approval_versions (
                id, tenant_id, connection_id, approval_version, approval_class,
                change_source, changed_by, change_reason
            )
            VALUES ($1, $2, $3, $4, $5, 'administrator', $6, $7)
            "#,
        )
        .bind(approval_id)
        .bind(tenant_id)
        .bind(connection_id)
        .bind(next_version)
        .bind(command.approval_class.as_str())
        .bind(changed_by)
        .bind(&command.change_reason)
        .execute(&mut *transaction)
        .await;
        if let Err(error) = inserted {
            return Err(map_write_error(error));
        }
        append_audit(
            &mut transaction,
            tenant_id,
            actor,
            request_context,
            "administration.ai_providers.connections.data_approval.update",
            connection_id,
            AuditOutcome::Succeeded,
            metadata([
                (
                    "approval_class",
                    Value::String(command.approval_class.as_str().to_owned()),
                ),
                (
                    "previous_approval_version",
                    Value::from(current.approval_version),
                ),
                ("approval_version", Value::from(next_version)),
            ]),
        )
        .await?;
        transaction.commit().await?;
        fetch_data_approval(&self.pool, tenant_id, connection_id, approval_id).await
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
                       max_output_tokens, supports_tools, source
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
                "INSERT INTO ai_provider_models (tenant_id, connection_id, credential_version, catalog_version, provider_model_id, display_name, context_window_tokens, max_output_tokens, supports_tools, source, refreshed_at) ",
            );
            builder.push_values(&models, |mut row, model| {
                row.push_bind(tenant_id)
                    .push_bind(connection_id)
                    .push_bind(loaded.credential_version)
                    .push_bind(catalog_version)
                    .push_bind(&model.id)
                    .push_bind(&model.display_name)
                    .push_bind(model.context_window_tokens)
                    .push_bind(model.max_output_tokens)
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

#[derive(Debug, FromRow)]
struct CurrentDataApprovalRow {
    approval_version: i64,
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

async fn fetch_data_approval(
    pool: &PgPool,
    tenant_id: Uuid,
    connection_id: Uuid,
    approval_id: Uuid,
) -> Result<crate::types::ProviderDataApproval, ServiceError> {
    sqlx::query_as::<_, crate::types::ProviderDataApproval>(
        r#"
        SELECT approval.id, approval.connection_id, approval.approval_version,
               approval.approval_class,
               'external_managed'::TEXT AS execution_environment_class,
               approval.change_source, changed_by.full_name AS changed_by_name,
               approval.change_reason, approval.created_at
        FROM ai_provider_data_approval_versions approval
        LEFT JOIN users changed_by
          ON changed_by.id = approval.changed_by
         AND changed_by.tenant_id = approval.tenant_id
        WHERE approval.id = $1
          AND approval.tenant_id = $2
          AND approval.connection_id = $3
        "#,
    )
    .bind(approval_id)
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

pub(crate) fn credential_context(
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
    use std::{collections::BTreeMap, time::Duration};

    use base64::{Engine as _, engine::general_purpose::STANDARD};
    use cp_audit::{AuditActor, RequestContext};
    use httpmock::{Method::GET, MockServer};
    use serde_json::{Value, json};
    use sqlx::{PgPool, postgres::PgPoolOptions};
    use uuid::Uuid;

    use crate::{
        CredentialKeyring,
        client::{ProviderEndpoints, ProviderHttpClient},
        types::{
            ApiKey, CreateConnectionCommand, ProviderKey, RotateCredentialCommand,
            SetProviderDataApprovalCommand, UpdateConnectionCommand,
        },
    };

    use super::{
        AiProviderOps, CONNECTION_PROJECTION, completed_attempt_metadata,
        connection_has_route_reference, credential_fingerprint, map_write_error,
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

    fn keyring() -> CredentialKeyring {
        CredentialKeyring::from_base64(
            BTreeMap::from([("active".to_owned(), STANDARD.encode([17_u8; 32]))]),
            "active",
        )
        .unwrap()
    }

    async fn insert_tenant_and_user(pool: &PgPool) -> (Uuid, Uuid) {
        let tenant_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let suffix = tenant_id.simple();
        sqlx::query("INSERT INTO tenants (id, slug, name) VALUES ($1, $2, $3)")
            .bind(tenant_id)
            .bind(format!("provider-service-{suffix}"))
            .bind("Provider service contract")
            .execute(pool)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO users (id, tenant_id, email, password_hash, full_name) VALUES ($1, $2, $3, 'not-a-login', 'Provider Service Contract')",
        )
        .bind(user_id)
        .bind(tenant_id)
        .bind(format!("provider-service-{suffix}@example.invalid"))
        .execute(pool)
        .await
        .unwrap();
        (tenant_id, user_id)
    }

    fn create_command(provider: &str, label: &str, key: &str) -> CreateConnectionCommand {
        CreateConnectionCommand::parse(provider, "api_key", label, key).unwrap()
    }

    fn context() -> RequestContext {
        RequestContext::generate(None)
    }

    #[tokio::test]
    #[ignore = "requires a disposable migrated AI_PROVIDER_EXECUTION_TEST_DATABASE_URL"]
    async fn postgres_administration_lifecycle_is_tenant_scoped_audited_and_fail_closed() {
        let database_url = std::env::var("AI_PROVIDER_EXECUTION_TEST_DATABASE_URL")
            .expect("AI_PROVIDER_EXECUTION_TEST_DATABASE_URL must target a disposable database");
        let pool = PgPoolOptions::new()
            .max_connections(8)
            .connect(&database_url)
            .await
            .unwrap();
        let (tenant_id, user_id) = insert_tenant_and_user(&pool).await;
        let (other_tenant_id, _) = insert_tenant_and_user(&pool).await;
        let actor = AuditActor::person(user_id);

        let server = MockServer::start_async().await;
        let openai_models = server
            .mock_async(|when, then| {
                when.method(GET).path("/openai/models");
                then.status(200).json_body(json!({
                    "data":[
                        {"id":"gpt-5"},
                        {"id":"gpt-5"},
                        {"id":"text-embedding-3-small"}
                    ]
                }));
            })
            .await;
        let ops = AiProviderOps::new(
            pool.clone(),
            Some(keyring()),
            ProviderHttpClient::with_endpoints(ProviderEndpoints::all(&server.base_url())),
        );
        let reads = AiProviderOps::for_reads(pool.clone());

        assert!(reads.list_connections(tenant_id).await.unwrap().is_empty());
        assert!(
            reads
                .list_connections(other_tenant_id)
                .await
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            reads
                .read_connection(tenant_id, Uuid::new_v4())
                .await
                .unwrap_err()
                .code(),
            "connection_not_found"
        );
        assert_eq!(
            reads
                .create_connection(
                    tenant_id,
                    actor,
                    context(),
                    create_command("openai", "Unavailable", "missing-key-material-123"),
                )
                .await
                .unwrap_err()
                .code(),
            "credential_storage_unavailable"
        );
        assert_eq!(
            ops.create_connection(
                tenant_id,
                AuditActor::agent(user_id),
                context(),
                create_command("openai", "Agent", "agent-key-material-123"),
            )
            .await
            .unwrap_err()
            .code(),
            "human_workflow_required"
        );

        let primary_key = "primary-provider-key-material-123";
        let primary = ops
            .create_connection(
                tenant_id,
                actor,
                context(),
                create_command("openai", "Primary", primary_key),
            )
            .await
            .unwrap();
        assert_eq!(primary.status, "untested");
        assert_eq!(primary.version, 1);
        assert_eq!(primary.provider_data_approval_version, 1);
        assert_eq!(primary.provider_data_approval_class, "unapproved");
        assert_eq!(primary.execution_environment_class, "external_managed");
        assert_eq!(
            ops.set_data_approval(
                tenant_id,
                primary.id,
                AuditActor::system(),
                context(),
                SetProviderDataApprovalCommand::parse(
                    "sensitive_data_approved",
                    1,
                    "Approve sensitive records for the contract.",
                )
                .unwrap(),
            )
            .await
            .unwrap_err()
            .code(),
            "human_workflow_required"
        );
        assert_eq!(
            ops.set_data_approval(
                tenant_id,
                primary.id,
                actor,
                context(),
                SetProviderDataApprovalCommand::parse(
                    "sensitive_data_approved",
                    2,
                    "Reject stale administrator approval.",
                )
                .unwrap(),
            )
            .await
            .unwrap_err()
            .code(),
            "stale_provider_data_approval"
        );
        let approval = ops
            .set_data_approval(
                tenant_id,
                primary.id,
                actor,
                context(),
                SetProviderDataApprovalCommand::parse(
                    "sensitive_data_approved",
                    1,
                    "Approve sensitive records for the contract.",
                )
                .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(approval.connection_id, primary.id);
        assert_eq!(approval.approval_version, 2);
        assert_eq!(approval.approval_class, "sensitive_data_approved");
        assert_eq!(approval.execution_environment_class, "external_managed");
        assert_eq!(
            reads
                .read_connection(tenant_id, primary.id)
                .await
                .unwrap()
                .provider_data_approval_version,
            2
        );
        assert_eq!(reads.list_connections(tenant_id).await.unwrap().len(), 1);
        assert_eq!(
            reads
                .read_connection(other_tenant_id, primary.id)
                .await
                .unwrap_err()
                .code(),
            "connection_not_found"
        );

        for (label, key) in [
            ("Primary", "different-provider-key-material-123"),
            ("Fingerprint collision", primary_key),
        ] {
            assert_eq!(
                ops.create_connection(
                    tenant_id,
                    actor,
                    context(),
                    create_command("openai", label, key),
                )
                .await
                .unwrap_err()
                .code(),
                "connection_exists"
            );
        }

        let second = ops
            .create_connection(
                tenant_id,
                actor,
                context(),
                create_command("openai", "Secondary", "secondary-provider-key-material-123"),
            )
            .await
            .unwrap();
        assert_eq!(
            ops.update_connection(
                tenant_id,
                second.id,
                actor,
                context(),
                UpdateConnectionCommand::parse("Primary", second.version).unwrap(),
            )
            .await
            .unwrap_err()
            .code(),
            "connection_exists"
        );
        assert_eq!(
            ops.update_connection(
                tenant_id,
                second.id,
                actor,
                context(),
                UpdateConnectionCommand::parse("Secondary renamed", 2).unwrap(),
            )
            .await
            .unwrap_err()
            .code(),
            "stale_connection"
        );
        assert_eq!(
            ops.update_connection(
                tenant_id,
                Uuid::new_v4(),
                actor,
                context(),
                UpdateConnectionCommand::parse("Missing", 1).unwrap(),
            )
            .await
            .unwrap_err()
            .code(),
            "connection_not_found"
        );

        let primary = ops
            .update_connection(
                tenant_id,
                primary.id,
                actor,
                context(),
                UpdateConnectionCommand::parse("Primary renamed", primary.version).unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(primary.version, 2);
        assert_eq!(primary.account_label, "Primary renamed");

        assert_eq!(
            ops.rotate_credential(
                tenant_id,
                second.id,
                AuditActor::system(),
                context(),
                RotateCredentialCommand::parse("rotation-key-material-123", 1).unwrap(),
            )
            .await
            .unwrap_err()
            .code(),
            "human_workflow_required"
        );
        assert_eq!(
            ops.rotate_credential(
                tenant_id,
                second.id,
                actor,
                context(),
                RotateCredentialCommand::parse("rotation-key-material-123", 2).unwrap(),
            )
            .await
            .unwrap_err()
            .code(),
            "stale_connection"
        );
        assert_eq!(
            reads
                .rotate_credential(
                    tenant_id,
                    second.id,
                    actor,
                    context(),
                    RotateCredentialCommand::parse("rotation-key-material-123", 1).unwrap(),
                )
                .await
                .unwrap_err()
                .code(),
            "credential_storage_unavailable"
        );
        let second = ops
            .rotate_credential(
                tenant_id,
                second.id,
                actor,
                context(),
                RotateCredentialCommand::parse("rotation-key-material-123", 1).unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(second.version, 2);
        assert_eq!(second.credential_version, 2);

        let no_client = AiProviderOps {
            pool: pool.clone(),
            keyring: Some(keyring()),
            provider_client: None,
        };
        assert_eq!(
            no_client
                .test_connection(tenant_id, second.id, 2, actor, context())
                .await
                .unwrap_err()
                .code(),
            "credential_storage_unavailable"
        );
        assert_eq!(
            no_client
                .refresh_models(tenant_id, second.id, 2, actor, context())
                .await
                .unwrap_err()
                .code(),
            "credential_storage_unavailable"
        );

        sqlx::query(
            "UPDATE ai_provider_connections SET credential_key_id = 'missing' WHERE tenant_id = $1 AND id = $2",
        )
        .bind(tenant_id)
        .bind(second.id)
        .execute(&pool)
        .await
        .unwrap();
        assert_eq!(
            ops.test_connection(tenant_id, second.id, 2, actor, context())
                .await
                .unwrap_err()
                .code(),
            "credential_unavailable"
        );
        sqlx::query(
            "UPDATE ai_provider_connections SET credential_key_id = 'active' WHERE tenant_id = $1 AND id = $2",
        )
        .bind(tenant_id)
        .bind(second.id)
        .execute(&pool)
        .await
        .unwrap();

        let tested = ops
            .test_connection(tenant_id, primary.id, primary.version, actor, context())
            .await
            .unwrap();
        assert_eq!(tested.outcome.status, "succeeded");
        assert_eq!(tested.connection.status, "ready");
        assert_eq!(tested.connection.version, 3);
        let empty_snapshot = reads.list_models(tenant_id, primary.id).await.unwrap();
        assert!(empty_snapshot.models.is_empty());
        assert!(empty_snapshot.refreshed_at.is_none());

        let snapshot = ops
            .refresh_models(tenant_id, primary.id, 3, actor, context())
            .await
            .unwrap();
        assert_eq!(snapshot.models.len(), 1);
        assert_eq!(snapshot.models[0].id, "gpt-5");
        assert!(snapshot.refreshed_at.is_some());
        let primary = reads.read_connection(tenant_id, primary.id).await.unwrap();
        assert_eq!(primary.version, 4);
        assert_eq!(primary.model_count, 1);

        let primary = ops
            .rotate_credential(
                tenant_id,
                primary.id,
                actor,
                context(),
                RotateCredentialCommand::parse("rotated-primary-key-material-123", 4).unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(primary.version, 5);
        assert_eq!(primary.credential_version, 2);
        assert!(
            reads
                .list_models(tenant_id, primary.id)
                .await
                .unwrap()
                .models
                .is_empty()
        );
        let tested = ops
            .test_connection(tenant_id, primary.id, 5, actor, context())
            .await
            .unwrap();
        assert_eq!(tested.connection.version, 6);

        let failure_server = MockServer::start_async().await;
        let unavailable = failure_server
            .mock_async(|when, then| {
                when.method(GET).path("/openai/models");
                then.status(503)
                    .body("raw upstream details must not persist");
            })
            .await;
        let failing_ops = AiProviderOps::new(
            pool.clone(),
            Some(keyring()),
            ProviderHttpClient::with_endpoints(ProviderEndpoints::all(&failure_server.base_url())),
        );
        let failed_test = failing_ops
            .test_connection(tenant_id, primary.id, 6, actor, context())
            .await
            .unwrap();
        assert_eq!(failed_test.outcome.status, "failed");
        assert_eq!(
            failed_test.outcome.failure_category.as_deref(),
            Some("unavailable")
        );
        assert_eq!(failed_test.connection.status, "error");
        assert_eq!(failed_test.connection.version, 7);
        let refresh_error = failing_ops
            .refresh_models(tenant_id, primary.id, 7, actor, context())
            .await
            .unwrap_err();
        assert_eq!(refresh_error.code(), "unavailable");
        let primary = reads.read_connection(tenant_id, primary.id).await.unwrap();
        assert_eq!(primary.version, 8);
        assert_eq!(
            primary.last_failure_category.as_deref(),
            Some("unavailable")
        );
        let snapshot = ops
            .refresh_models(tenant_id, primary.id, 8, actor, context())
            .await
            .unwrap();
        assert_eq!(snapshot.credential_version, 2);
        let primary = reads.read_connection(tenant_id, primary.id).await.unwrap();
        assert_eq!(primary.version, 9);
        assert_eq!(primary.model_count, 1);
        openai_models.assert_hits_async(4).await;
        unavailable.assert_hits_async(2).await;

        let model_id = sqlx::query_scalar::<_, Uuid>(
            "SELECT id FROM ai_provider_models WHERE tenant_id = $1 AND connection_id = $2 AND credential_version = 2 AND catalog_version = 2",
        )
        .bind(tenant_id)
        .bind(primary.id)
        .fetch_one(&pool)
        .await
        .unwrap();
        let route_set_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO ai_route_sets (id, tenant_id, scope_kind, configured_by, change_reason) VALUES ($1, $2, 'tenant_default', $3, 'Provider service contract')",
        )
        .bind(route_set_id)
        .bind(tenant_id)
        .bind(user_id)
        .execute(&pool)
        .await
        .unwrap();
        let route_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO ai_task_routes (id, tenant_id, route_set_id, priority, connection_id, model_id, provider_data_approval_id, requires_tools, created_by) VALUES ($1, $2, $3, 1, $4, $5, $6, FALSE, $7)",
        )
        .bind(route_id)
        .bind(tenant_id)
        .bind(route_set_id)
        .bind(primary.id)
        .bind(model_id)
        .bind(approval.id)
        .bind(user_id)
        .execute(&pool)
        .await
        .unwrap();
        assert_eq!(
            ops.disconnect(tenant_id, primary.id, 9, actor, context())
                .await
                .unwrap_err()
                .code(),
            "connection_in_use"
        );
        sqlx::query(
            "UPDATE ai_task_routes SET deleted_at = NOW(), updated_at = NOW() + INTERVAL '1 second' WHERE id = $1",
        )
        .bind(route_id)
        .execute(&pool)
        .await
        .unwrap();
        let disconnected = ops
            .disconnect(tenant_id, primary.id, 9, actor, context())
            .await
            .unwrap();
        assert_eq!(disconnected.disconnected_id, primary.id);
        assert_eq!(
            reads
                .read_connection(tenant_id, primary.id)
                .await
                .unwrap_err()
                .code(),
            "connection_not_found"
        );
        assert_eq!(
            ops.disconnect(tenant_id, second.id, 3, actor, context())
                .await
                .unwrap_err()
                .code(),
            "stale_connection"
        );
        assert_eq!(
            ops.disconnect(tenant_id, Uuid::new_v4(), 1, actor, context())
                .await
                .unwrap_err()
                .code(),
            "connection_not_found"
        );

        let delayed_server = MockServer::start_async().await;
        delayed_server
            .mock_async(|when, then| {
                when.method(GET).path("/openrouter/key");
                then.delay(Duration::from_millis(150)).status(200);
            })
            .await;
        delayed_server
            .mock_async(|when, then| {
                when.method(GET).path("/anthropic/models");
                then.delay(Duration::from_millis(150))
                    .status(200)
                    .json_body(
                        json!({"data":[{"id":"claude-sonnet-4","display_name":"Claude Sonnet 4"}]}),
                    );
            })
            .await;
        delayed_server
            .mock_async(|when, then| {
                when.method(GET).path("/openrouter/models");
                then.delay(Duration::from_millis(150)).status(200).json_body(
                    json!({"data":[{"id":"openai/gpt-5","name":"GPT-5","supported_parameters":[]}]}),
                );
            })
            .await;
        let delayed_ops = AiProviderOps::new(
            pool.clone(),
            Some(keyring()),
            ProviderHttpClient::with_endpoints(ProviderEndpoints::all(&delayed_server.base_url())),
        );

        let stale_test = ops
            .create_connection(
                tenant_id,
                actor,
                context(),
                create_command(
                    "openrouter",
                    "Stale test",
                    "stale-test-provider-key-material-123",
                ),
            )
            .await
            .unwrap();
        let task_ops = delayed_ops.clone();
        let test_task = tokio::spawn(async move {
            task_ops
                .test_connection(tenant_id, stale_test.id, 1, actor, context())
                .await
        });
        tokio::time::sleep(Duration::from_millis(30)).await;
        sqlx::query("UPDATE ai_provider_connections SET version = version + 1 WHERE id = $1")
            .bind(stale_test.id)
            .execute(&pool)
            .await
            .unwrap();
        assert_eq!(
            test_task.await.unwrap().unwrap_err().code(),
            "stale_connection"
        );

        let stale_refresh = ops
            .create_connection(
                tenant_id,
                actor,
                context(),
                create_command(
                    "anthropic",
                    "Stale refresh",
                    "stale-refresh-provider-key-material-123",
                ),
            )
            .await
            .unwrap();
        let task_ops = delayed_ops.clone();
        let refresh_task = tokio::spawn(async move {
            task_ops
                .refresh_models(tenant_id, stale_refresh.id, 1, actor, context())
                .await
        });
        tokio::time::sleep(Duration::from_millis(30)).await;
        sqlx::query("UPDATE ai_provider_connections SET version = version + 1 WHERE id = $1")
            .bind(stale_refresh.id)
            .execute(&pool)
            .await
            .unwrap();
        assert_eq!(
            refresh_task.await.unwrap().unwrap_err().code(),
            "stale_connection"
        );

        let deleted_refresh = ops
            .create_connection(
                tenant_id,
                actor,
                context(),
                create_command(
                    "openrouter",
                    "Deleted refresh",
                    "deleted-refresh-provider-key-material-123",
                ),
            )
            .await
            .unwrap();
        let task_ops = delayed_ops.clone();
        let deleted_task = tokio::spawn(async move {
            task_ops
                .refresh_models(tenant_id, deleted_refresh.id, 1, actor, context())
                .await
        });
        tokio::time::sleep(Duration::from_millis(30)).await;
        sqlx::query(
            "UPDATE ai_provider_connections SET status = 'disconnected', credential_ciphertext = NULL, credential_nonce = NULL, credential_key_id = NULL, credential_fingerprint = NULL, credential_version = credential_version + 1, version = version + 1, deleted_at = NOW(), updated_at = NOW() WHERE id = $1",
        )
        .bind(deleted_refresh.id)
        .execute(&pool)
        .await
        .unwrap();
        assert_eq!(
            deleted_task.await.unwrap().unwrap_err().code(),
            "connection_not_found"
        );

        let audit_payloads = sqlx::query_scalar::<_, String>(
            "SELECT redacted_metadata::TEXT FROM actor_audit_events WHERE tenant_id = $1",
        )
        .bind(tenant_id)
        .fetch_all(&pool)
        .await
        .unwrap();
        assert!(audit_payloads.len() >= 15);
        assert!(audit_payloads.iter().all(|payload| {
            !payload.contains(primary_key)
                && !payload.contains("raw upstream details")
                && !payload.contains("rotated-primary-key")
        }));

        assert_eq!(
            next_catalog_version(i64::MAX).unwrap_err().code(),
            "stale_connection"
        );
        assert_eq!(
            map_write_error(sqlx::Error::Protocol("contract".to_owned())).code(),
            "provider_storage_error"
        );
        let mut transaction = pool.begin().await.unwrap();
        sqlx::query("DROP TABLE ai_task_routes CASCADE")
            .execute(&mut *transaction)
            .await
            .unwrap();
        assert!(
            !connection_has_route_reference(&mut transaction, tenant_id, second.id)
                .await
                .unwrap()
        );
        transaction.rollback().await.unwrap();
    }
}

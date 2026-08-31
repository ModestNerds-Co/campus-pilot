//! Revalidates and opens one provider route target for Agent execution.
//!
//! Credentials are decrypted only after tenant, readiness, credential version,
//! immutable model snapshot, provider model ID, and tool support all match.

use std::str::FromStr;

use cp_common::{
    ProviderApprovalClass, ProviderDataClass, ProviderDataEligibilityError,
    ProviderExecutionEnvironmentClass, evaluate_provider_data_eligibility,
};
use sqlx::FromRow;
use uuid::Uuid;

use crate::{
    crypto::EncryptedCredential,
    execution_binding::{ProviderExecutionFingerprintSource, input_fingerprint},
    execution_client::PreparedProviderHttpRequest,
    execution_types::{ExecuteProviderCommand, ProviderExecutionError, ProviderExecutionResponse},
    service::{AiProviderOps, credential_context},
    types::{AuthMethod, ProviderKey},
};

const EXECUTION_STATE_QUERY: &str = r#"
    SELECT
        route.route_set_id,
        route_set.version AS route_version,
        route.connection_id AS route_connection_id,
        route.model_id AS route_model_snapshot_id,
        route.provider_data_approval_id,
        route.requires_tools AS route_requires_tools,
        approval.approval_class AS provider_data_approval_class,
        latest_approval.id AS latest_provider_data_approval_id,
        'external_managed'::TEXT AS execution_environment_class,
        c.provider,
        c.auth_method,
        c.status,
        c.credential_ciphertext,
        c.credential_nonce,
        c.credential_key_id,
        c.credential_envelope_version,
        c.credential_version,
        c.model_catalog_version AS connection_catalog_version,
        m.provider_model_id AS model_provider_model_id,
        m.credential_version AS model_credential_version,
        m.catalog_version AS snapshot_catalog_version,
        m.context_window_tokens AS model_context_window_tokens,
        m.max_output_tokens AS model_max_output_tokens,
        m.supports_tools AS model_supports_tools,
        m.deleted_at AS model_deleted_at
    FROM ai_task_routes route
    JOIN ai_route_sets route_set
      ON route_set.id = route.route_set_id
     AND route_set.tenant_id = route.tenant_id
     AND route_set.deleted_at IS NULL
    JOIN ai_provider_connections c
      ON c.id = route.connection_id
     AND c.tenant_id = route.tenant_id
     AND c.deleted_at IS NULL
    JOIN ai_provider_data_approval_versions approval
      ON approval.id = route.provider_data_approval_id
     AND approval.tenant_id = route.tenant_id
     AND approval.connection_id = route.connection_id
    JOIN LATERAL (
        SELECT current_approval.id
        FROM ai_provider_data_approval_versions current_approval
        WHERE current_approval.tenant_id = route.tenant_id
          AND current_approval.connection_id = route.connection_id
        ORDER BY current_approval.approval_version DESC
        LIMIT 1
    ) latest_approval ON TRUE
    LEFT JOIN ai_provider_models m
      ON m.id = $3
     AND m.tenant_id = c.tenant_id
     AND m.connection_id = c.id
    WHERE route.tenant_id = $1
      AND route.id = $2
      AND route.deleted_at IS NULL
"#;

const EXECUTION_SEND_STATE_QUERY: &str = r#"
    SELECT
        route.route_set_id,
        route_set.version AS route_version,
        route.connection_id AS route_connection_id,
        route.model_id AS route_model_snapshot_id,
        route.provider_data_approval_id,
        route.requires_tools AS route_requires_tools,
        approval.approval_class AS provider_data_approval_class,
        latest_approval.id AS latest_provider_data_approval_id,
        'external_managed'::TEXT AS execution_environment_class,
        c.provider,
        c.auth_method,
        c.status,
        c.credential_version,
        c.model_catalog_version AS connection_catalog_version,
        m.id AS model_snapshot_id,
        m.provider_model_id AS model_provider_model_id,
        m.credential_version AS model_credential_version,
        m.catalog_version AS snapshot_catalog_version,
        m.supports_tools AS model_supports_tools,
        m.deleted_at AS model_deleted_at
    FROM ai_task_routes route
    JOIN ai_route_sets route_set
      ON route_set.id = route.route_set_id
     AND route_set.tenant_id = route.tenant_id
     AND route_set.deleted_at IS NULL
    JOIN ai_provider_connections c
      ON c.id = route.connection_id
     AND c.tenant_id = route.tenant_id
     AND c.deleted_at IS NULL
    JOIN ai_provider_data_approval_versions approval
      ON approval.id = route.provider_data_approval_id
     AND approval.tenant_id = route.tenant_id
     AND approval.connection_id = route.connection_id
    JOIN LATERAL (
        SELECT current_approval.id
        FROM ai_provider_data_approval_versions current_approval
        WHERE current_approval.tenant_id = route.tenant_id
          AND current_approval.connection_id = route.connection_id
        ORDER BY current_approval.approval_version DESC
        LIMIT 1
    ) latest_approval ON TRUE
    LEFT JOIN ai_provider_models m
      ON m.id = $3
     AND m.tenant_id = c.tenant_id
     AND m.connection_id = c.id
    WHERE route.tenant_id = $1
      AND route.id = $2
      AND route.deleted_at IS NULL
"#;

/// Opaque, authenticated provider request returned after all local preflight work.
///
/// The durable worker may persist and claim its in-flight attempt after this
/// value is returned. The value intentionally implements neither `Clone`,
/// `Debug`, nor either serde trait and is consumed by
/// [`AiProviderOps::send_prepared_execution`].
pub struct PreparedProviderExecution {
    request: PreparedProviderHttpRequest,
    binding: PreparedExecutionBinding,
    telemetry: PreparedExecutionTelemetry,
    input_fingerprint_sha256: [u8; 32],
}

impl PreparedProviderExecution {
    /// Canonical digest of every immutable provider-input and route dimension.
    ///
    /// The canonical plaintext and credential material are never exposed.
    #[must_use]
    pub const fn input_fingerprint_sha256(&self) -> [u8; 32] {
        self.input_fingerprint_sha256
    }
}

/// Exact non-secret state that must still be current at provider dispatch.
///
/// Fields remain private so only successful preparation can construct the
/// proof. The request and this binding are consumed together at send time.
struct PreparedExecutionBinding {
    tenant_id: Uuid,
    route_set_id: Uuid,
    route_version: i64,
    route_target_id: Uuid,
    connection_id: Uuid,
    provider: ProviderKey,
    auth_method: AuthMethod,
    credential_version: i64,
    provider_data_approval_id: Uuid,
    required_provider_data_class: ProviderDataClass,
    model_snapshot_id: Uuid,
    provider_model_id: String,
    model_catalog_version: i64,
    route_requires_tools: bool,
    requires_tools: bool,
}

struct PreparedExecutionTelemetry {
    tenant_id: Uuid,
    connection_id: Uuid,
    credential_version: i64,
    model_snapshot_id: Uuid,
    provider_model_id: String,
}

impl AiProviderOps {
    /// Revalidates, decrypts, encodes, and authenticates one provider request.
    ///
    /// This method performs no outbound HTTP. Once it succeeds, the durable
    /// worker can atomically claim `provider_in_flight` and immediately move the
    /// returned request into [`Self::send_prepared_execution`].
    pub async fn prepare_execution(
        &self,
        tenant_id: Uuid,
        command: ExecuteProviderCommand,
    ) -> Result<PreparedProviderExecution, ProviderExecutionError> {
        let target = command.target();
        let row = sqlx::query_as::<_, ExecutionStateRow>(EXECUTION_STATE_QUERY)
            .bind(tenant_id)
            .bind(target.route_target_id)
            .bind(target.model_snapshot_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|_| ProviderExecutionError::Storage)?
            .ok_or(ProviderExecutionError::ConnectionUnavailable)?;
        self.prepare_execution_from_state(tenant_id, command, row)
    }

    fn prepare_execution_from_state(
        &self,
        tenant_id: Uuid,
        command: ExecuteProviderCommand,
        row: ExecutionStateRow,
    ) -> Result<PreparedProviderExecution, ProviderExecutionError> {
        let target = command.target();
        let validated = ValidatedExecutionState::parse(row, &command)?;
        let encrypted = EncryptedCredential {
            ciphertext: validated
                .credential_ciphertext
                .ok_or(ProviderExecutionError::CredentialUnavailable)?,
            nonce: validated
                .credential_nonce
                .ok_or(ProviderExecutionError::CredentialUnavailable)?,
            key_id: validated
                .credential_key_id
                .ok_or(ProviderExecutionError::CredentialUnavailable)?,
            envelope_version: validated.credential_envelope_version,
        };
        let provider_client = self
            .provider_client
            .as_ref()
            .ok_or(ProviderExecutionError::InvalidConfiguration)?;
        let api_key = self
            .keyring
            .as_ref()
            .ok_or(ProviderExecutionError::CredentialUnavailable)?
            .decrypt(
                credential_context(
                    tenant_id,
                    target.connection_id,
                    validated.provider,
                    validated.auth_method,
                    target.expected_credential_version,
                ),
                &encrypted,
            )
            .map_err(|_| ProviderExecutionError::CredentialUnavailable)?;
        let provider_model_id = command.provider_model_id().to_owned();
        let request = provider_client.prepare_execution(validated.provider, &api_key, command);
        drop(api_key);
        let request = request?;
        // Provider-specific encoding and its existing request-size ceiling have
        // succeeded before the canonical digest is computed.
        let input_fingerprint_sha256 = input_fingerprint(ProviderExecutionFingerprintSource {
            tenant_id,
            provider: validated.provider,
            auth_method: validated.auth_method,
            command: request.command(),
        });
        let required_provider_data_class = request.command().required_provider_data_class();
        let requires_tools = target.requires_tools || !request.command().tools().is_empty();
        Ok(PreparedProviderExecution {
            request,
            binding: PreparedExecutionBinding {
                tenant_id,
                route_set_id: target.route_set_id,
                route_version: target.route_version,
                route_target_id: target.route_target_id,
                connection_id: target.connection_id,
                provider: validated.provider,
                auth_method: validated.auth_method,
                credential_version: target.expected_credential_version,
                provider_data_approval_id: target.provider_data_approval_id,
                required_provider_data_class,
                model_snapshot_id: target.model_snapshot_id,
                provider_model_id: provider_model_id.clone(),
                model_catalog_version: validated.model_catalog_version,
                route_requires_tools: target.requires_tools,
                requires_tools,
            },
            telemetry: PreparedExecutionTelemetry {
                tenant_id,
                connection_id: target.connection_id,
                credential_version: target.expected_credential_version,
                model_snapshot_id: target.model_snapshot_id,
                provider_model_id,
            },
            input_fingerprint_sha256,
        })
    }

    /// Performs the sole outbound HTTP send for one successfully prepared turn.
    ///
    /// Consuming the opaque request prevents accidental replay. A successful
    /// provider result remains successful if best-effort `last_used_at`
    /// telemetry loses a race with credential rotation or model refresh.
    pub async fn send_prepared_execution(
        &self,
        prepared: PreparedProviderExecution,
    ) -> Result<ProviderExecutionResponse, ProviderExecutionError> {
        let row = sqlx::query_as::<_, SendExecutionStateRow>(EXECUTION_SEND_STATE_QUERY)
            .bind(prepared.binding.tenant_id)
            .bind(prepared.binding.route_target_id)
            .bind(prepared.binding.model_snapshot_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|_| ProviderExecutionError::Storage)?
            .ok_or(ProviderExecutionError::ConnectionUnavailable)?;
        self.send_prepared_execution_from_state(prepared, row).await
    }

    async fn send_prepared_execution_from_state(
        &self,
        prepared: PreparedProviderExecution,
        row: SendExecutionStateRow,
    ) -> Result<ProviderExecutionResponse, ProviderExecutionError> {
        let PreparedProviderExecution {
            request,
            binding,
            telemetry,
            input_fingerprint_sha256: _,
        } = prepared;
        ValidatedSendExecutionState::parse(row, &binding)?;

        // Keep the revalidation immediately adjacent to the only outbound
        // dispatch. No secret lookup, encoding, or unrelated work belongs in
        // this final fail-closed window.
        let response = crate::client::ProviderHttpClient::send_prepared(request).await?;

        // This is health telemetry, not the durable run/usage trail. Never turn
        // a successful provider response into failure if currentness changed.
        let _ = mark_last_used_if_still_current(
            &self.pool,
            telemetry.tenant_id,
            telemetry.connection_id,
            telemetry.credential_version,
            telemetry.model_snapshot_id,
            &telemetry.provider_model_id,
        )
        .await;
        Ok(response)
    }
}

#[derive(FromRow)]
struct ExecutionStateRow {
    route_set_id: Uuid,
    route_version: i64,
    route_connection_id: Uuid,
    route_model_snapshot_id: Uuid,
    provider_data_approval_id: Uuid,
    route_requires_tools: bool,
    provider_data_approval_class: String,
    latest_provider_data_approval_id: Uuid,
    execution_environment_class: String,
    provider: String,
    auth_method: String,
    status: String,
    credential_ciphertext: Option<Vec<u8>>,
    credential_nonce: Option<Vec<u8>>,
    credential_key_id: Option<String>,
    credential_envelope_version: i16,
    credential_version: i64,
    connection_catalog_version: i64,
    model_provider_model_id: Option<String>,
    model_credential_version: Option<i64>,
    snapshot_catalog_version: Option<i64>,
    model_context_window_tokens: Option<i64>,
    model_max_output_tokens: Option<i64>,
    model_supports_tools: Option<bool>,
    model_deleted_at: Option<chrono::DateTime<chrono::Utc>>,
}

struct ValidatedExecutionState {
    provider: ProviderKey,
    auth_method: AuthMethod,
    credential_ciphertext: Option<Vec<u8>>,
    credential_nonce: Option<Vec<u8>>,
    credential_key_id: Option<String>,
    credential_envelope_version: i16,
    model_catalog_version: i64,
}

impl ValidatedExecutionState {
    fn parse(
        row: ExecutionStateRow,
        command: &ExecuteProviderCommand,
    ) -> Result<Self, ProviderExecutionError> {
        let target = command.target();
        if row.route_set_id != target.route_set_id
            || row.route_version != target.route_version
            || row.route_connection_id != target.connection_id
            || row.route_model_snapshot_id != target.model_snapshot_id
            || row.route_requires_tools != target.requires_tools
        {
            return Err(ProviderExecutionError::ConnectionUnavailable);
        }
        if row.provider_data_approval_id != target.provider_data_approval_id
            || row.latest_provider_data_approval_id != target.provider_data_approval_id
        {
            return Err(ProviderExecutionError::ProviderDataApprovalChanged);
        }
        let approval = ProviderApprovalClass::from_str(&row.provider_data_approval_class)
            .map_err(|_| ProviderExecutionError::InvalidConfiguration)?;
        let environment =
            ProviderExecutionEnvironmentClass::from_str(&row.execution_environment_class)
                .map_err(|_| ProviderExecutionError::InvalidConfiguration)?;
        evaluate_provider_data_eligibility(
            command.required_provider_data_class(),
            approval,
            environment,
        )
        .map_err(|error| match error {
            ProviderDataEligibilityError::ProviderDataNotApproved => {
                ProviderExecutionError::ProviderDataNotApproved
            }
            ProviderDataEligibilityError::LocalExecutionRequired => {
                ProviderExecutionError::LocalExecutionRequired
            }
        })?;
        if row.status != "ready" {
            return Err(ProviderExecutionError::ConnectionUnavailable);
        }
        if row.credential_version != target.expected_credential_version {
            return Err(ProviderExecutionError::StaleCredential);
        }
        if row.model_provider_model_id.as_deref() != Some(command.provider_model_id())
            || row.model_credential_version != Some(row.credential_version)
            || row.snapshot_catalog_version != Some(row.connection_catalog_version)
            || row.model_deleted_at.is_some()
        {
            return Err(ProviderExecutionError::StaleModel);
        }
        if (target.requires_tools || !command.tools().is_empty())
            && row.model_supports_tools != Some(true)
        {
            return Err(ProviderExecutionError::ToolsUnsupported);
        }
        let context_window_tokens = row
            .model_context_window_tokens
            .filter(|tokens| *tokens > 0)
            .ok_or(ProviderExecutionError::ModelContextUnavailable)?;
        let max_output_tokens = row
            .model_max_output_tokens
            .filter(|tokens| *tokens > 0)
            .ok_or(ProviderExecutionError::ModelOutputUnavailable)?;
        if i64::from(command.max_output_tokens()) > max_output_tokens {
            return Err(ProviderExecutionError::OutputBudgetExceeded);
        }
        let required_tokens = command
            .conservative_input_token_upper_bound()
            .checked_add(u64::from(command.max_output_tokens()))
            .ok_or(ProviderExecutionError::ContextWindowExceeded)?;
        if required_tokens > context_window_tokens as u64 {
            return Err(ProviderExecutionError::ContextWindowExceeded);
        }
        let provider = ProviderKey::from_str(&row.provider)
            .map_err(|_| ProviderExecutionError::InvalidConfiguration)?;
        let auth_method = AuthMethod::from_str(&row.auth_method)
            .map_err(|_| ProviderExecutionError::InvalidConfiguration)?;
        Ok(Self {
            provider,
            auth_method,
            credential_ciphertext: row.credential_ciphertext,
            credential_nonce: row.credential_nonce,
            credential_key_id: row.credential_key_id,
            credential_envelope_version: row.credential_envelope_version,
            model_catalog_version: row.connection_catalog_version,
        })
    }
}

#[derive(FromRow)]
struct SendExecutionStateRow {
    route_set_id: Uuid,
    route_version: i64,
    route_connection_id: Uuid,
    route_model_snapshot_id: Uuid,
    provider_data_approval_id: Uuid,
    route_requires_tools: bool,
    provider_data_approval_class: String,
    latest_provider_data_approval_id: Uuid,
    execution_environment_class: String,
    provider: String,
    auth_method: String,
    status: String,
    credential_version: i64,
    connection_catalog_version: i64,
    model_snapshot_id: Option<Uuid>,
    model_provider_model_id: Option<String>,
    model_credential_version: Option<i64>,
    snapshot_catalog_version: Option<i64>,
    model_supports_tools: Option<bool>,
    model_deleted_at: Option<chrono::DateTime<chrono::Utc>>,
}

struct ValidatedSendExecutionState;

impl ValidatedSendExecutionState {
    fn parse(
        row: SendExecutionStateRow,
        binding: &PreparedExecutionBinding,
    ) -> Result<Self, ProviderExecutionError> {
        if row.route_set_id != binding.route_set_id
            || row.route_version != binding.route_version
            || row.route_connection_id != binding.connection_id
            || row.route_model_snapshot_id != binding.model_snapshot_id
            || row.route_requires_tools != binding.route_requires_tools
        {
            return Err(ProviderExecutionError::ConnectionUnavailable);
        }
        if row.provider_data_approval_id != binding.provider_data_approval_id
            || row.latest_provider_data_approval_id != binding.provider_data_approval_id
        {
            return Err(ProviderExecutionError::ProviderDataApprovalChanged);
        }
        let approval = ProviderApprovalClass::from_str(&row.provider_data_approval_class)
            .map_err(|_| ProviderExecutionError::InvalidConfiguration)?;
        let environment =
            ProviderExecutionEnvironmentClass::from_str(&row.execution_environment_class)
                .map_err(|_| ProviderExecutionError::InvalidConfiguration)?;
        evaluate_provider_data_eligibility(
            binding.required_provider_data_class,
            approval,
            environment,
        )
        .map_err(|error| match error {
            ProviderDataEligibilityError::ProviderDataNotApproved => {
                ProviderExecutionError::ProviderDataNotApproved
            }
            ProviderDataEligibilityError::LocalExecutionRequired => {
                ProviderExecutionError::LocalExecutionRequired
            }
        })?;
        let provider = ProviderKey::from_str(&row.provider)
            .map_err(|_| ProviderExecutionError::InvalidConfiguration)?;
        let auth_method = AuthMethod::from_str(&row.auth_method)
            .map_err(|_| ProviderExecutionError::InvalidConfiguration)?;
        if row.status != "ready"
            || provider != binding.provider
            || auth_method != binding.auth_method
        {
            return Err(ProviderExecutionError::ConnectionUnavailable);
        }
        if row.credential_version != binding.credential_version {
            return Err(ProviderExecutionError::StaleCredential);
        }
        if row.model_snapshot_id != Some(binding.model_snapshot_id)
            || row.model_provider_model_id.as_deref() != Some(binding.provider_model_id.as_str())
            || row.model_credential_version != Some(binding.credential_version)
            || row.snapshot_catalog_version != Some(binding.model_catalog_version)
            || row.connection_catalog_version != binding.model_catalog_version
            || row.model_deleted_at.is_some()
        {
            return Err(ProviderExecutionError::StaleModel);
        }
        if binding.requires_tools && row.model_supports_tools != Some(true) {
            return Err(ProviderExecutionError::ToolsUnsupported);
        }
        Ok(Self)
    }
}

async fn mark_last_used_if_still_current(
    pool: &sqlx::PgPool,
    tenant_id: Uuid,
    connection_id: Uuid,
    credential_version: i64,
    model_snapshot_id: Uuid,
    provider_model_id: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        UPDATE ai_provider_connections c
        SET last_used_at = NOW()
        WHERE c.tenant_id = $1
          AND c.id = $2
          AND c.status = 'ready'
          AND c.credential_version = $3
          AND c.deleted_at IS NULL
          AND EXISTS (
              SELECT 1
              FROM ai_provider_models m
              WHERE m.id = $4
                AND m.tenant_id = c.tenant_id
                AND m.connection_id = c.id
                AND m.provider_model_id = $5
                AND m.credential_version = c.credential_version
                AND m.catalog_version = c.model_catalog_version
                AND m.deleted_at IS NULL
          )
        "#,
    )
    .bind(tenant_id)
    .bind(connection_id)
    .bind(credential_version)
    .bind(model_snapshot_id)
    .bind(provider_model_id)
    .execute(pool)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use base64::{Engine as _, engine::general_purpose::STANDARD};
    use chrono::Utc;
    use cp_common::ProviderDataClass;
    use httpmock::{Method::POST, MockServer};
    use serde_json::json;
    use sqlx::postgres::PgPoolOptions;
    use uuid::Uuid;

    use super::{
        EXECUTION_SEND_STATE_QUERY, EXECUTION_STATE_QUERY, ExecutionStateRow,
        SendExecutionStateRow, ValidatedExecutionState, credential_context,
    };
    use crate::{
        AiProviderOps, CredentialKeyring, ExecuteProviderCommand, ProviderExecutionTarget,
        ProviderMessage, ProviderToolDefinition,
        client::{ProviderEndpoints, ProviderHttpClient},
        types::{ApiKey, AuthMethod, ProviderKey},
    };

    const ROUTE_SET_ID: Uuid = Uuid::from_u128(10);
    const ROUTE_TARGET_ID: Uuid = Uuid::from_u128(11);
    const CONNECTION_ID: Uuid = Uuid::from_u128(12);
    const MODEL_SNAPSHOT_ID: Uuid = Uuid::from_u128(13);
    const PROVIDER_DATA_APPROVAL_ID: Uuid = Uuid::from_u128(14);

    fn command(requires_tools: bool) -> ExecuteProviderCommand {
        command_for(CONNECTION_ID, MODEL_SNAPSHOT_ID, requires_tools)
    }

    fn command_for(
        connection_id: Uuid,
        model_snapshot_id: Uuid,
        requires_tools: bool,
    ) -> ExecuteProviderCommand {
        let tools = requires_tools.then(|| {
            ProviderToolDefinition::parse("lookup", "Look up a record.", json!({"type":"object"}))
                .unwrap()
        });
        ExecuteProviderCommand::parse(
            ProviderExecutionTarget::parse(
                ROUTE_SET_ID,
                1,
                ROUTE_TARGET_ID,
                connection_id,
                4,
                model_snapshot_id,
                PROVIDER_DATA_APPROVAL_ID,
                requires_tools,
            )
            .unwrap(),
            "module_read_reporting",
            "gpt-5",
            "Use records.",
            vec![ProviderMessage::user("Hello").unwrap()],
            tools.into_iter().collect(),
            512,
        )
        .unwrap()
    }

    fn row() -> ExecutionStateRow {
        ExecutionStateRow {
            route_set_id: ROUTE_SET_ID,
            route_version: 1,
            route_connection_id: CONNECTION_ID,
            route_model_snapshot_id: MODEL_SNAPSHOT_ID,
            provider_data_approval_id: PROVIDER_DATA_APPROVAL_ID,
            route_requires_tools: false,
            provider_data_approval_class: "sensitive_data_approved".to_owned(),
            latest_provider_data_approval_id: PROVIDER_DATA_APPROVAL_ID,
            execution_environment_class: "external_managed".to_owned(),
            provider: "openai".to_owned(),
            auth_method: "api_key".to_owned(),
            status: "ready".to_owned(),
            credential_ciphertext: Some(vec![1; 32]),
            credential_nonce: Some(vec![2; 12]),
            credential_key_id: Some("active".to_owned()),
            credential_envelope_version: 1,
            credential_version: 4,
            connection_catalog_version: 3,
            model_provider_model_id: Some("gpt-5".to_owned()),
            model_credential_version: Some(4),
            snapshot_catalog_version: Some(3),
            model_context_window_tokens: Some(128_000),
            model_max_output_tokens: Some(16_384),
            model_supports_tools: Some(true),
            model_deleted_at: None,
        }
    }

    fn keyring() -> CredentialKeyring {
        CredentialKeyring::from_base64(
            BTreeMap::from([("active".to_owned(), STANDARD.encode([7_u8; 32]))]),
            "active",
        )
        .unwrap()
    }

    fn encrypted_row(
        keyring: &CredentialKeyring,
        tenant_id: Uuid,
        connection_id: Uuid,
        model_snapshot_id: Uuid,
    ) -> ExecutionStateRow {
        let encrypted = keyring
            .encrypt(
                credential_context(
                    tenant_id,
                    connection_id,
                    ProviderKey::OpenAi,
                    AuthMethod::ApiKey,
                    4,
                ),
                &ApiKey::parse("secret-key-material-123").unwrap(),
            )
            .unwrap();
        ExecutionStateRow {
            route_connection_id: connection_id,
            route_model_snapshot_id: model_snapshot_id,
            credential_ciphertext: Some(encrypted.ciphertext),
            credential_nonce: Some(encrypted.nonce),
            credential_key_id: Some(encrypted.key_id),
            credential_envelope_version: encrypted.envelope_version,
            ..row()
        }
    }

    fn send_row(connection_id: Uuid, model_snapshot_id: Uuid) -> SendExecutionStateRow {
        SendExecutionStateRow {
            route_set_id: ROUTE_SET_ID,
            route_version: 1,
            route_connection_id: connection_id,
            route_model_snapshot_id: model_snapshot_id,
            provider_data_approval_id: PROVIDER_DATA_APPROVAL_ID,
            route_requires_tools: false,
            provider_data_approval_class: "sensitive_data_approved".to_owned(),
            latest_provider_data_approval_id: PROVIDER_DATA_APPROVAL_ID,
            execution_environment_class: "external_managed".to_owned(),
            provider: "openai".to_owned(),
            auth_method: "api_key".to_owned(),
            status: "ready".to_owned(),
            credential_version: 4,
            connection_catalog_version: 3,
            model_snapshot_id: Some(model_snapshot_id),
            model_provider_model_id: Some("gpt-5".to_owned()),
            model_credential_version: Some(4),
            snapshot_catalog_version: Some(3),
            model_supports_tools: Some(true),
            model_deleted_at: None,
        }
    }

    #[test]
    fn exact_current_ready_state_is_accepted() {
        let mut current = row();
        current.route_requires_tools = true;
        let state = ValidatedExecutionState::parse(current, &command(true)).unwrap();
        assert_eq!(state.provider.as_str(), "openai");
        assert_eq!(state.credential_envelope_version, 1);
        assert!(EXECUTION_STATE_QUERY.contains("m.id = $3"));
        assert!(EXECUTION_STATE_QUERY.contains("m.connection_id = c.id"));
        assert!(EXECUTION_SEND_STATE_QUERY.contains("latest_approval.id"));
        assert!(EXECUTION_SEND_STATE_QUERY.contains("c.credential_version"));
    }

    #[test]
    fn readiness_credential_and_model_drift_fail_closed() {
        let mut unavailable = row();
        unavailable.status = "error".to_owned();
        assert_eq!(
            ValidatedExecutionState::parse(unavailable, &command(false))
                .err()
                .unwrap()
                .code(),
            "provider_connection_unavailable"
        );

        let mut credential = row();
        credential.credential_version = 5;
        assert_eq!(
            ValidatedExecutionState::parse(credential, &command(false))
                .err()
                .unwrap()
                .code(),
            "provider_credential_changed"
        );

        for mut stale in [row(), row(), row(), row()] {
            stale.model_provider_model_id = None;
            assert_eq!(
                ValidatedExecutionState::parse(stale, &command(false))
                    .err()
                    .unwrap()
                    .code(),
                "provider_model_changed"
            );
        }
    }

    #[test]
    fn approval_drift_and_unapproved_data_fail_before_provider_state() {
        let mut stale = row();
        stale.latest_provider_data_approval_id = Uuid::new_v4();
        // A deliberately invalid credential version must not mask approval drift.
        stale.credential_version = -1;
        assert_eq!(
            ValidatedExecutionState::parse(stale, &command(false))
                .err()
                .unwrap()
                .code(),
            "provider_data_approval_changed"
        );

        let mut unapproved = row();
        unapproved.provider_data_approval_class = "unapproved".to_owned();
        unapproved.credential_version = -1;
        assert_eq!(
            ValidatedExecutionState::parse(unapproved, &command(false))
                .err()
                .unwrap()
                .code(),
            "provider_data_not_approved"
        );
    }

    #[test]
    fn every_current_external_provider_rejects_local_only_before_credentials() {
        for provider in ["openai", "anthropic", "openrouter"] {
            let mut external = row();
            external.provider = provider.to_owned();
            external.credential_ciphertext = None;
            external.credential_nonce = None;
            external.credential_key_id = None;
            let local_only =
                command(false).requiring_provider_data_class(ProviderDataClass::LocalOnly);
            assert_eq!(
                ValidatedExecutionState::parse(external, &local_only)
                    .err()
                    .unwrap()
                    .code(),
                "local_execution_required",
                "{provider} must not accept LocalOnly input"
            );
        }
    }

    #[test]
    fn model_identity_catalog_deletion_and_tool_support_are_checked() {
        let mut wrong_id = row();
        wrong_id.model_provider_model_id = Some("gpt-other".to_owned());
        assert_eq!(
            ValidatedExecutionState::parse(wrong_id, &command(false))
                .err()
                .unwrap()
                .code(),
            "provider_model_changed"
        );
        let mut stale_credential = row();
        stale_credential.model_credential_version = Some(3);
        assert_eq!(
            ValidatedExecutionState::parse(stale_credential, &command(false))
                .err()
                .unwrap()
                .code(),
            "provider_model_changed"
        );
        let mut stale_catalog = row();
        stale_catalog.snapshot_catalog_version = Some(2);
        assert_eq!(
            ValidatedExecutionState::parse(stale_catalog, &command(false))
                .err()
                .unwrap()
                .code(),
            "provider_model_changed"
        );
        let mut deleted = row();
        deleted.model_deleted_at = Some(Utc::now());
        assert_eq!(
            ValidatedExecutionState::parse(deleted, &command(false))
                .err()
                .unwrap()
                .code(),
            "provider_model_changed"
        );
        let mut no_tools = row();
        no_tools.route_requires_tools = true;
        no_tools.model_supports_tools = None;
        assert_eq!(
            ValidatedExecutionState::parse(no_tools, &command(true))
                .err()
                .unwrap()
                .code(),
            "provider_tools_unsupported"
        );

        let mut unknown_context = row();
        unknown_context.model_context_window_tokens = None;
        assert_eq!(
            ValidatedExecutionState::parse(unknown_context, &command(false))
                .err()
                .unwrap()
                .code(),
            "provider_model_context_unavailable"
        );
        let mut insufficient_context = row();
        insufficient_context.model_context_window_tokens = Some(1);
        assert_eq!(
            ValidatedExecutionState::parse(insufficient_context, &command(false))
                .err()
                .unwrap()
                .code(),
            "provider_context_window_exceeded"
        );
        let mut unknown_output = row();
        unknown_output.model_max_output_tokens = None;
        assert_eq!(
            ValidatedExecutionState::parse(unknown_output, &command(false))
                .err()
                .unwrap()
                .code(),
            "provider_model_output_unavailable"
        );
        let mut insufficient_output = row();
        insufficient_output.model_max_output_tokens = Some(1);
        assert_eq!(
            ValidatedExecutionState::parse(insufficient_output, &command(false))
                .err()
                .unwrap()
                .code(),
            "provider_output_budget_exceeded"
        );
    }

    #[test]
    fn invalid_persisted_provider_or_auth_is_not_exposed() {
        let mut provider = row();
        provider.provider = "malicious".to_owned();
        assert_eq!(
            ValidatedExecutionState::parse(provider, &command(false))
                .err()
                .unwrap()
                .code(),
            "provider_configuration_invalid"
        );
        let mut auth = row();
        auth.auth_method = "oauth".to_owned();
        assert_eq!(
            ValidatedExecutionState::parse(auth, &command(false))
                .err()
                .unwrap()
                .code(),
            "provider_configuration_invalid"
        );
    }

    #[tokio::test]
    async fn stale_or_undecryptable_preflight_never_sends_outbound_http() {
        let server = MockServer::start_async().await;
        let outbound = server
            .mock_async(|when, then| {
                when.method(POST).path("/openai/chat/completions");
                then.status(200).json_body(json!({
                    "choices":[{"message":{"content":"Unexpected.","tool_calls":[]}}]
                }));
            })
            .await;
        let pool = PgPoolOptions::new()
            .connect_lazy("postgresql://campus-pilot.invalid/campus_pilot")
            .unwrap();
        pool.close().await;
        let tenant_id = Uuid::new_v4();
        let connection_id = Uuid::new_v4();
        let model_snapshot_id = Uuid::new_v4();
        let keyring = keyring();
        let ops = AiProviderOps::new(
            pool,
            Some(keyring.clone()),
            ProviderHttpClient::with_endpoints(ProviderEndpoints::all(&server.base_url())),
        );

        let mut stale = encrypted_row(&keyring, tenant_id, connection_id, model_snapshot_id);
        stale.credential_version = 5;
        let stale_error = ops
            .prepare_execution_from_state(
                tenant_id,
                command_for(connection_id, model_snapshot_id, false),
                stale,
            )
            .err()
            .unwrap();
        assert_eq!(stale_error.code(), "provider_credential_changed");

        let mut undecryptable =
            encrypted_row(&keyring, tenant_id, connection_id, model_snapshot_id);
        undecryptable.credential_ciphertext = Some(vec![0; 32]);
        let credential_error = ops
            .prepare_execution_from_state(
                tenant_id,
                command_for(connection_id, model_snapshot_id, false),
                undecryptable,
            )
            .err()
            .unwrap();
        assert_eq!(credential_error.code(), "provider_credential_unavailable");

        for provider in ["openai", "anthropic", "openrouter"] {
            let mut external = encrypted_row(&keyring, tenant_id, connection_id, model_snapshot_id);
            external.provider = provider.to_owned();
            external.credential_ciphertext = Some(vec![0; 32]);
            let local_only = command_for(connection_id, model_snapshot_id, false)
                .requiring_provider_data_class(ProviderDataClass::LocalOnly);
            let error = ops
                .prepare_execution_from_state(tenant_id, local_only, external)
                .err()
                .unwrap();
            assert_eq!(error.code(), "local_execution_required");
        }
        outbound.assert_hits_async(0).await;
    }

    #[tokio::test]
    async fn prepared_request_is_inert_until_consumed_by_the_send_boundary() {
        let server = MockServer::start_async().await;
        let outbound = server
            .mock_async(|when, then| {
                when.method(POST)
                    .path("/openai/chat/completions")
                    .header("authorization", "Bearer secret-key-material-123");
                then.status(200).json_body(json!({
                    "choices":[{"message":{"content":"Prepared.","tool_calls":[]}}],
                    "usage":null
                }));
            })
            .await;
        let pool = PgPoolOptions::new()
            .connect_lazy("postgresql://campus-pilot.invalid/campus_pilot")
            .unwrap();
        pool.close().await;
        let tenant_id = Uuid::new_v4();
        let connection_id = Uuid::new_v4();
        let model_snapshot_id = Uuid::new_v4();
        let keyring = keyring();
        let ops = AiProviderOps::new(
            pool,
            Some(keyring.clone()),
            ProviderHttpClient::with_endpoints(ProviderEndpoints::all(&server.base_url())),
        );
        let prepared = ops
            .prepare_execution_from_state(
                tenant_id,
                command_for(connection_id, model_snapshot_id, false),
                encrypted_row(&keyring, tenant_id, connection_id, model_snapshot_id),
            )
            .unwrap();
        assert_ne!(prepared.input_fingerprint_sha256(), [0; 32]);
        outbound.assert_hits_async(0).await;

        let response = ops
            .send_prepared_execution_from_state(
                prepared,
                send_row(connection_id, model_snapshot_id),
            )
            .await
            .unwrap();
        assert_eq!(response.assistant_text.as_deref(), Some("Prepared."));
        outbound.assert_hits_async(1).await;
    }

    #[tokio::test]
    async fn approval_revocation_or_credential_rotation_after_prepare_never_sends() {
        let server = MockServer::start_async().await;
        let outbound = server
            .mock_async(|when, then| {
                when.method(POST).path("/openai/chat/completions");
                then.status(200).json_body(json!({
                    "choices":[{"message":{"content":"Unexpected.","tool_calls":[]}}]
                }));
            })
            .await;
        let pool = PgPoolOptions::new()
            .connect_lazy("postgresql://campus-pilot.invalid/campus_pilot")
            .unwrap();
        pool.close().await;
        let tenant_id = Uuid::new_v4();
        let connection_id = Uuid::new_v4();
        let model_snapshot_id = Uuid::new_v4();
        let keyring = keyring();
        let ops = AiProviderOps::new(
            pool,
            Some(keyring.clone()),
            ProviderHttpClient::with_endpoints(ProviderEndpoints::all(&server.base_url())),
        );

        let approval_bound = ops
            .prepare_execution_from_state(
                tenant_id,
                command_for(connection_id, model_snapshot_id, false),
                encrypted_row(&keyring, tenant_id, connection_id, model_snapshot_id),
            )
            .unwrap();
        let mut revoked = send_row(connection_id, model_snapshot_id);
        revoked.latest_provider_data_approval_id = Uuid::new_v4();
        revoked.provider_data_approval_class = "unapproved".to_owned();
        assert_eq!(
            ops.send_prepared_execution_from_state(approval_bound, revoked)
                .await
                .unwrap_err()
                .code(),
            "provider_data_approval_changed"
        );

        let credential_bound = ops
            .prepare_execution_from_state(
                tenant_id,
                command_for(connection_id, model_snapshot_id, false),
                encrypted_row(&keyring, tenant_id, connection_id, model_snapshot_id),
            )
            .unwrap();
        let mut rotated = send_row(connection_id, model_snapshot_id);
        rotated.credential_version = 5;
        assert_eq!(
            ops.send_prepared_execution_from_state(credential_bound, rotated)
                .await
                .unwrap_err()
                .code(),
            "provider_credential_changed"
        );

        outbound.assert_hits_async(0).await;
    }

    #[tokio::test]
    #[ignore = "requires a disposable migrated AI_PROVIDER_EXECUTION_TEST_DATABASE_URL"]
    async fn postgres_execution_revalidates_and_updates_only_safe_health_telemetry() {
        let database_url = std::env::var("AI_PROVIDER_EXECUTION_TEST_DATABASE_URL")
            .expect("AI_PROVIDER_EXECUTION_TEST_DATABASE_URL must target a disposable database");
        let pool = PgPoolOptions::new()
            .max_connections(3)
            .connect(&database_url)
            .await
            .expect("provider execution contract database must connect");
        let tenant_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let connection_id = Uuid::new_v4();
        let model_snapshot_id = Uuid::new_v4();
        let provider_data_approval_id = Uuid::new_v4();
        let route_set_id = Uuid::new_v4();
        let route_target_id = Uuid::new_v4();
        let suffix = tenant_id.simple();
        sqlx::query("INSERT INTO tenants (id, slug, name) VALUES ($1, $2, $3)")
            .bind(tenant_id)
            .bind(format!("provider-execution-{suffix}"))
            .bind("Provider execution contract")
            .execute(&pool)
            .await
            .expect("contract tenant must insert");
        sqlx::query(
            "INSERT INTO users (id, tenant_id, email, password_hash, full_name) VALUES ($1, $2, $3, 'not-a-login', 'Provider Contract')",
        )
        .bind(user_id)
        .bind(tenant_id)
        .bind(format!("provider-execution-{suffix}@example.invalid"))
        .execute(&pool)
        .await
        .expect("contract user must insert");

        let keyring = keyring();
        let api_key = ApiKey::parse("secret-key-material-123").unwrap();
        let encrypted = keyring
            .encrypt(
                credential_context(
                    tenant_id,
                    connection_id,
                    ProviderKey::OpenAi,
                    AuthMethod::ApiKey,
                    4,
                ),
                &api_key,
            )
            .unwrap();
        let mut transaction = pool.begin().await.unwrap();
        sqlx::query(
            r#"
            INSERT INTO ai_provider_connections (
                id, tenant_id, provider, auth_method, account_label, status,
                credential_ciphertext, credential_nonce, credential_key_id,
                credential_envelope_version, credential_version,
                credential_fingerprint, configured_by, model_catalog_version,
                model_catalog_refreshed_at
            )
            VALUES ($1, $2, 'openai', 'api_key', 'Execution contract', 'ready',
                    $3, $4, $5, $6, 4, 'sha256:contract', $7, 3, NOW())
            "#,
        )
        .bind(connection_id)
        .bind(tenant_id)
        .bind(encrypted.ciphertext)
        .bind(encrypted.nonce)
        .bind(encrypted.key_id)
        .bind(encrypted.envelope_version)
        .bind(user_id)
        .execute(&mut *transaction)
        .await
        .expect("contract connection must insert");
        sqlx::query(
            r#"
            INSERT INTO ai_provider_data_approval_versions (
                id, tenant_id, connection_id, approval_version, approval_class,
                change_source, change_reason
            )
            VALUES ($1, $2, $3, 1, 'unapproved', 'system_default',
                    'Initial provider execution contract approval.')
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(tenant_id)
        .bind(connection_id)
        .execute(&mut *transaction)
        .await
        .expect("contract default approval must insert");
        sqlx::query(
            r#"
            INSERT INTO ai_provider_data_approval_versions (
                id, tenant_id, connection_id, approval_version, approval_class,
                change_source, changed_by, change_reason
            )
            VALUES ($1, $2, $3, 2, 'sensitive_data_approved', 'administrator', $4,
                    'Approve sensitive data for provider execution contract.')
            "#,
        )
        .bind(provider_data_approval_id)
        .bind(tenant_id)
        .bind(connection_id)
        .bind(user_id)
        .execute(&mut *transaction)
        .await
        .expect("contract sensitive-data approval must insert");
        sqlx::query(
            r#"
            INSERT INTO ai_provider_models (
                id, tenant_id, connection_id, credential_version, catalog_version,
                provider_model_id, display_name, context_window_tokens,
                max_output_tokens, supports_tools, refreshed_at
            )
            VALUES ($1, $2, $3, 4, 3, 'gpt-5', 'GPT-5', 400000, 128000, TRUE, NOW())
            "#,
        )
        .bind(model_snapshot_id)
        .bind(tenant_id)
        .bind(connection_id)
        .execute(&mut *transaction)
        .await
        .expect("contract model must insert");
        sqlx::query(
            r#"
            INSERT INTO ai_route_sets (
                id, tenant_id, scope_kind, configured_by, change_reason
            )
            VALUES ($1, $2, 'tenant_default', $3, 'Provider execution contract route.')
            "#,
        )
        .bind(route_set_id)
        .bind(tenant_id)
        .bind(user_id)
        .execute(&mut *transaction)
        .await
        .expect("contract route set must insert");
        sqlx::query(
            r#"
            INSERT INTO ai_task_routes (
                id, tenant_id, route_set_id, priority, connection_id, model_id,
                provider_data_approval_id, requires_tools, created_by
            )
            VALUES ($1, $2, $3, 1, $4, $5, $6, FALSE, $7)
            "#,
        )
        .bind(route_target_id)
        .bind(tenant_id)
        .bind(route_set_id)
        .bind(connection_id)
        .bind(model_snapshot_id)
        .bind(provider_data_approval_id)
        .bind(user_id)
        .execute(&mut *transaction)
        .await
        .expect("contract route target must insert");
        transaction.commit().await.unwrap();

        let server = MockServer::start_async().await;
        let completion = server
            .mock_async(|when, then| {
                when.method(POST)
                    .path("/openai/chat/completions")
                    .header("authorization", "Bearer secret-key-material-123");
                then.status(200).json_body(json!({
                    "choices":[{"message":{"content":"Verified.","tool_calls":[]}}],
                    "usage":null
                }));
            })
            .await;
        let ops = AiProviderOps::new(
            pool.clone(),
            Some(keyring),
            ProviderHttpClient::with_endpoints(ProviderEndpoints::all(&server.base_url())),
        );
        let execution_command = || {
            ExecuteProviderCommand::parse(
                ProviderExecutionTarget::parse(
                    route_set_id,
                    1,
                    route_target_id,
                    connection_id,
                    4,
                    model_snapshot_id,
                    provider_data_approval_id,
                    false,
                )
                .unwrap(),
                "module_read_reporting",
                "gpt-5",
                "Use records.",
                vec![ProviderMessage::user("Hello").unwrap()],
                Vec::new(),
                512,
            )
            .unwrap()
        };
        let prepared = ops
            .prepare_execution(tenant_id, execution_command())
            .await
            .unwrap();
        completion.assert_hits_async(0).await;
        let response = ops.send_prepared_execution(prepared).await.unwrap();
        assert_eq!(response.assistant_text.as_deref(), Some("Verified."));
        assert!(
            sqlx::query_scalar::<_, Option<chrono::DateTime<Utc>>>(
                "SELECT last_used_at FROM ai_provider_connections WHERE tenant_id = $1 AND id = $2",
            )
            .bind(tenant_id)
            .bind(connection_id)
            .fetch_one(&pool)
            .await
            .unwrap()
            .is_some()
        );

        sqlx::query(
            "UPDATE ai_provider_connections SET status = 'error' WHERE tenant_id = $1 AND id = $2",
        )
        .bind(tenant_id)
        .bind(connection_id)
        .execute(&pool)
        .await
        .unwrap();
        assert_eq!(
            ops.prepare_execution(tenant_id, execution_command())
                .await
                .err()
                .unwrap()
                .code(),
            "provider_connection_unavailable"
        );
        completion.assert_hits_async(1).await;
    }
}

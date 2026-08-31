//! Exposes secret-free AI-provider Administration reads to the Agent broker.
//!
//! Handlers share the provider domain service used by HTTP and never select or
//! serialize ciphertext, nonces, wrapping-key identifiers, or credentials.
//!
//! Copyright (c) 2026 Codecraft Solutions. All rights reserved.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use cp_agent::{
    AuthorizedCapabilityContext, Capability, CapabilityDescriptor, CapabilityExecutionError,
    CapabilityExecutionErrorCode, CapabilityResource, CapabilityScope, DataSensitivity,
};
use cp_ai_providers::{AiProviderConnection, AiProviderOps, provider_catalog};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

use super::administration::read_descriptor;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct EmptyInput {}

pub(super) struct AiProviderCatalogCapability {
    descriptor: CapabilityDescriptor,
}

impl AiProviderCatalogCapability {
    pub(super) fn new() -> Self {
        Self {
            descriptor: read_descriptor(
                "administration.ai_providers.catalog.list",
                "List supported AI providers",
                "Returns code-owned provider and authentication metadata without credentials.",
                json!({}),
                json!({ "providers": { "type": "array" } }),
                DataSensitivity::General,
                "administration.ai_providers.catalog",
            ),
        }
    }
}

#[async_trait]
impl Capability for AiProviderCatalogCapability {
    type Input = EmptyInput;
    type Output = Value;

    fn descriptor(&self) -> &CapabilityDescriptor {
        &self.descriptor
    }

    fn scope(&self, _input: &Self::Input) -> CapabilityScope {
        CapabilityScope::TenantWide
    }

    async fn execute(
        &self,
        _context: AuthorizedCapabilityContext,
        _input: Self::Input,
    ) -> Result<Self::Output, CapabilityExecutionError> {
        Ok(json!({ "providers": provider_catalog() }))
    }
}

pub(super) struct AiProviderConnectionsListCapability {
    ops: AiProviderOps,
    descriptor: CapabilityDescriptor,
}

impl AiProviderConnectionsListCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            ops: AiProviderOps::for_reads(pool),
            descriptor: read_descriptor(
                "administration.ai_providers.connections.list",
                "List AI provider connections",
                "Returns provider labels, status, test state, and model-cache readiness without credentials.",
                json!({}),
                json!({ "connections": { "type": "array" } }),
                DataSensitivity::Sensitive,
                "administration.ai_providers.connections",
            ),
        }
    }
}

#[async_trait]
impl Capability for AiProviderConnectionsListCapability {
    type Input = EmptyInput;
    type Output = Value;

    fn descriptor(&self) -> &CapabilityDescriptor {
        &self.descriptor
    }

    fn scope(&self, _input: &Self::Input) -> CapabilityScope {
        CapabilityScope::TenantWide
    }

    async fn execute(
        &self,
        context: AuthorizedCapabilityContext,
        _input: Self::Input,
    ) -> Result<Self::Output, CapabilityExecutionError> {
        let connections = self
            .ops
            .list_connections(context.principal().tenant_id())
            .await
            .map_err(|_| dependency_failure("AI provider connections could not be loaded."))?;
        let connections = connections
            .iter()
            .map(AgentConnectionProjection::from)
            .collect::<Vec<_>>();
        Ok(json!({ "connections": connections }))
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ConnectionInput {
    connection_id: Uuid,
}

pub(super) struct AiProviderConnectionReadCapability {
    ops: AiProviderOps,
    descriptor: CapabilityDescriptor,
}

impl AiProviderConnectionReadCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            ops: AiProviderOps::for_reads(pool),
            descriptor: read_descriptor(
                "administration.ai_providers.connections.read",
                "Read an AI provider connection",
                "Returns one secret-free provider connection status record.",
                json!({ "connection_id": { "type": "string", "format": "uuid" } }),
                json!({ "connection": { "type": "object" } }),
                DataSensitivity::Sensitive,
                "administration.ai_providers.connections",
            ),
        }
    }
}

#[async_trait]
impl Capability for AiProviderConnectionReadCapability {
    type Input = ConnectionInput;
    type Output = Value;

    fn descriptor(&self) -> &CapabilityDescriptor {
        &self.descriptor
    }

    fn scope(&self, input: &Self::Input) -> CapabilityScope {
        connection_scope(input.connection_id)
    }

    async fn execute(
        &self,
        context: AuthorizedCapabilityContext,
        input: Self::Input,
    ) -> Result<Self::Output, CapabilityExecutionError> {
        let connection = self
            .ops
            .read_connection(context.principal().tenant_id(), input.connection_id)
            .await
            .map_err(|_| dependency_failure("AI provider connection could not be loaded."))?;
        Ok(json!({
            "connection": AgentConnectionProjection::from(&connection)
        }))
    }
}

pub(super) struct AiProviderModelsListCapability {
    ops: AiProviderOps,
    descriptor: CapabilityDescriptor,
}

impl AiProviderModelsListCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            ops: AiProviderOps::for_reads(pool),
            descriptor: read_descriptor(
                "administration.ai_providers.models.list",
                "List cached provider models",
                "Returns the current immutable model snapshot for one provider connection.",
                json!({ "connection_id": { "type": "string", "format": "uuid" } }),
                json!({ "snapshot": { "type": "object" } }),
                DataSensitivity::Sensitive,
                "administration.ai_providers.models",
            ),
        }
    }
}

#[async_trait]
impl Capability for AiProviderModelsListCapability {
    type Input = ConnectionInput;
    type Output = Value;

    fn descriptor(&self) -> &CapabilityDescriptor {
        &self.descriptor
    }

    fn scope(&self, input: &Self::Input) -> CapabilityScope {
        connection_scope(input.connection_id)
    }

    async fn execute(
        &self,
        context: AuthorizedCapabilityContext,
        input: Self::Input,
    ) -> Result<Self::Output, CapabilityExecutionError> {
        let snapshot = self
            .ops
            .list_models(context.principal().tenant_id(), input.connection_id)
            .await
            .map_err(|_| dependency_failure("AI provider models could not be loaded."))?;
        Ok(json!({ "snapshot": snapshot }))
    }
}

fn connection_scope(connection_id: Uuid) -> CapabilityScope {
    CapabilityScope::resources([CapabilityResource::parse(
        "ai_provider_connection",
        connection_id.to_string(),
    )
    .unwrap_or_else(|_| unreachable!())])
    .unwrap_or_else(|_| unreachable!())
}

fn dependency_failure(message: &str) -> CapabilityExecutionError {
    CapabilityExecutionError::new(CapabilityExecutionErrorCode::DependencyUnavailable, message)
}

/// The intentionally smaller model-visible shape. HTTP administrators may see
/// a credential fingerprint and configuration attribution, but the Agent only
/// needs operational routing health and model-cache readiness.
#[derive(Debug, Serialize)]
struct AgentConnectionProjection<'a> {
    id: Uuid,
    provider: &'a str,
    provider_label: &'a str,
    auth_method: &'a str,
    account_label: &'a str,
    status: &'a str,
    credential_version: i64,
    version: i64,
    last_tested_at: Option<DateTime<Utc>>,
    last_test_status: Option<&'a str>,
    last_failure_category: Option<&'a str>,
    model_count: i64,
    model_catalog_refreshed_at: Option<DateTime<Utc>>,
    provider_data_approval_version: i64,
    provider_data_approval_class: &'a str,
    execution_environment_class: &'a str,
}

impl<'a> From<&'a AiProviderConnection> for AgentConnectionProjection<'a> {
    fn from(connection: &'a AiProviderConnection) -> Self {
        Self {
            id: connection.id,
            provider: &connection.provider,
            provider_label: &connection.provider_label,
            auth_method: &connection.auth_method,
            account_label: &connection.account_label,
            status: &connection.status,
            credential_version: connection.credential_version,
            version: connection.version,
            last_tested_at: connection.last_tested_at,
            last_test_status: connection.last_test_status.as_deref(),
            last_failure_category: connection.last_failure_category.as_deref(),
            model_count: connection.model_count,
            model_catalog_refreshed_at: connection.model_catalog_refreshed_at,
            provider_data_approval_version: connection.provider_data_approval_version,
            provider_data_approval_class: &connection.provider_data_approval_class,
            execution_environment_class: &connection.execution_environment_class,
        }
    }
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;

    #[test]
    fn agent_connection_projection_omits_fingerprint_and_actor_metadata() {
        let timestamp = Utc
            .with_ymd_and_hms(2026, 8, 28, 8, 30, 0)
            .single()
            .unwrap_or_else(|| unreachable!());
        let connection = AiProviderConnection {
            id: Uuid::new_v4(),
            provider: "openai".to_owned(),
            provider_label: "OpenAI".to_owned(),
            auth_method: "api_key".to_owned(),
            account_label: "School account".to_owned(),
            status: "ready".to_owned(),
            credential_fingerprint: "sha256:must-not-be-model-visible".to_owned(),
            credential_version: 2,
            version: 4,
            configured_by_name: "Person Name".to_owned(),
            last_tested_at: Some(timestamp),
            last_test_status: Some("succeeded".to_owned()),
            last_failure_category: None,
            last_used_at: Some(timestamp),
            model_count: 3,
            model_catalog_refreshed_at: Some(timestamp),
            provider_data_approval_id: Uuid::new_v4(),
            provider_data_approval_version: 2,
            provider_data_approval_class: "sensitive_data_approved".to_owned(),
            execution_environment_class: "external_managed".to_owned(),
            created_at: timestamp,
            updated_at: timestamp,
        };

        let value = serde_json::to_value(AgentConnectionProjection::from(&connection))
            .unwrap_or_else(|_| unreachable!());
        let object = value.as_object().unwrap_or_else(|| unreachable!());

        assert_eq!(object.get("status"), Some(&json!("ready")));
        assert_eq!(object.get("model_count"), Some(&json!(3)));
        assert_eq!(
            object.get("provider_data_approval_class"),
            Some(&json!("sensitive_data_approved"))
        );
        for forbidden in [
            "credential_fingerprint",
            "configured_by_name",
            "last_used_at",
            "created_at",
            "updated_at",
        ] {
            assert!(!object.contains_key(forbidden));
        }
    }
}

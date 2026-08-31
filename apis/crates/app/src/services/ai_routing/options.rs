//! Builds secret-free choices for Administration Agent routing workflows.

use cp_ai_providers::{AiProviderOps, ServiceError};
use serde::Serialize;
use uuid::Uuid;

use crate::services::access::catalog::module_catalog;

use super::selectors::RoutingCapabilityOption;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct RoutingTargetOption {
    pub connection_id: Uuid,
    pub provider: String,
    pub provider_label: String,
    pub account_label: String,
    pub provider_model_id: String,
    pub model_display_name: String,
    pub context_window_tokens: Option<i64>,
    pub supports_tools: Option<bool>,
    pub provider_data_approval_class: String,
    pub execution_environment_class: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct RoutingModuleOption {
    pub module_key: String,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct RoutingOptions {
    pub targets: Vec<RoutingTargetOption>,
    pub capabilities: Vec<RoutingCapabilityOption>,
    pub modules: Vec<RoutingModuleOption>,
}

pub(crate) async fn load_routing_options(
    provider_ops: &AiProviderOps,
    tenant_id: Uuid,
    capabilities: Vec<RoutingCapabilityOption>,
) -> Result<RoutingOptions, ServiceError> {
    let connections = provider_ops.list_connections(tenant_id).await?;
    let mut targets = Vec::new();
    for connection in connections.into_iter().filter(|connection| {
        connection.status == "ready" && connection.provider_data_approval_class != "unapproved"
    }) {
        let snapshot = provider_ops.list_models(tenant_id, connection.id).await?;
        targets.extend(
            snapshot
                .models
                .into_iter()
                .map(|model| RoutingTargetOption {
                    connection_id: connection.id,
                    provider: connection.provider.clone(),
                    provider_label: connection.provider_label.clone(),
                    account_label: connection.account_label.clone(),
                    provider_model_id: model.id,
                    model_display_name: model.display_name,
                    context_window_tokens: model.context_window_tokens,
                    supports_tools: model.supports_tools,
                    provider_data_approval_class: connection.provider_data_approval_class.clone(),
                    execution_environment_class: connection.execution_environment_class.clone(),
                }),
        );
    }
    targets.sort_by(|left, right| {
        left.provider_label
            .cmp(&right.provider_label)
            .then_with(|| left.account_label.cmp(&right.account_label))
            .then_with(|| left.model_display_name.cmp(&right.model_display_name))
            .then_with(|| left.provider_model_id.cmp(&right.provider_model_id))
    });

    let modules = module_catalog()
        .into_iter()
        .map(|module| RoutingModuleOption {
            module_key: module.key.to_owned(),
            label: module.label.to_owned(),
        })
        .collect();

    Ok(RoutingOptions {
        targets,
        capabilities,
        modules,
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn target_option_shape_omits_internal_and_credential_fields() {
        let value = serde_json::to_value(RoutingTargetOption {
            connection_id: Uuid::new_v4(),
            provider: "openai".to_owned(),
            provider_label: "OpenAI".to_owned(),
            account_label: "School account".to_owned(),
            provider_model_id: "gpt-5".to_owned(),
            model_display_name: "GPT-5".to_owned(),
            context_window_tokens: Some(128_000),
            supports_tools: Some(true),
            provider_data_approval_class: "sensitive_data_approved".to_owned(),
            execution_environment_class: "external_managed".to_owned(),
        })
        .unwrap_or_else(|_| unreachable!());
        let object = value.as_object().unwrap_or_else(|| unreachable!());

        assert_eq!(object.get("provider_model_id"), Some(&json!("gpt-5")));
        assert_eq!(object.get("supports_tools"), Some(&json!(true)));
        for forbidden in [
            "model_id",
            "credential_fingerprint",
            "credential_version",
            "configured_by_name",
        ] {
            assert!(!object.contains_key(forbidden));
        }
    }
}

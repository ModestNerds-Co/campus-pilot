//! Exposes secret-free Agent routing reads to the capability broker.
//!
//! These handlers use the same routing service as Administration HTTP routes.
//! They reveal route readiness but never provider credentials or raw failures.

use async_trait::async_trait;
use cp_agent::{
    AuthorizedCapabilityContext, Capability, CapabilityDescriptor, CapabilityExecutionError,
    CapabilityExecutionErrorCode, CapabilityResource, CapabilityScope, DataSensitivity,
};
use cp_agent_runtime::{AiRoutingError, AiRoutingOps, ResolveRouteCommand};
use cp_ai_providers::AiProviderOps;
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

use super::administration::read_descriptor;
use crate::services::access::catalog::is_known_module;
use crate::services::ai_routing::options::load_routing_options;
use crate::services::ai_routing::selectors::{
    RoutingCapabilityOption, canonicalize_capability_selectors,
};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct EmptyInput {}

pub(super) struct AiRoutesListCapability {
    ops: AiRoutingOps,
    descriptor: CapabilityDescriptor,
}

impl AiRoutesListCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            ops: AiRoutingOps::for_reads(pool),
            descriptor: read_descriptor(
                "administration.ai_routing.routes.list",
                "List Agent routes",
                "Returns active ordered provider/model routes and their current readiness.",
                json!({}),
                json!({ "routes": { "type": "array" } }),
                DataSensitivity::Sensitive,
                "administration.ai_routing.routes",
            ),
        }
    }
}

#[async_trait]
impl Capability for AiRoutesListCapability {
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
        let routes = self
            .ops
            .list_routes(context.principal().tenant_id())
            .await
            .map_err(|error| routing_failure(error, "Agent routes could not be loaded."))?;
        Ok(json!({ "routes": routes }))
    }
}

pub(super) struct AiRoutingOptionsCapability {
    provider_ops: AiProviderOps,
    descriptor: CapabilityDescriptor,
    capabilities: Vec<RoutingCapabilityOption>,
}

impl AiRoutingOptionsCapability {
    pub(super) fn new(pool: PgPool, capabilities: Vec<RoutingCapabilityOption>) -> Self {
        Self {
            provider_ops: AiProviderOps::for_reads(pool),
            descriptor: read_descriptor(
                "administration.ai_routing.routes.options",
                "List Agent routing options",
                "Returns ready provider/model choices and code-owned routing selectors without credentials.",
                json!({}),
                json!({
                    "targets": { "type": "array" },
                    "capabilities": { "type": "array" },
                    "modules": { "type": "array" }
                }),
                DataSensitivity::Sensitive,
                "administration.ai_routing.options",
            ),
            capabilities,
        }
    }
}

#[async_trait]
impl Capability for AiRoutingOptionsCapability {
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
        let options = load_routing_options(
            &self.provider_ops,
            context.principal().tenant_id(),
            self.capabilities.clone(),
        )
        .await
        .map_err(|_| dependency_failure("Agent routing options could not be loaded."))?;
        serde_json::to_value(options).map_err(|_| {
            CapabilityExecutionError::new(
                CapabilityExecutionErrorCode::Internal,
                "Agent routing options could not be prepared.",
            )
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RouteSetInput {
    route_set_id: Uuid,
}

pub(super) struct AiRouteReadCapability {
    ops: AiRoutingOps,
    descriptor: CapabilityDescriptor,
}

impl AiRouteReadCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            ops: AiRoutingOps::for_reads(pool),
            descriptor: read_descriptor(
                "administration.ai_routing.routes.read",
                "Read an Agent route",
                "Returns one ordered provider/model route and its current readiness.",
                json!({ "route_set_id": { "type": "string", "format": "uuid" } }),
                json!({ "route": { "type": "object" } }),
                DataSensitivity::Sensitive,
                "administration.ai_routing.routes",
            ),
        }
    }
}

#[async_trait]
impl Capability for AiRouteReadCapability {
    type Input = RouteSetInput;
    type Output = Value;

    fn descriptor(&self) -> &CapabilityDescriptor {
        &self.descriptor
    }

    fn scope(&self, input: &Self::Input) -> CapabilityScope {
        route_scope(input.route_set_id)
    }

    async fn execute(
        &self,
        context: AuthorizedCapabilityContext,
        input: Self::Input,
    ) -> Result<Self::Output, CapabilityExecutionError> {
        let route = self
            .ops
            .read_route(context.principal().tenant_id(), input.route_set_id)
            .await
            .map_err(|error| routing_failure(error, "The Agent route could not be loaded."))?;
        Ok(json!({ "route": route }))
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ResolveRouteInput {
    task_class: String,
    module_key: Option<String>,
    operation_class: Option<String>,
    capability_key: Option<String>,
    capability_version: Option<i32>,
    requires_tools: bool,
}

pub(super) struct AiRouteResolveCapability {
    ops: AiRoutingOps,
    descriptor: CapabilityDescriptor,
    capabilities: Vec<RoutingCapabilityOption>,
}

impl AiRouteResolveCapability {
    pub(super) fn new(pool: PgPool, capabilities: Vec<RoutingCapabilityOption>) -> Self {
        Self {
            ops: AiRoutingOps::for_reads(pool),
            descriptor: read_descriptor(
                "administration.ai_routing.routes.resolve",
                "Resolve an Agent route",
                "Explains the highest-precedence usable provider/model route for one task.",
                json!({
                    "task_class": { "type": "string" },
                    "module_key": { "type": "string" },
                    "operation_class": { "type": "string" },
                    "capability_key": { "type": "string" },
                    "capability_version": { "type": "integer" },
                    "requires_tools": { "type": "boolean" }
                }),
                json!({ "resolution": { "type": "object" } }),
                DataSensitivity::Sensitive,
                "administration.ai_routing.resolve",
            ),
            capabilities,
        }
    }

    fn parse_command(
        &self,
        input: ResolveRouteInput,
    ) -> Result<ResolveRouteCommand, CapabilityExecutionError> {
        let mut module_key = input.module_key;
        let mut operation_class = input.operation_class;
        if let (Some(capability_key), Some(capability_version)) =
            (input.capability_key.as_deref(), input.capability_version)
        {
            let (canonical_module, canonical_operation) = canonicalize_capability_selectors(
                &self.capabilities,
                module_key.as_deref(),
                operation_class.as_deref(),
                capability_key,
                capability_version,
            )
            .map_err(|error| invalid_selector(error.safe_message()))?;
            module_key = Some(canonical_module);
            operation_class = Some(canonical_operation);
        } else if module_key
            .as_deref()
            .is_some_and(|module_key| !is_known_module(module_key))
        {
            return Err(invalid_selector("The Agent route module is not available."));
        }

        ResolveRouteCommand::parse(
            &input.task_class,
            module_key.as_deref(),
            operation_class.as_deref(),
            input.capability_key.as_deref(),
            input.capability_version,
            input.requires_tools,
        )
        .map_err(|error| routing_failure(error, "The Agent route request is invalid."))
    }
}

#[async_trait]
impl Capability for AiRouteResolveCapability {
    type Input = ResolveRouteInput;
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
        input: Self::Input,
    ) -> Result<Self::Output, CapabilityExecutionError> {
        let command = self.parse_command(input)?;
        let resolution = self
            .ops
            .resolve_route(context.principal().tenant_id(), command)
            .await
            .map_err(|error| routing_failure(error, "No usable Agent route is available."))?;
        Ok(json!({ "resolution": resolution }))
    }
}

fn invalid_selector(message: &'static str) -> CapabilityExecutionError {
    CapabilityExecutionError::new(CapabilityExecutionErrorCode::InvalidState, message)
}

fn dependency_failure(message: &'static str) -> CapabilityExecutionError {
    CapabilityExecutionError::new(CapabilityExecutionErrorCode::DependencyUnavailable, message)
}

fn route_scope(route_set_id: Uuid) -> CapabilityScope {
    CapabilityScope::resources([CapabilityResource::parse(
        "ai_route_set",
        route_set_id.to_string(),
    )
    .unwrap_or_else(|_| unreachable!())])
    .unwrap_or_else(|_| unreachable!())
}

fn routing_failure(error: AiRoutingError, safe_message: &'static str) -> CapabilityExecutionError {
    let code = match error {
        AiRoutingError::Conflict { .. } => CapabilityExecutionErrorCode::Conflict,
        AiRoutingError::InvalidInput { .. }
        | AiRoutingError::NotFound
        | AiRoutingError::NoMatchingRoute
        | AiRoutingError::UnusableRoute { .. } => CapabilityExecutionErrorCode::InvalidState,
        AiRoutingError::Storage(_) => CapabilityExecutionErrorCode::DependencyUnavailable,
    };
    CapabilityExecutionError::new(code, safe_message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn route_read_scope_is_bound_to_the_requested_route_set() {
        let route_set_id = Uuid::new_v4();
        let scope = route_scope(route_set_id);
        let resource = scope.primary_resource().unwrap_or_else(|| unreachable!());

        assert_eq!(resource.kind(), "ai_route_set");
        assert_eq!(resource.id(), route_set_id.to_string());
    }

    fn finance_capability() -> RoutingCapabilityOption {
        RoutingCapabilityOption {
            capability_key: "finance.journals.list".to_owned(),
            label: "List finance journals".to_owned(),
            module_key: "finance".to_owned(),
            operation_class: "read".to_owned(),
            capability_version: 1,
        }
    }

    fn resolve_input(module_key: Option<&str>, operation_class: Option<&str>) -> ResolveRouteInput {
        ResolveRouteInput {
            task_class: "module_read_reporting".to_owned(),
            module_key: module_key.map(str::to_owned),
            operation_class: operation_class.map(str::to_owned),
            capability_key: Some("finance.journals.list".to_owned()),
            capability_version: Some(1),
            requires_tools: false,
        }
    }

    #[tokio::test]
    async fn typed_resolve_injects_capability_fallback_and_rejects_mismatches() {
        let capability = AiRouteResolveCapability::new(
            PgPool::connect_lazy("postgresql://campus-pilot.invalid/campus_pilot")
                .unwrap_or_else(|_| unreachable!()),
            vec![finance_capability()],
        );

        assert!(capability.parse_command(resolve_input(None, None)).is_ok());
        assert!(
            capability
                .parse_command(resolve_input(Some("sis"), Some("read")))
                .is_err()
        );
        assert!(
            capability
                .parse_command(resolve_input(Some("finance"), Some("mutate")))
                .is_err()
        );
    }

    #[test]
    fn typed_resolve_rejects_unregistered_capability_versions() {
        assert!(
            crate::services::ai_routing::selectors::capability_option(
                &[finance_capability()],
                "finance.journals.list",
                2,
            )
            .is_err()
        );
    }
}

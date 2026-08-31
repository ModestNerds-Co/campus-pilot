//! Exposes licensed, exactly-permissioned Agent routing Administration routes.
//!
//! The HTTP boundary parses route scopes and optimistic commands before any
//! storage work. Responses contain provider/model readiness but no credentials.

use actix_web::{
    HttpResponse, delete, get,
    http::StatusCode,
    post, put,
    web::{self, ServiceConfig},
};
use cp_agent_runtime::{
    AiRouteScope, AiRoutingError, ArchiveRouteCommand, CreateRouteCommand, ReplaceRouteCommand,
    ResolveRouteCommand, RouteTargetDraft,
};
use cp_ai_providers::ServiceError;
use cp_audit::{AuditActor, RequestContext};
use cp_common::{ApiResponse, TenantId};
use serde::Serialize;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::services::access::catalog::is_known_module;
use crate::state::AppState;

use super::dtos::{
    ArchiveRouteRequest, CreateRouteRequest, ReplaceRouteRequest, ResolveRouteRequest,
    RouteTargetRequest,
};
use super::options::load_routing_options;
use super::selectors::{
    SelectorError, canonicalize_capability_selectors, capability_option, routing_capability_options,
};

#[get("/routes")]
async fn list_routes(state: web::Data<AppState>, tenant: web::ReqData<TenantId>) -> HttpResponse {
    respond(
        state
            .ai_routing_ops
            .list_routes(tenant.into_inner().0)
            .await,
        StatusCode::OK,
    )
}

#[get("/routes/options")]
async fn list_route_options(
    state: web::Data<AppState>,
    tenant: web::ReqData<TenantId>,
) -> HttpResponse {
    let capabilities = routing_capability_options(&state.agent_capabilities);
    match load_routing_options(&state.ai_provider_ops, tenant.into_inner().0, capabilities).await {
        Ok(options) => HttpResponse::Ok().json(ApiResponse::from_status(
            StatusCode::OK,
            Some(options),
            None,
        )),
        Err(error) => options_error(error),
    }
}

#[post("/routes/resolve")]
async fn resolve_route(
    state: web::Data<AppState>,
    tenant: web::ReqData<TenantId>,
    body: web::Json<ResolveRouteRequest>,
) -> HttpResponse {
    let request = body.into_inner();
    let command = match resolve_command(request, &state.agent_capabilities) {
        Ok(command) => command,
        Err(error) => return routing_error(error),
    };
    respond(
        state
            .ai_routing_ops
            .resolve_route(tenant.into_inner().0, command)
            .await,
        StatusCode::OK,
    )
}

#[post("/routes")]
async fn create_route(
    state: web::Data<AppState>,
    tenant: web::ReqData<TenantId>,
    actor: web::ReqData<AuditActor>,
    request_context: web::ReqData<RequestContext>,
    body: web::Json<CreateRouteRequest>,
) -> HttpResponse {
    let command = match create_command(body.into_inner(), &state.agent_capabilities) {
        Ok(command) => command,
        Err(error) => return routing_error(error),
    };
    respond(
        state
            .ai_routing_ops
            .create_route(
                tenant.into_inner().0,
                actor.into_inner(),
                request_context.into_inner(),
                command,
            )
            .await,
        StatusCode::CREATED,
    )
}

#[get("/routes/{route_set_id}")]
async fn read_route(
    state: web::Data<AppState>,
    tenant: web::ReqData<TenantId>,
    route_set_id: web::Path<Uuid>,
) -> HttpResponse {
    respond(
        state
            .ai_routing_ops
            .read_route(tenant.into_inner().0, route_set_id.into_inner())
            .await,
        StatusCode::OK,
    )
}

#[put("/routes/{route_set_id}")]
async fn replace_route(
    state: web::Data<AppState>,
    tenant: web::ReqData<TenantId>,
    actor: web::ReqData<AuditActor>,
    request_context: web::ReqData<RequestContext>,
    route_set_id: web::Path<Uuid>,
    body: web::Json<ReplaceRouteRequest>,
) -> HttpResponse {
    let command = match replace_command(body.into_inner()) {
        Ok(command) => command,
        Err(error) => return routing_error(error),
    };
    respond(
        state
            .ai_routing_ops
            .replace_route(
                tenant.into_inner().0,
                route_set_id.into_inner(),
                actor.into_inner(),
                request_context.into_inner(),
                command,
            )
            .await,
        StatusCode::OK,
    )
}

#[delete("/routes/{route_set_id}")]
async fn archive_route(
    state: web::Data<AppState>,
    tenant: web::ReqData<TenantId>,
    actor: web::ReqData<AuditActor>,
    request_context: web::ReqData<RequestContext>,
    route_set_id: web::Path<Uuid>,
    query: web::Query<ArchiveRouteRequest>,
) -> HttpResponse {
    let request = query.into_inner();
    let command = match ArchiveRouteCommand::parse(request.expected_version, request.audit_reason) {
        Ok(command) => command,
        Err(error) => return routing_error(error),
    };
    respond(
        state
            .ai_routing_ops
            .archive_route(
                tenant.into_inner().0,
                route_set_id.into_inner(),
                actor.into_inner(),
                request_context.into_inner(),
                command,
            )
            .await,
        StatusCode::OK,
    )
}

fn create_command(
    request: CreateRouteRequest,
    registry: &cp_agent::CapabilityRegistry,
) -> Result<CreateRouteCommand, AiRoutingError> {
    let scope = parse_scope(
        &request.scope_kind,
        request.task_class.as_deref(),
        request.module_key.as_deref(),
        request.operation_class.as_deref(),
        request.capability_key.as_deref(),
        request.capability_version,
    )?;
    ensure_known_scope(&scope, registry)?;
    CreateRouteCommand::parse(
        scope,
        request.requires_tools,
        parse_targets(request.targets)?,
        request.audit_reason,
    )
}

fn ensure_known_scope(
    scope: &AiRouteScope,
    registry: &cp_agent::CapabilityRegistry,
) -> Result<(), AiRoutingError> {
    match scope {
        AiRouteScope::ModuleOperation { module_key, .. } => ensure_known_module(module_key),
        AiRouteScope::Capability {
            capability_key,
            capability_version,
        } => ensure_registered_capability(registry, capability_key, *capability_version),
        AiRouteScope::TenantDefault | AiRouteScope::TaskClass { .. } => Ok(()),
    }
}

fn ensure_known_module(module_key: &str) -> Result<(), AiRoutingError> {
    if is_known_module(module_key) {
        Ok(())
    } else {
        Err(AiRoutingError::invalid(
            "unknown_module_key",
            "Choose a module from the current Campus Pilot catalogue",
        ))
    }
}

fn ensure_registered_capability(
    registry: &cp_agent::CapabilityRegistry,
    capability_key: &str,
    capability_version: i32,
) -> Result<(), AiRoutingError> {
    capability_option(
        &routing_capability_options(registry),
        capability_key,
        capability_version,
    )
    .map(|_| ())
    .map_err(selector_error)
}

fn resolve_command(
    request: ResolveRouteRequest,
    registry: &cp_agent::CapabilityRegistry,
) -> Result<ResolveRouteCommand, AiRoutingError> {
    let mut module_key = request.module_key;
    let mut operation_class = request.operation_class;
    if let (Some(capability_key), Some(capability_version)) = (
        request.capability_key.as_deref(),
        request.capability_version,
    ) {
        let (canonical_module, canonical_operation) = canonicalize_capability_selectors(
            &routing_capability_options(registry),
            module_key.as_deref(),
            operation_class.as_deref(),
            capability_key,
            capability_version,
        )
        .map_err(selector_error)?;
        module_key = Some(canonical_module);
        operation_class = Some(canonical_operation);
    } else if let Some(module_key) = module_key.as_deref() {
        ensure_known_module(module_key)?;
    }

    let command = ResolveRouteCommand::parse(
        &request.task_class,
        module_key.as_deref(),
        operation_class.as_deref(),
        request.capability_key.as_deref(),
        request.capability_version,
        request.requires_tools,
    )?;
    let Some((capability_key, capability_version)) = request
        .capability_key
        .as_deref()
        .zip(request.capability_version)
    else {
        return Ok(command);
    };
    let descriptor = registry
        .descriptors()
        .into_iter()
        .find(|descriptor| {
            descriptor.key().as_str() == capability_key
                && i32::from(descriptor.version().get()) == capability_version
        })
        .ok_or_else(|| selector_error(SelectorError::UnknownCapability))?;
    Ok(command.requiring_provider_data_class(descriptor.policy().provider_data_class()))
}

fn selector_error(error: SelectorError) -> AiRoutingError {
    AiRoutingError::invalid(error.code(), error.safe_message())
}

fn replace_command(request: ReplaceRouteRequest) -> Result<ReplaceRouteCommand, AiRoutingError> {
    ReplaceRouteCommand::parse(
        request.expected_version,
        request.requires_tools,
        parse_targets(request.targets)?,
        request.audit_reason,
    )
}

fn parse_scope(
    scope_kind: &str,
    task_class: Option<&str>,
    module_key: Option<&str>,
    operation_class: Option<&str>,
    capability_key: Option<&str>,
    capability_version: Option<i32>,
) -> Result<AiRouteScope, AiRoutingError> {
    AiRouteScope::parse(
        scope_kind,
        task_class,
        module_key,
        operation_class,
        capability_key,
        capability_version,
    )
}

fn parse_targets(
    targets: Vec<RouteTargetRequest>,
) -> Result<Vec<RouteTargetDraft>, AiRoutingError> {
    targets
        .into_iter()
        .map(|target| RouteTargetDraft::parse(target.connection_id, target.provider_model_id))
        .collect()
}

fn respond<T: Serialize>(result: Result<T, AiRoutingError>, status: StatusCode) -> HttpResponse {
    match result {
        Ok(data) => {
            HttpResponse::build(status).json(ApiResponse::from_status(status, Some(data), None))
        }
        Err(error) => routing_error(error),
    }
}

fn routing_error(error: AiRoutingError) -> HttpResponse {
    let status = match &error {
        AiRoutingError::InvalidInput { .. } => StatusCode::BAD_REQUEST,
        AiRoutingError::NotFound => StatusCode::NOT_FOUND,
        AiRoutingError::Conflict { .. } => StatusCode::CONFLICT,
        AiRoutingError::NoMatchingRoute | AiRoutingError::UnusableRoute { .. } => {
            StatusCode::UNPROCESSABLE_ENTITY
        }
        AiRoutingError::Storage(_) => StatusCode::INTERNAL_SERVER_ERROR,
    };
    if matches!(error, AiRoutingError::Storage(_)) {
        log::error!("Agent routing persistence failed: {error}");
    }
    let body = ApiResponse::<Value>::from_status(
        status,
        Some(json!({ "code": error.code() })),
        Some(vec![error.safe_message()]),
    );
    HttpResponse::build(status).json(body)
}

fn options_error(error: ServiceError) -> HttpResponse {
    let status = match &error {
        ServiceError::InvalidInput { .. } => StatusCode::BAD_REQUEST,
        ServiceError::NotFound => StatusCode::NOT_FOUND,
        ServiceError::Conflict { .. } => StatusCode::CONFLICT,
        ServiceError::CredentialStorageUnavailable | ServiceError::CredentialUnavailable => {
            StatusCode::SERVICE_UNAVAILABLE
        }
        ServiceError::ProviderFailed(_) => StatusCode::BAD_GATEWAY,
        ServiceError::Storage(_) => StatusCode::INTERNAL_SERVER_ERROR,
    };
    if matches!(error, ServiceError::Storage(_)) {
        log::error!("Agent routing options could not be loaded: {error}");
    }
    HttpResponse::build(status).json(ApiResponse::<Value>::from_status(
        status,
        Some(json!({ "code": error.code() })),
        Some(vec![error.safe_message()]),
    ))
}

pub fn routes(cfg: &mut ServiceConfig) {
    cfg.service(list_routes)
        .service(list_route_options)
        .service(resolve_route)
        .service(create_route)
        .service(read_route)
        .service(replace_route)
        .service(archive_route);
}

#[cfg(test)]
mod tests {
    use actix_web::http::StatusCode;
    use sqlx::postgres::PgPoolOptions;

    use crate::{config::LicenseConfig, services::agent::build_capability_registry};

    use super::{
        AiRoutingError, ResolveRouteRequest, ensure_known_module, ensure_registered_capability,
        resolve_command, routing_error,
    };

    #[test]
    fn stable_routing_failures_map_without_storage_details() {
        assert_eq!(
            routing_error(AiRoutingError::NotFound).status(),
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            routing_error(AiRoutingError::NoMatchingRoute).status(),
            StatusCode::UNPROCESSABLE_ENTITY
        );
    }

    #[tokio::test]
    async fn route_selectors_reject_unknown_modules_capabilities_and_versions() {
        let registry = build_capability_registry(
            PgPoolOptions::new()
                .connect_lazy("postgresql://campus-pilot.invalid/campus_pilot")
                .unwrap_or_else(|_| unreachable!()),
            LicenseConfig {
                trusted_public_keys: Default::default(),
                issuer: "campus-pilot-control-plane".to_owned(),
                audience: "campus-pilot".to_owned(),
                control_plane_url: None,
                credential_key_base64: None,
                installation_name: "Test installation".to_owned(),
            },
        );

        assert!(ensure_known_module("finance").is_ok());
        assert_eq!(
            ensure_known_module("unknown_module")
                .err()
                .unwrap_or_else(|| unreachable!())
                .code(),
            "unknown_module_key"
        );
        assert!(ensure_registered_capability(&registry, "finance.journals.list", 1).is_ok());
        for (key, version) in [
            ("finance.journals.list", 2),
            ("finance.journals.post", 1),
            ("unknown.capability", 1),
        ] {
            assert_eq!(
                ensure_registered_capability(&registry, key, version)
                    .err()
                    .unwrap_or_else(|| unreachable!())
                    .code(),
                "unknown_capability"
            );
        }
    }

    #[tokio::test]
    async fn capability_resolve_rejects_mismatches_and_accepts_capability_only_fallback() {
        let registry = build_capability_registry(
            PgPoolOptions::new()
                .connect_lazy("postgresql://campus-pilot.invalid/campus_pilot")
                .unwrap_or_else(|_| unreachable!()),
            LicenseConfig {
                trusted_public_keys: Default::default(),
                issuer: "campus-pilot-control-plane".to_owned(),
                audience: "campus-pilot".to_owned(),
                control_plane_url: None,
                credential_key_base64: None,
                installation_name: "Test installation".to_owned(),
            },
        );
        let request =
            |module_key: Option<&str>, operation_class: Option<&str>| ResolveRouteRequest {
                task_class: "module_read_reporting".to_owned(),
                module_key: module_key.map(str::to_owned),
                operation_class: operation_class.map(str::to_owned),
                capability_key: Some("finance.journals.list".to_owned()),
                capability_version: Some(1),
                requires_tools: false,
            };

        assert!(resolve_command(request(None, None), &registry).is_ok());
        assert!(matches!(
            resolve_command(request(Some("sis"), Some("read")), &registry),
            Err(AiRoutingError::InvalidInput {
                code: "capability_module_mismatch",
                ..
            })
        ));
        assert!(matches!(
            resolve_command(request(Some("finance"), Some("mutate")), &registry),
            Err(AiRoutingError::InvalidInput {
                code: "capability_operation_mismatch",
                ..
            })
        ));
    }
}

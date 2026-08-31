//! Exposes licensed, exactly-permissioned AI-provider Administration routes.
//!
//! Reads return secret-free views. Credential creation and rotation remain direct
//! human workflows and all writes delegate to the audited shared service.
//!
//! Copyright (c) 2026 Codecraft Solutions. All rights reserved.

use actix_web::{
    HttpResponse, delete, get,
    http::StatusCode,
    post, put,
    web::{self, ServiceConfig},
};
use cp_ai_providers::{
    CreateConnectionCommand, ProviderFailureCategory, RotateCredentialCommand, ServiceError,
    SetProviderDataApprovalCommand, UpdateConnectionCommand, provider_catalog,
};
use cp_audit::{AuditActor, RequestContext};
use cp_common::{ApiResponse, TenantId};
use serde::Serialize;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::state::AppState;

use super::dtos::{
    CreateConnectionRequest, DisconnectQuery, RotateCredentialRequest,
    SetProviderDataApprovalRequest, UpdateConnectionRequest, VersionedActionRequest,
};

#[get("/providers")]
async fn list_providers() -> HttpResponse {
    ok(provider_catalog())
}

#[get("/connections")]
async fn list_connections(
    state: web::Data<AppState>,
    tenant: web::ReqData<TenantId>,
) -> HttpResponse {
    respond(
        state
            .ai_provider_ops
            .list_connections(tenant.into_inner().0)
            .await,
        StatusCode::OK,
    )
}

#[post("/connections")]
async fn create_connection(
    state: web::Data<AppState>,
    tenant: web::ReqData<TenantId>,
    actor: web::ReqData<AuditActor>,
    request_context: web::ReqData<RequestContext>,
    body: web::Json<CreateConnectionRequest>,
) -> HttpResponse {
    let command = match CreateConnectionCommand::parse(
        &body.provider,
        &body.auth_method,
        body.account_label.clone(),
        body.api_key.clone(),
    ) {
        Ok(command) => command,
        Err(error) => return service_error(error),
    };
    respond(
        state
            .ai_provider_ops
            .create_connection(
                tenant.into_inner().0,
                actor.into_inner(),
                request_context.into_inner(),
                command,
            )
            .await,
        StatusCode::CREATED,
    )
}

#[get("/connections/{connection_id}")]
async fn read_connection(
    state: web::Data<AppState>,
    tenant: web::ReqData<TenantId>,
    connection_id: web::Path<Uuid>,
) -> HttpResponse {
    respond(
        state
            .ai_provider_ops
            .read_connection(tenant.into_inner().0, connection_id.into_inner())
            .await,
        StatusCode::OK,
    )
}

#[put("/connections/{connection_id}")]
async fn update_connection(
    state: web::Data<AppState>,
    tenant: web::ReqData<TenantId>,
    actor: web::ReqData<AuditActor>,
    request_context: web::ReqData<RequestContext>,
    connection_id: web::Path<Uuid>,
    body: web::Json<UpdateConnectionRequest>,
) -> HttpResponse {
    let command =
        match UpdateConnectionCommand::parse(body.account_label.clone(), body.expected_version) {
            Ok(command) => command,
            Err(error) => return service_error(error),
        };
    respond(
        state
            .ai_provider_ops
            .update_connection(
                tenant.into_inner().0,
                connection_id.into_inner(),
                actor.into_inner(),
                request_context.into_inner(),
                command,
            )
            .await,
        StatusCode::OK,
    )
}

#[put("/connections/{connection_id}/data-approval")]
async fn set_data_approval(
    state: web::Data<AppState>,
    tenant: web::ReqData<TenantId>,
    actor: web::ReqData<AuditActor>,
    request_context: web::ReqData<RequestContext>,
    connection_id: web::Path<Uuid>,
    body: web::Json<SetProviderDataApprovalRequest>,
) -> HttpResponse {
    let command = match SetProviderDataApprovalCommand::parse(
        &body.approval_class,
        body.expected_approval_version,
        body.change_reason.clone(),
    ) {
        Ok(command) => command,
        Err(error) => return service_error(error),
    };
    respond(
        state
            .ai_provider_ops
            .set_data_approval(
                tenant.into_inner().0,
                connection_id.into_inner(),
                actor.into_inner(),
                request_context.into_inner(),
                command,
            )
            .await,
        StatusCode::OK,
    )
}

#[post("/connections/{connection_id}/credentials/rotate")]
async fn rotate_credential(
    state: web::Data<AppState>,
    tenant: web::ReqData<TenantId>,
    actor: web::ReqData<AuditActor>,
    request_context: web::ReqData<RequestContext>,
    connection_id: web::Path<Uuid>,
    body: web::Json<RotateCredentialRequest>,
) -> HttpResponse {
    let command = match RotateCredentialCommand::parse(body.api_key.clone(), body.expected_version)
    {
        Ok(command) => command,
        Err(error) => return service_error(error),
    };
    respond(
        state
            .ai_provider_ops
            .rotate_credential(
                tenant.into_inner().0,
                connection_id.into_inner(),
                actor.into_inner(),
                request_context.into_inner(),
                command,
            )
            .await,
        StatusCode::OK,
    )
}

#[post("/connections/{connection_id}/test")]
async fn test_connection(
    state: web::Data<AppState>,
    tenant: web::ReqData<TenantId>,
    actor: web::ReqData<AuditActor>,
    request_context: web::ReqData<RequestContext>,
    connection_id: web::Path<Uuid>,
    body: web::Json<VersionedActionRequest>,
) -> HttpResponse {
    respond(
        state
            .ai_provider_ops
            .test_connection(
                tenant.into_inner().0,
                connection_id.into_inner(),
                body.expected_version,
                actor.into_inner(),
                request_context.into_inner(),
            )
            .await,
        StatusCode::OK,
    )
}

#[get("/connections/{connection_id}/models")]
async fn list_models(
    state: web::Data<AppState>,
    tenant: web::ReqData<TenantId>,
    connection_id: web::Path<Uuid>,
) -> HttpResponse {
    respond(
        state
            .ai_provider_ops
            .list_models(tenant.into_inner().0, connection_id.into_inner())
            .await,
        StatusCode::OK,
    )
}

#[post("/connections/{connection_id}/models/refresh")]
async fn refresh_models(
    state: web::Data<AppState>,
    tenant: web::ReqData<TenantId>,
    actor: web::ReqData<AuditActor>,
    request_context: web::ReqData<RequestContext>,
    connection_id: web::Path<Uuid>,
    body: web::Json<VersionedActionRequest>,
) -> HttpResponse {
    respond(
        state
            .ai_provider_ops
            .refresh_models(
                tenant.into_inner().0,
                connection_id.into_inner(),
                body.expected_version,
                actor.into_inner(),
                request_context.into_inner(),
            )
            .await,
        StatusCode::OK,
    )
}

#[delete("/connections/{connection_id}")]
async fn disconnect(
    state: web::Data<AppState>,
    tenant: web::ReqData<TenantId>,
    actor: web::ReqData<AuditActor>,
    request_context: web::ReqData<RequestContext>,
    connection_id: web::Path<Uuid>,
    query: web::Query<DisconnectQuery>,
) -> HttpResponse {
    respond(
        state
            .ai_provider_ops
            .disconnect(
                tenant.into_inner().0,
                connection_id.into_inner(),
                query.expected_version,
                actor.into_inner(),
                request_context.into_inner(),
            )
            .await,
        StatusCode::OK,
    )
}

fn ok<T: Serialize + ?Sized>(data: &T) -> HttpResponse {
    HttpResponse::Ok().json(ApiResponse::from_status(StatusCode::OK, Some(data), None))
}

fn respond<T: Serialize>(result: Result<T, ServiceError>, status: StatusCode) -> HttpResponse {
    match result {
        Ok(data) => {
            HttpResponse::build(status).json(ApiResponse::from_status(status, Some(data), None))
        }
        Err(error) => service_error(error),
    }
}

fn service_error(error: ServiceError) -> HttpResponse {
    let status = match &error {
        ServiceError::InvalidInput { .. } => StatusCode::BAD_REQUEST,
        ServiceError::NotFound => StatusCode::NOT_FOUND,
        ServiceError::Conflict { .. } => StatusCode::CONFLICT,
        ServiceError::CredentialStorageUnavailable | ServiceError::CredentialUnavailable => {
            StatusCode::SERVICE_UNAVAILABLE
        }
        ServiceError::ProviderFailed(category) => match category {
            ProviderFailureCategory::Authentication => StatusCode::UNPROCESSABLE_ENTITY,
            ProviderFailureCategory::RateLimited => StatusCode::TOO_MANY_REQUESTS,
            ProviderFailureCategory::Timeout => StatusCode::GATEWAY_TIMEOUT,
            ProviderFailureCategory::Unavailable
            | ProviderFailureCategory::Network
            | ProviderFailureCategory::InvalidResponse
            | ProviderFailureCategory::Unsupported => StatusCode::BAD_GATEWAY,
        },
        ServiceError::Storage(_) => StatusCode::INTERNAL_SERVER_ERROR,
    };
    if matches!(error, ServiceError::Storage(_)) {
        log::error!("AI provider Administration persistence failed: {}", error);
    }
    let safe_message = error.safe_message();
    let body = ApiResponse::<Value>::from_status(
        status,
        Some(json!({ "code": error.code() })),
        Some(vec![safe_message]),
    );
    HttpResponse::build(status).json(body)
}

pub fn routes(cfg: &mut ServiceConfig) {
    cfg.service(list_providers)
        .service(list_connections)
        .service(create_connection)
        .service(read_connection)
        .service(update_connection)
        .service(set_data_approval)
        .service(rotate_credential)
        .service(test_connection)
        .service(list_models)
        .service(refresh_models)
        .service(disconnect);
}

#[cfg(test)]
mod tests {
    use actix_web::http::StatusCode;
    use cp_ai_providers::{ProviderFailureCategory, ServiceError};

    use super::service_error;

    #[test]
    fn stable_service_failures_map_without_upstream_details() {
        assert_eq!(
            service_error(ServiceError::NotFound).status(),
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            service_error(ServiceError::ProviderFailed(
                ProviderFailureCategory::Authentication
            ))
            .status(),
            StatusCode::UNPROCESSABLE_ENTITY
        );
        assert_eq!(
            service_error(ServiceError::CredentialStorageUnavailable).status(),
            StatusCode::SERVICE_UNAVAILABLE
        );
    }
}

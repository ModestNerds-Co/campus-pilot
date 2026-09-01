//! Authenticated, licensed, permission-authoritative Transport routes.

use actix_web::http::StatusCode;
use actix_web::{HttpResponse, get, post, put, web};
use cp_audit::{AuditActor, RequestContext};
use cp_common::{
    AccessContext, ApiResponse, PaginationMeta, RequirePermission, TenantId,
    flatten_validation_errors,
};
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;
use validator::Validate;

use crate::{
    CancelRunRequest, CreateRiderAssignmentRequest, CreateRouteRequest, CreateRunRequest,
    CreateStopRequest, EndRiderAssignmentRequest, ListRidersQuery, ListRoutesQuery, ListRunsQuery,
    MarkManifestEntryRequest, ReferenceQuery, RemoveStopRequest, RidersPage, RoutesPage,
    RunTransitionRequest, RunsPage, TransportOps, UpdateRouteRequest, UpdateStopRequest,
};

#[get("/references")]
async fn reference_data(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    access: web::ReqData<AccessContext>,
    query: web::Query<ReferenceQuery>,
) -> HttpResponse {
    if !authorised(&access, "transport:view") {
        return forbidden("view Transport reference data");
    }
    value_or_error(TransportOps::reference_data(&pool, tenant_id(tenant), &query).await)
}

#[get("/routes")]
async fn list_routes(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    access: web::ReqData<AccessContext>,
    query: web::Query<ListRoutesQuery>,
) -> HttpResponse {
    if !authorised(&access, "transport:view") {
        return forbidden("view Transport routes");
    }
    let (page, per_page) = bounded_page(query.page, query.per_page);
    match TransportOps::list_routes(&pool, tenant_id(tenant), &query).await {
        Ok((routes, total)) => paginated(RoutesPage { routes }, page, per_page, total),
        Err(error) => operation_error(error),
    }
}

#[post("/routes")]
async fn create_route(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    access: web::ReqData<AccessContext>,
    actor: web::ReqData<AuditActor>,
    context: web::ReqData<RequestContext>,
    body: web::Json<CreateRouteRequest>,
) -> HttpResponse {
    if !authorised(&access, "transport:configure") {
        return forbidden("create Transport routes");
    }
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    created_or_error(
        TransportOps::create_route(
            &pool,
            tenant_id(tenant),
            actor.into_inner(),
            context.into_inner(),
            &body,
        )
        .await,
    )
}

#[get("/routes/{id}")]
async fn read_route(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    access: web::ReqData<AccessContext>,
    path: web::Path<Uuid>,
) -> HttpResponse {
    if !authorised(&access, "transport:view") {
        return forbidden("view Transport routes");
    }
    found(
        TransportOps::get_route(&pool, tenant_id(tenant), path.into_inner()).await,
        "Transport route",
    )
}

#[put("/routes/{id}")]
async fn update_route(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    access: web::ReqData<AccessContext>,
    actor: web::ReqData<AuditActor>,
    context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<UpdateRouteRequest>,
) -> HttpResponse {
    if !authorised(&access, "transport:configure") {
        return forbidden("change Transport routes");
    }
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    updated_or_error(
        TransportOps::update_route(
            &pool,
            tenant_id(tenant),
            path.into_inner(),
            actor.into_inner(),
            context.into_inner(),
            &body,
        )
        .await,
        "Transport route",
    )
}

#[post("/routes/{id}/stops")]
async fn create_stop(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    access: web::ReqData<AccessContext>,
    actor: web::ReqData<AuditActor>,
    context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<CreateStopRequest>,
) -> HttpResponse {
    if !authorised(&access, "transport:configure") {
        return forbidden("create Transport stops");
    }
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    updated_or_error(
        TransportOps::create_stop(
            &pool,
            tenant_id(tenant),
            path.into_inner(),
            actor.into_inner(),
            context.into_inner(),
            &body,
        )
        .await,
        "Transport route",
    )
}

#[put("/routes/{route_id}/stops/{stop_id}")]
async fn update_stop(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    access: web::ReqData<AccessContext>,
    actor: web::ReqData<AuditActor>,
    context: web::ReqData<RequestContext>,
    path: web::Path<(Uuid, Uuid)>,
    body: web::Json<UpdateStopRequest>,
) -> HttpResponse {
    if !authorised(&access, "transport:configure") {
        return forbidden("change Transport stops");
    }
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    let (route_id, stop_id) = path.into_inner();
    updated_or_error(
        TransportOps::update_stop(
            &pool,
            tenant_id(tenant),
            route_id,
            stop_id,
            actor.into_inner(),
            context.into_inner(),
            &body,
        )
        .await,
        "Transport stop",
    )
}

#[post("/routes/{route_id}/stops/{stop_id}/remove")]
async fn remove_stop(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    access: web::ReqData<AccessContext>,
    actor: web::ReqData<AuditActor>,
    context: web::ReqData<RequestContext>,
    path: web::Path<(Uuid, Uuid)>,
    body: web::Json<RemoveStopRequest>,
) -> HttpResponse {
    if !authorised(&access, "transport:configure") {
        return forbidden("remove Transport stops");
    }
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    let (route_id, stop_id) = path.into_inner();
    updated_or_error(
        TransportOps::remove_stop(
            &pool,
            tenant_id(tenant),
            route_id,
            stop_id,
            actor.into_inner(),
            context.into_inner(),
            body.expected_version,
        )
        .await,
        "Transport stop",
    )
}

#[get("/riders")]
async fn list_riders(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    access: web::ReqData<AccessContext>,
    query: web::Query<ListRidersQuery>,
) -> HttpResponse {
    if !authorised(&access, "transport:view") {
        return forbidden("view Transport riders");
    }
    let (page, per_page) = bounded_page(query.page, query.per_page);
    match TransportOps::list_riders(&pool, tenant_id(tenant), &query).await {
        Ok((riders, total)) => paginated(RidersPage { riders }, page, per_page, total),
        Err(error) => operation_error(error),
    }
}

#[post("/riders")]
async fn create_rider(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    access: web::ReqData<AccessContext>,
    actor: web::ReqData<AuditActor>,
    context: web::ReqData<RequestContext>,
    body: web::Json<CreateRiderAssignmentRequest>,
) -> HttpResponse {
    if !authorised(&access, "transport:configure") {
        return forbidden("assign Transport riders");
    }
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    created_or_error(
        TransportOps::create_rider_assignment(
            &pool,
            tenant_id(tenant),
            actor.into_inner(),
            context.into_inner(),
            &body,
        )
        .await,
    )
}

#[post("/riders/{id}/end")]
async fn end_rider(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    access: web::ReqData<AccessContext>,
    actor: web::ReqData<AuditActor>,
    context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<EndRiderAssignmentRequest>,
) -> HttpResponse {
    if !authorised(&access, "transport:configure") {
        return forbidden("end Transport rider assignments");
    }
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    updated_or_error(
        TransportOps::end_rider_assignment(
            &pool,
            tenant_id(tenant),
            path.into_inner(),
            actor.into_inner(),
            context.into_inner(),
            &body,
        )
        .await,
        "Transport rider assignment",
    )
}

#[get("/runs")]
async fn list_runs(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    access: web::ReqData<AccessContext>,
    query: web::Query<ListRunsQuery>,
) -> HttpResponse {
    if !authorised(&access, "transport:view") {
        return forbidden("view Transport runs");
    }
    let (page, per_page) = bounded_page(query.page, query.per_page);
    match TransportOps::list_runs(&pool, tenant_id(tenant), &query).await {
        Ok((runs, total)) => paginated(RunsPage { runs }, page, per_page, total),
        Err(error) => operation_error(error),
    }
}

#[post("/runs")]
async fn create_run(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    access: web::ReqData<AccessContext>,
    actor: web::ReqData<AuditActor>,
    context: web::ReqData<RequestContext>,
    body: web::Json<CreateRunRequest>,
) -> HttpResponse {
    if !authorised(&access, "transport:operate") {
        return forbidden("create Transport runs");
    }
    created_or_error(
        TransportOps::create_run(
            &pool,
            tenant_id(tenant),
            actor.into_inner(),
            context.into_inner(),
            &body,
        )
        .await,
    )
}

#[get("/runs/{id}")]
async fn read_run(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    access: web::ReqData<AccessContext>,
    path: web::Path<Uuid>,
) -> HttpResponse {
    if !authorised(&access, "transport:view") {
        return forbidden("view Transport runs");
    }
    found(
        TransportOps::get_run(&pool, tenant_id(tenant), path.into_inner()).await,
        "Transport run",
    )
}

#[post("/runs/{id}/boarding")]
async fn start_boarding(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    access: web::ReqData<AccessContext>,
    actor: web::ReqData<AuditActor>,
    context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<RunTransitionRequest>,
) -> HttpResponse {
    run_transition(pool, tenant, access, actor, context, path, body, "boarding").await
}

#[post("/runs/{id}/depart")]
async fn depart_run(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    access: web::ReqData<AccessContext>,
    actor: web::ReqData<AuditActor>,
    context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<RunTransitionRequest>,
) -> HttpResponse {
    run_transition(pool, tenant, access, actor, context, path, body, "depart").await
}

#[post("/runs/{id}/complete")]
async fn complete_run(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    access: web::ReqData<AccessContext>,
    actor: web::ReqData<AuditActor>,
    context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<RunTransitionRequest>,
) -> HttpResponse {
    run_transition(pool, tenant, access, actor, context, path, body, "complete").await
}

#[post("/runs/{id}/cancel")]
async fn cancel_run(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    access: web::ReqData<AccessContext>,
    actor: web::ReqData<AuditActor>,
    context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<CancelRunRequest>,
) -> HttpResponse {
    if !authorised(&access, "transport:manage") {
        return forbidden("cancel Transport runs");
    }
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    updated_or_error(
        TransportOps::cancel_run(
            &pool,
            tenant_id(tenant),
            path.into_inner(),
            actor.into_inner(),
            context.into_inner(),
            &body,
        )
        .await,
        "Transport run",
    )
}

#[put("/runs/{run_id}/manifest/{entry_id}")]
async fn mark_manifest(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    access: web::ReqData<AccessContext>,
    actor: web::ReqData<AuditActor>,
    context: web::ReqData<RequestContext>,
    path: web::Path<(Uuid, Uuid)>,
    body: web::Json<MarkManifestEntryRequest>,
) -> HttpResponse {
    if !authorised(&access, "transport:operate") {
        return forbidden("mark Transport manifests");
    }
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    let (run_id, entry_id) = path.into_inner();
    updated_or_error(
        TransportOps::mark_manifest_entry(
            &pool,
            tenant_id(tenant),
            run_id,
            entry_id,
            actor.into_inner(),
            context.into_inner(),
            &body,
        )
        .await,
        "Transport run",
    )
}

#[allow(
    clippy::too_many_arguments,
    reason = "Actix extractors remain explicit"
)]
async fn run_transition(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    access: web::ReqData<AccessContext>,
    actor: web::ReqData<AuditActor>,
    context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<RunTransitionRequest>,
    transition: &str,
) -> HttpResponse {
    if !authorised(&access, "transport:operate") {
        return forbidden("operate Transport runs");
    }
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    let result = match transition {
        "boarding" => {
            TransportOps::start_boarding(
                &pool,
                tenant_id(tenant),
                path.into_inner(),
                actor.into_inner(),
                context.into_inner(),
                &body,
            )
            .await
        }
        "depart" => {
            TransportOps::depart_run(
                &pool,
                tenant_id(tenant),
                path.into_inner(),
                actor.into_inner(),
                context.into_inner(),
                &body,
            )
            .await
        }
        _ => {
            TransportOps::complete_run(
                &pool,
                tenant_id(tenant),
                path.into_inner(),
                actor.into_inner(),
                context.into_inner(),
                &body,
            )
            .await
        }
    };
    updated_or_error(result, "Transport run")
}

pub fn routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("")
            .wrap(RequirePermission::new("transport"))
            .service(reference_data)
            .service(list_routes)
            .service(create_route)
            .service(read_route)
            .service(update_route)
            .service(create_stop)
            .service(update_stop)
            .service(remove_stop)
            .service(list_riders)
            .service(create_rider)
            .service(end_rider)
            .service(list_runs)
            .service(create_run)
            .service(read_run)
            .service(start_boarding)
            .service(depart_run)
            .service(complete_run)
            .service(cancel_run)
            .service(mark_manifest),
    );
}

fn authorised(access: &AccessContext, permission: &str) -> bool {
    access.has_permission("*") || access.has_permission(permission)
}

fn tenant_id(value: web::ReqData<TenantId>) -> Uuid {
    value.into_inner().into_inner()
}

fn bounded_page(page: Option<i64>, per_page: Option<i64>) -> (i64, i64) {
    (
        page.unwrap_or(1).clamp(1, 1_000_000),
        per_page.unwrap_or(20).clamp(1, 100),
    )
}

fn validation_response<T: Validate>(value: &T) -> Option<HttpResponse> {
    value.validate().err().map(|errors| {
        HttpResponse::BadRequest().json(ApiResponse::from_status(
            StatusCode::BAD_REQUEST,
            None::<()>,
            Some(flatten_validation_errors(&errors)),
        ))
    })
}

fn paginated<T: Serialize>(value: T, page: i64, per_page: i64, total: i64) -> HttpResponse {
    HttpResponse::Ok().json(ApiResponse::with_pagination(
        StatusCode::OK,
        Some(value),
        PaginationMeta::new(page as u32, per_page as u32, total),
        None,
    ))
}

fn value_or_error<T: Serialize>(result: anyhow::Result<T>) -> HttpResponse {
    match result {
        Ok(value) => ok(value),
        Err(error) => operation_error(error),
    }
}

fn created_or_error<T: Serialize>(result: anyhow::Result<T>) -> HttpResponse {
    match result {
        Ok(value) => HttpResponse::Created().json(ApiResponse::from_status(
            StatusCode::CREATED,
            Some(value),
            None,
        )),
        Err(error) => operation_error(error),
    }
}

fn updated_or_error<T: Serialize>(result: anyhow::Result<Option<T>>, label: &str) -> HttpResponse {
    match result {
        Ok(Some(value)) => ok(value),
        Ok(None) => not_found(label),
        Err(error) => operation_error(error),
    }
}

fn found<T: Serialize>(result: anyhow::Result<Option<T>>, label: &str) -> HttpResponse {
    updated_or_error(result, label)
}
fn ok<T: Serialize>(value: T) -> HttpResponse {
    HttpResponse::Ok().json(ApiResponse::from_status(StatusCode::OK, Some(value), None))
}
fn forbidden(action: &str) -> HttpResponse {
    HttpResponse::Forbidden().json(ApiResponse::from_status(
        StatusCode::FORBIDDEN,
        None::<()>,
        Some(vec![format!("Your role cannot {action}")]),
    ))
}
fn not_found(label: &str) -> HttpResponse {
    HttpResponse::NotFound().json(ApiResponse::from_status(
        StatusCode::NOT_FOUND,
        None::<()>,
        Some(vec![format!("{label} not found")]),
    ))
}

fn operation_error(error: anyhow::Error) -> HttpResponse {
    let message = error.to_string();
    if message.contains("changed") || message.contains("already") || message.contains("overlapping")
    {
        return HttpResponse::Conflict().json(ApiResponse::from_status(
            StatusCode::CONFLICT,
            None::<()>,
            Some(vec![message]),
        ));
    }
    let operational = [
        "A ",
        "Only ",
        "The ",
        "Both ",
        "Every ",
        "End ",
        "Mark ",
        "Manifest ",
        "Transport ",
    ]
    .iter()
    .any(|prefix| message.starts_with(prefix));
    if operational {
        HttpResponse::BadRequest().json(ApiResponse::from_status(
            StatusCode::BAD_REQUEST,
            None::<()>,
            Some(vec![message]),
        ))
    } else {
        HttpResponse::InternalServerError().json(ApiResponse::from_status(
            StatusCode::INTERNAL_SERVER_ERROR,
            None::<()>,
            Some(vec!["Transport could not complete the request".to_string()]),
        ))
    }
}

#[cfg(test)]
mod tests {
    use actix_web::http::StatusCode;
    use anyhow::anyhow;
    use cp_common::{AccessContext, EntitlementSnapshot, LeaseLifecycle, ModuleEntitlementState};
    use serde_json::json;

    use super::*;

    fn access(permissions: &[&str]) -> AccessContext {
        AccessContext {
            role_keys: vec![],
            permissions: permissions
                .iter()
                .map(|value| (*value).to_string())
                .collect(),
            enabled_modules: vec!["transport".to_string()],
            entitlements: EntitlementSnapshot::new(
                LeaseLifecycle::Active,
                [("transport".to_string(), ModuleEntitlementState::Enabled)],
                [],
            )
            .unwrap_or_else(|_| unreachable!()),
        }
    }

    #[test]
    fn officer_cannot_configure_routes() {
        let officer = access(&["transport:view", "transport:operate"]);
        assert!(authorised(&officer, "transport:operate"));
        assert!(!authorised(&officer, "transport:configure"));
        assert!(!authorised(&officer, "transport:manage"));
        assert!(authorised(&access(&["*"]), "transport:manage"));
    }

    #[test]
    fn response_helpers_preserve_operational_http_statuses() {
        assert_eq!(bounded_page(Some(0), Some(500)), (1, 100));
        assert_eq!(
            validation_response(&CreateRouteRequest {
                code: String::new(),
                name: String::new(),
                direction: crate::RouteDirection::Inbound,
                notes: None,
            })
            .unwrap_or_else(|| unreachable!())
            .status(),
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            paginated(json!({"items": []}), 1, 20, 0).status(),
            StatusCode::OK
        );
        assert_eq!(
            value_or_error::<serde_json::Value>(Ok(json!({"ok": true}))).status(),
            StatusCode::OK
        );
        assert_eq!(
            created_or_error::<serde_json::Value>(Ok(json!({"id": 1}))).status(),
            StatusCode::CREATED
        );
        assert_eq!(
            updated_or_error::<serde_json::Value>(Ok(None), "Transport route").status(),
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            found::<serde_json::Value>(Ok(Some(json!({"id": 1}))), "Transport route").status(),
            StatusCode::OK
        );
        assert_eq!(
            operation_error(anyhow!("The Transport route changed")).status(),
            StatusCode::CONFLICT
        );
        assert_eq!(
            operation_error(anyhow!("Only an active route can be used")).status(),
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            operation_error(anyhow!("database disconnected")).status(),
            StatusCode::INTERNAL_SERVER_ERROR
        );
        assert_eq!(
            forbidden("manage Transport").status(),
            StatusCode::FORBIDDEN
        );
    }
}

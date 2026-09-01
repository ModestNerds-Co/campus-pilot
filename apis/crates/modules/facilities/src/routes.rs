//! Mounts permission-authoritative Facilities HTTP routes.
//!
//! Record scopes are refined into request- and assignment-bound capabilities
//! before domain operations run; the API remains authoritative over the UI.

use actix_web::{HttpResponse, get, http::StatusCode, post, put, web};
use cp_audit::{AuditActor, RequestContext};
use cp_common::{
    AccessContext, ApiResponse, EffectiveRecordScope, PaginationMeta, RecordScopeFamilyKey,
    RecordScopeGrants, RequirePermission, TenantId, flatten_validation_errors,
};
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;
use validator::Validate;

use crate::{
    ArchiveFacilityLocationRequest, CreateFacilityLocationRequest, CreateFacilityServiceRequest,
    CreateFacilityWorkOrderRequest, FacilitiesOps, FacilitiesRequestScope,
    FacilitiesWorkOrderScope, FacilityLocationQuery, FacilityReferenceQuery, FacilityRequestQuery,
    FacilityTransitionRequest, FacilityWorkOrderQuery, FacilityWorkOrderTransitionRequest,
    InspectFacilityWorkOrderRequest, SubmitFacilityCompletionRequest,
    UpdateFacilityLocationRequest,
};

#[get("/locations")]
async fn list_locations(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    query: web::Query<FacilityLocationQuery>,
) -> HttpResponse {
    value_or_error(FacilitiesOps::list_locations(&pool, tenant_id(tenant), &query).await)
}

#[get("/locations/{id}")]
async fn read_location(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    path: web::Path<Uuid>,
) -> HttpResponse {
    found(
        FacilitiesOps::get_location(&pool, tenant_id(tenant), path.into_inner()).await,
        "Facilities location",
    )
}

#[post("/locations")]
async fn create_location(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    access: web::ReqData<AccessContext>,
    actor: web::ReqData<AuditActor>,
    context: web::ReqData<RequestContext>,
    body: web::Json<CreateFacilityLocationRequest>,
) -> HttpResponse {
    if !authorised(&access, "facilities:manage") {
        return forbidden("configure Facilities locations");
    }
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    created_or_error(
        FacilitiesOps::create_location(
            &pool,
            tenant_id(tenant),
            actor.into_inner(),
            context.into_inner(),
            &body,
        )
        .await,
    )
}

#[put("/locations/{id}")]
async fn update_location(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    access: web::ReqData<AccessContext>,
    actor: web::ReqData<AuditActor>,
    context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<UpdateFacilityLocationRequest>,
) -> HttpResponse {
    if !authorised(&access, "facilities:manage") {
        return forbidden("configure Facilities locations");
    }
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    updated_or_error(
        FacilitiesOps::update_location(
            &pool,
            tenant_id(tenant),
            path.into_inner(),
            actor.into_inner(),
            context.into_inner(),
            &body,
        )
        .await,
        "Facilities location",
    )
}

#[post("/locations/{id}/archive")]
async fn archive_location(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    access: web::ReqData<AccessContext>,
    actor: web::ReqData<AuditActor>,
    context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<ArchiveFacilityLocationRequest>,
) -> HttpResponse {
    if !authorised(&access, "facilities:manage") {
        return forbidden("archive Facilities locations");
    }
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    updated_or_error(
        FacilitiesOps::archive_location(
            &pool,
            tenant_id(tenant),
            path.into_inner(),
            actor.into_inner(),
            context.into_inner(),
            &body,
        )
        .await,
        "Facilities location",
    )
}

#[get("/references")]
async fn reference_data(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    access: web::ReqData<AccessContext>,
    query: web::Query<FacilityReferenceQuery>,
) -> HttpResponse {
    if !authorised(&access, "facilities:manage") {
        return forbidden("load Facilities assignment references");
    }
    value_or_error(FacilitiesOps::reference_data(&pool, tenant_id(tenant), &query).await)
}

#[get("/requests")]
async fn list_requests(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    access: web::ReqData<AccessContext>,
    grants: web::ReqData<RecordScopeGrants>,
    actor: web::ReqData<AuditActor>,
    query: web::Query<FacilityRequestQuery>,
) -> HttpResponse {
    let Some(scope) = request_scope(&access, &grants, actor.into_inner()) else {
        return forbidden("read Facilities service requests");
    };
    let (page, per_page) = bounded_page(query.page, query.per_page);
    match FacilitiesOps::list_requests(&pool, tenant_id(tenant), scope, &query).await {
        Ok((requests, total)) => paginated(requests, page, per_page, total),
        Err(error) => operation_error(error),
    }
}

#[post("/requests")]
async fn create_request(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    access: web::ReqData<AccessContext>,
    grants: web::ReqData<RecordScopeGrants>,
    actor: web::ReqData<AuditActor>,
    context: web::ReqData<RequestContext>,
    body: web::Json<CreateFacilityServiceRequest>,
) -> HttpResponse {
    if !authorised(&access, "facilities:request") {
        return forbidden("submit Facilities service requests");
    }
    let Some(scope) = request_scope(&access, &grants, *actor) else {
        return forbidden("submit Facilities service requests");
    };
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    created_or_error(
        FacilitiesOps::create_request(
            &pool,
            tenant_id(tenant),
            scope,
            actor.into_inner(),
            context.into_inner(),
            &body,
        )
        .await,
    )
}

#[get("/requests/{id}")]
async fn read_request(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    access: web::ReqData<AccessContext>,
    grants: web::ReqData<RecordScopeGrants>,
    actor: web::ReqData<AuditActor>,
    path: web::Path<Uuid>,
) -> HttpResponse {
    let Some(scope) = request_scope(&access, &grants, actor.into_inner()) else {
        return forbidden("read Facilities service requests");
    };
    found(
        FacilitiesOps::get_request(&pool, tenant_id(tenant), path.into_inner(), scope).await,
        "Facilities service request",
    )
}

#[post("/requests/{id}/cancel")]
async fn cancel_request(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    access: web::ReqData<AccessContext>,
    actor: web::ReqData<AuditActor>,
    context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<FacilityTransitionRequest>,
) -> HttpResponse {
    let Some(scope) = request_cancellation_scope(&access, *actor) else {
        return forbidden("cancel Facilities service requests");
    };
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    updated_or_error(
        FacilitiesOps::cancel_request(
            &pool,
            tenant_id(tenant),
            path.into_inner(),
            scope,
            actor.into_inner(),
            context.into_inner(),
            &body,
        )
        .await,
        "Facilities service request",
    )
}

#[post("/requests/{id}/close")]
#[expect(
    clippy::too_many_arguments,
    reason = "Actix extractors keep authorization, audit, route, and body inputs explicit"
)]
async fn close_request(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    access: web::ReqData<AccessContext>,
    grants: web::ReqData<RecordScopeGrants>,
    actor: web::ReqData<AuditActor>,
    context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<FacilityTransitionRequest>,
) -> HttpResponse {
    if !authorised(&access, "facilities:manage") {
        return forbidden("close Facilities service requests");
    }
    let Some(scope) = request_scope(&access, &grants, *actor) else {
        return forbidden("close Facilities service requests");
    };
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    updated_or_error(
        FacilitiesOps::close_request(
            &pool,
            tenant_id(tenant),
            path.into_inner(),
            scope,
            actor.into_inner(),
            context.into_inner(),
            &body,
        )
        .await,
        "Facilities service request",
    )
}

#[get("/work-orders")]
async fn list_work_orders(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    access: web::ReqData<AccessContext>,
    grants: web::ReqData<RecordScopeGrants>,
    actor: web::ReqData<AuditActor>,
    query: web::Query<FacilityWorkOrderQuery>,
) -> HttpResponse {
    let Some(scope) = work_order_scope(&access, &grants, actor.into_inner()) else {
        return forbidden("read Facilities work orders");
    };
    let (page, per_page) = bounded_page(query.page, query.per_page);
    match FacilitiesOps::list_work_orders(&pool, tenant_id(tenant), scope, &query).await {
        Ok((orders, total)) => paginated(orders, page, per_page, total),
        Err(error) => operation_error(error),
    }
}

#[post("/work-orders")]
async fn create_work_order(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    access: web::ReqData<AccessContext>,
    grants: web::ReqData<RecordScopeGrants>,
    actor: web::ReqData<AuditActor>,
    context: web::ReqData<RequestContext>,
    body: web::Json<CreateFacilityWorkOrderRequest>,
) -> HttpResponse {
    if !authorised(&access, "facilities:manage") {
        return forbidden("create Facilities work orders");
    }
    let Some(scope) = work_order_scope(&access, &grants, *actor) else {
        return forbidden("create Facilities work orders");
    };
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    created_or_error(
        FacilitiesOps::create_work_order(
            &pool,
            tenant_id(tenant),
            scope,
            actor.into_inner(),
            context.into_inner(),
            &body,
        )
        .await,
    )
}

#[get("/work-orders/{id}")]
async fn read_work_order(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    access: web::ReqData<AccessContext>,
    grants: web::ReqData<RecordScopeGrants>,
    actor: web::ReqData<AuditActor>,
    path: web::Path<Uuid>,
) -> HttpResponse {
    let Some(scope) = work_order_scope(&access, &grants, actor.into_inner()) else {
        return forbidden("read Facilities work orders");
    };
    found(
        FacilitiesOps::get_work_order(&pool, tenant_id(tenant), path.into_inner(), scope).await,
        "Facilities work order",
    )
}

#[post("/work-orders/{id}/start")]
#[expect(
    clippy::too_many_arguments,
    reason = "Actix extractors keep authorization, audit, route, and body inputs explicit"
)]
async fn start_work_order(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    access: web::ReqData<AccessContext>,
    grants: web::ReqData<RecordScopeGrants>,
    actor: web::ReqData<AuditActor>,
    context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<FacilityWorkOrderTransitionRequest>,
) -> HttpResponse {
    if !authorised_any(&access, &["facilities:operate", "facilities:manage"]) {
        return forbidden("start Facilities work orders");
    }
    let Some(scope) = work_order_scope(&access, &grants, *actor) else {
        return forbidden("start Facilities work orders");
    };
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    updated_or_error(
        FacilitiesOps::start_work_order(
            &pool,
            tenant_id(tenant),
            path.into_inner(),
            scope,
            actor.into_inner(),
            context.into_inner(),
            &body,
        )
        .await,
        "Facilities work order",
    )
}

#[post("/work-orders/{id}/submit-completion")]
#[expect(
    clippy::too_many_arguments,
    reason = "Actix extractors keep authorization, audit, route, and body inputs explicit"
)]
async fn submit_completion(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    access: web::ReqData<AccessContext>,
    grants: web::ReqData<RecordScopeGrants>,
    actor: web::ReqData<AuditActor>,
    context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<SubmitFacilityCompletionRequest>,
) -> HttpResponse {
    if !authorised_any(&access, &["facilities:operate", "facilities:manage"]) {
        return forbidden("submit Facilities work-order completion");
    }
    let Some(scope) = work_order_scope(&access, &grants, *actor) else {
        return forbidden("submit Facilities work-order completion");
    };
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    updated_or_error(
        FacilitiesOps::submit_completion(
            &pool,
            tenant_id(tenant),
            path.into_inner(),
            scope,
            actor.into_inner(),
            context.into_inner(),
            &body,
        )
        .await,
        "Facilities work order",
    )
}

#[post("/work-orders/{id}/cancel")]
#[expect(
    clippy::too_many_arguments,
    reason = "Actix extractors keep authorization, audit, route, and body inputs explicit"
)]
async fn cancel_work_order(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    access: web::ReqData<AccessContext>,
    grants: web::ReqData<RecordScopeGrants>,
    actor: web::ReqData<AuditActor>,
    context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<FacilityTransitionRequest>,
) -> HttpResponse {
    if !authorised(&access, "facilities:manage") {
        return forbidden("cancel Facilities work orders");
    }
    let Some(scope) = work_order_scope(&access, &grants, *actor) else {
        return forbidden("cancel Facilities work orders");
    };
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    updated_or_error(
        FacilitiesOps::cancel_work_order(
            &pool,
            tenant_id(tenant),
            path.into_inner(),
            scope,
            actor.into_inner(),
            context.into_inner(),
            &body,
        )
        .await,
        "Facilities work order",
    )
}

#[post("/work-orders/{id}/inspections")]
#[expect(
    clippy::too_many_arguments,
    reason = "Actix extractors keep authorization, audit, route, and body inputs explicit"
)]
async fn inspect_work_order(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    access: web::ReqData<AccessContext>,
    grants: web::ReqData<RecordScopeGrants>,
    actor: web::ReqData<AuditActor>,
    context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<InspectFacilityWorkOrderRequest>,
) -> HttpResponse {
    if !authorised(&access, "facilities:manage") {
        return forbidden("inspect Facilities work orders");
    }
    let Some(scope) = work_order_scope(&access, &grants, *actor) else {
        return forbidden("inspect Facilities work orders");
    };
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    updated_or_error(
        FacilitiesOps::inspect_work_order(
            &pool,
            tenant_id(tenant),
            path.into_inner(),
            scope,
            actor.into_inner(),
            context.into_inner(),
            &body,
        )
        .await,
        "Facilities work order",
    )
}

pub fn routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("")
            .wrap(RequirePermission::new("facilities"))
            .service(list_locations)
            .service(read_location)
            .service(create_location)
            .service(update_location)
            .service(archive_location)
            .service(reference_data)
            .service(list_requests)
            .service(create_request)
            .service(read_request)
            .service(cancel_request)
            .service(close_request)
            .service(list_work_orders)
            .service(create_work_order)
            .service(read_work_order)
            .service(start_work_order)
            .service(submit_completion)
            .service(cancel_work_order)
            .service(inspect_work_order),
    );
}

fn request_scope(
    access: &AccessContext,
    grants: &RecordScopeGrants,
    actor: AuditActor,
) -> Option<FacilitiesRequestScope> {
    if access.has_permission("*") {
        return Some(FacilitiesRequestScope::Campus);
    }
    let family = RecordScopeFamilyKey::parse("facilities.requests").ok()?;
    match grants.effective_scope(&family) {
        Some(EffectiveRecordScope::Campus) => Some(FacilitiesRequestScope::Campus),
        Some(EffectiveRecordScope::SelfRecord | EffectiveRecordScope::SelfAndAssigned) => {
            actor.user_id().map(FacilitiesRequestScope::SelfRecord)
        }
        Some(EffectiveRecordScope::Assigned) | None => None,
    }
}

/// Keeps campus request visibility from becoming cancellation authority.
fn request_cancellation_scope(
    access: &AccessContext,
    actor: AuditActor,
) -> Option<FacilitiesRequestScope> {
    if authorised(access, "facilities:manage") {
        return Some(FacilitiesRequestScope::Campus);
    }
    if authorised(access, "facilities:request") {
        return actor.user_id().map(FacilitiesRequestScope::SelfRecord);
    }
    None
}

fn work_order_scope(
    access: &AccessContext,
    grants: &RecordScopeGrants,
    actor: AuditActor,
) -> Option<FacilitiesWorkOrderScope> {
    if access.has_permission("*") {
        return Some(FacilitiesWorkOrderScope::Campus);
    }
    let family = RecordScopeFamilyKey::parse("facilities.work_orders").ok()?;
    match grants.effective_scope(&family) {
        Some(EffectiveRecordScope::Campus) => Some(FacilitiesWorkOrderScope::Campus),
        Some(EffectiveRecordScope::Assigned | EffectiveRecordScope::SelfAndAssigned) => actor
            .user_id()
            .map(FacilitiesWorkOrderScope::AssignedAccount),
        Some(EffectiveRecordScope::SelfRecord) | None => None,
    }
}

fn authorised(access: &AccessContext, permission: &str) -> bool {
    access.has_permission("*") || access.has_permission(permission)
}

fn authorised_any(access: &AccessContext, permissions: &[&str]) -> bool {
    access.has_permission("*")
        || permissions
            .iter()
            .any(|permission| access.has_permission(permission))
}

fn tenant_id(value: web::ReqData<TenantId>) -> Uuid {
    value.into_inner().into_inner()
}

fn bounded_page(page: Option<i64>, per_page: Option<i64>) -> (i64, i64) {
    (
        page.unwrap_or(1).max(1),
        per_page.unwrap_or(20).clamp(1, 100),
    )
}

fn validation_response<T: Validate>(value: &T) -> Option<HttpResponse> {
    value.validate().err().map(|errors| {
        HttpResponse::BadRequest().json(ApiResponse::<()>::from_status(
            StatusCode::BAD_REQUEST,
            None,
            Some(flatten_validation_errors(&errors)),
        ))
    })
}

fn paginated<T: Serialize>(value: T, page: i64, per_page: i64, total: i64) -> HttpResponse {
    HttpResponse::Ok().json(ApiResponse::with_pagination(
        actix_web::http::StatusCode::OK,
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
        Ok(value) => created(value),
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

fn created<T: Serialize>(value: T) -> HttpResponse {
    HttpResponse::Created().json(ApiResponse::from_status(
        StatusCode::CREATED,
        Some(value),
        None,
    ))
}

fn forbidden(action: &str) -> HttpResponse {
    HttpResponse::Forbidden().json(ApiResponse::<()>::from_status(
        StatusCode::FORBIDDEN,
        None,
        Some(vec![format!(
            "Your current Facilities access does not allow you to {action}"
        )]),
    ))
}

fn not_found(label: &str) -> HttpResponse {
    HttpResponse::NotFound().json(ApiResponse::<()>::from_status(
        StatusCode::NOT_FOUND,
        None,
        Some(vec![format!("{label} not found")]),
    ))
}

fn operation_error(error: anyhow::Error) -> HttpResponse {
    let message = error.to_string();
    if message.contains("changed") || message.contains("already") {
        return HttpResponse::Conflict().json(ApiResponse::<()>::from_status(
            StatusCode::CONFLICT,
            None,
            Some(vec![message]),
        ));
    }
    let operational = [
        "A ",
        "An ",
        "Only ",
        "The selected ",
        "The Facilities ",
        "This Facilities ",
        "Move active ",
        "Cancel the linked ",
        "Facilities ",
        "Location ",
        "The parent ",
        "The facility ",
    ]
    .iter()
    .any(|prefix| message.starts_with(prefix));
    if operational {
        HttpResponse::BadRequest().json(ApiResponse::<()>::from_status(
            StatusCode::BAD_REQUEST,
            None,
            Some(vec![message]),
        ))
    } else {
        HttpResponse::InternalServerError().json(ApiResponse::<()>::from_status(
            StatusCode::INTERNAL_SERVER_ERROR,
            None,
            Some(vec![
                "Facilities could not complete the request".to_string(),
            ]),
        ))
    }
}

#[cfg(test)]
mod tests {
    use cp_audit::AuditActor;
    use cp_common::{
        AccessContext, EntitlementSnapshot, LeaseLifecycle, ModuleEntitlementState,
        RecordScopeFamilyKey, RecordScopeGrant, RecordScopeGrants, RecordScopeKind,
    };
    use uuid::Uuid;

    use super::{request_cancellation_scope, request_scope, work_order_scope};
    use crate::{FacilitiesRequestScope, FacilitiesWorkOrderScope};

    fn access(permissions: &[&str]) -> AccessContext {
        AccessContext {
            role_keys: vec![],
            permissions: permissions
                .iter()
                .map(|value| (*value).to_string())
                .collect(),
            enabled_modules: vec!["facilities".to_string()],
            entitlements: EntitlementSnapshot::new(
                LeaseLifecycle::Active,
                [("facilities".to_string(), ModuleEntitlementState::Enabled)],
                [],
            )
            .unwrap_or_else(|_| unreachable!()),
        }
    }

    fn grants(family: &str, kind: RecordScopeKind) -> RecordScopeGrants {
        RecordScopeGrants::from_grants([RecordScopeGrant::new(
            RecordScopeFamilyKey::parse(family).unwrap_or_else(|_| unreachable!()),
            kind,
        )])
    }

    #[test]
    fn request_self_scope_binds_to_current_person() {
        let user_id = Uuid::new_v4();
        assert_eq!(
            request_scope(
                &access(&["facilities:view"]),
                &grants("facilities.requests", RecordScopeKind::SelfRecord),
                AuditActor::person(user_id),
            ),
            Some(FacilitiesRequestScope::SelfRecord(user_id))
        );
    }

    #[test]
    fn assigned_work_order_scope_binds_to_current_person() {
        let user_id = Uuid::new_v4();
        assert_eq!(
            work_order_scope(
                &access(&["facilities:view"]),
                &grants("facilities.work_orders", RecordScopeKind::Assigned),
                AuditActor::person(user_id),
            ),
            Some(FacilitiesWorkOrderScope::AssignedAccount(user_id))
        );
    }

    #[test]
    fn missing_scope_denies_non_owner_access() {
        assert_eq!(
            request_scope(
                &access(&["facilities:view"]),
                &RecordScopeGrants::empty(),
                AuditActor::person(Uuid::new_v4()),
            ),
            None
        );
    }

    #[test]
    fn officer_cancellation_stays_self_scoped_despite_campus_visibility() {
        let user_id = Uuid::new_v4();
        let officer_access = access(&[
            "facilities:view",
            "facilities:request",
            "facilities:operate",
        ]);
        let campus_grants = grants("facilities.requests", RecordScopeKind::Campus);

        assert_eq!(
            request_scope(&officer_access, &campus_grants, AuditActor::person(user_id)),
            Some(FacilitiesRequestScope::Campus)
        );
        assert_eq!(
            request_cancellation_scope(&officer_access, AuditActor::person(user_id)),
            Some(FacilitiesRequestScope::SelfRecord(user_id))
        );
    }

    #[test]
    fn manager_cancellation_has_campus_scope() {
        assert_eq!(
            request_cancellation_scope(
                &access(&[
                    "facilities:view",
                    "facilities:request",
                    "facilities:operate",
                    "facilities:manage",
                ]),
                AuditActor::person(Uuid::new_v4()),
            ),
            Some(FacilitiesRequestScope::Campus)
        );
    }
}

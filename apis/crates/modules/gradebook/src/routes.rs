//! Authenticated Gradebook HTTP routes over typed module operations.
//!
//! Authentication is mounted by the application. This scope applies the exact
//! Academics operation, including SIS and HR licensing dependencies.

use actix_web::http::StatusCode;
use actix_web::{HttpResponse, delete, get, post, put, web};
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
    CreateMarkSheetRequest, DeleteMarkSheetQuery, GradebookAccessScope, GradebookOps,
    GradebookSheetListQuery, ReopenMarkSheetRequest, TransitionMarkSheetRequest,
    UpdateGradebookMarksRequest,
};

type GradebookRouteAuthority = (
    web::ReqData<AuditActor>,
    web::ReqData<AccessContext>,
    web::ReqData<RecordScopeGrants>,
);

#[get("/references")]
async fn reference_data(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: GradebookRouteAuthority,
) -> HttpResponse {
    let (actor, access, grants) = authority;
    let Ok(scope) = gradebook_access_scope(&access, &grants, actor.into_inner()) else {
        return forbidden();
    };
    match GradebookOps::reference_data(pool.get_ref(), tenant_id(tenant), scope).await {
        Ok(data) => ok(data),
        Err(_) => internal_error(),
    }
}

#[get("/mark-sheets")]
async fn list_mark_sheets(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: GradebookRouteAuthority,
    query: web::Query<GradebookSheetListQuery>,
) -> HttpResponse {
    let (actor, access, grants) = authority;
    let Ok(scope) = gradebook_access_scope(&access, &grants, actor.into_inner()) else {
        return forbidden();
    };
    let (page, per_page) = bounded_page(query.page, query.per_page);
    match GradebookOps::list(pool.get_ref(), tenant_id(tenant), &query.0, scope).await {
        Ok((data, total)) => HttpResponse::Ok().json(ApiResponse::with_pagination(
            StatusCode::OK,
            Some(data),
            PaginationMeta::new(page as u32, per_page as u32, total),
            None,
        )),
        Err(error) => operation_error(error),
    }
}

#[post("/mark-sheets")]
async fn create_mark_sheet(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    actor: web::ReqData<AuditActor>,
    request_context: web::ReqData<RequestContext>,
    authority: (web::ReqData<AccessContext>, web::ReqData<RecordScopeGrants>),
    body: web::Json<CreateMarkSheetRequest>,
) -> HttpResponse {
    let (access, grants) = authority;
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    let tenant_id = tenant_id(tenant);
    let actor = actor.into_inner();
    let Ok(scope) = gradebook_access_scope(&access, &grants, actor) else {
        return forbidden();
    };
    match GradebookOps::can_access_component(
        pool.get_ref(),
        tenant_id,
        body.assessment_component_id,
        scope,
    )
    .await
    {
        Ok(true) => {}
        Ok(false) => return not_found(),
        Err(_) => return internal_error(),
    }
    match GradebookOps::create(
        pool.get_ref(),
        tenant_id,
        actor,
        request_context.into_inner(),
        &body.0,
    )
    .await
    {
        Ok(sheet) => HttpResponse::Created().json(ApiResponse::from_status(
            StatusCode::CREATED,
            Some(sheet),
            None,
        )),
        Err(error) => operation_error(error),
    }
}

#[get("/mark-sheets/{id}")]
async fn read_mark_sheet(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: GradebookRouteAuthority,
    path: web::Path<Uuid>,
) -> HttpResponse {
    let (actor, access, grants) = authority;
    let Ok(scope) = gradebook_access_scope(&access, &grants, actor.into_inner()) else {
        return forbidden();
    };
    match GradebookOps::get(pool.get_ref(), tenant_id(tenant), path.into_inner(), scope).await {
        Ok(Some(sheet)) => ok(sheet),
        Ok(None) => not_found(),
        Err(_) => internal_error(),
    }
}

#[put("/mark-sheets/{id}/marks")]
async fn update_marks(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    actor: web::ReqData<AuditActor>,
    request_context: web::ReqData<RequestContext>,
    authority: (web::ReqData<AccessContext>, web::ReqData<RecordScopeGrants>),
    path: web::Path<Uuid>,
    body: web::Json<UpdateGradebookMarksRequest>,
) -> HttpResponse {
    let (access, grants) = authority;
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    let tenant_id = tenant_id(tenant);
    let actor = actor.into_inner();
    let mark_sheet_id = path.into_inner();
    let Ok(scope) = gradebook_access_scope(&access, &grants, actor) else {
        return forbidden();
    };
    if let Some(response) = scope_mark_sheet(pool.get_ref(), tenant_id, mark_sheet_id, scope).await
    {
        return response;
    }
    match GradebookOps::update_marks(
        pool.get_ref(),
        tenant_id,
        mark_sheet_id,
        actor,
        request_context.into_inner(),
        &body.0,
    )
    .await
    {
        Ok(Some(sheet)) => ok(sheet),
        Ok(None) => not_found(),
        Err(error) => operation_error(error),
    }
}

#[post("/mark-sheets/{id}/submit")]
async fn submit_mark_sheet(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    actor: web::ReqData<AuditActor>,
    request_context: web::ReqData<RequestContext>,
    authority: (web::ReqData<AccessContext>, web::ReqData<RecordScopeGrants>),
    path: web::Path<Uuid>,
    body: web::Json<TransitionMarkSheetRequest>,
) -> HttpResponse {
    let (access, grants) = authority;
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    let tenant_id = tenant_id(tenant);
    let actor = actor.into_inner();
    let mark_sheet_id = path.into_inner();
    let Ok(scope) = gradebook_access_scope(&access, &grants, actor) else {
        return forbidden();
    };
    if let Some(response) = scope_mark_sheet(pool.get_ref(), tenant_id, mark_sheet_id, scope).await
    {
        return response;
    }
    match GradebookOps::submit(
        pool.get_ref(),
        tenant_id,
        mark_sheet_id,
        actor,
        request_context.into_inner(),
        body.expected_version,
    )
    .await
    {
        Ok(Some(sheet)) => ok(sheet),
        Ok(None) => not_found(),
        Err(error) => operation_error(error),
    }
}

#[post("/mark-sheets/{id}/publish")]
async fn publish_mark_sheet(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    actor: web::ReqData<AuditActor>,
    request_context: web::ReqData<RequestContext>,
    authority: (web::ReqData<AccessContext>, web::ReqData<RecordScopeGrants>),
    path: web::Path<Uuid>,
    body: web::Json<TransitionMarkSheetRequest>,
) -> HttpResponse {
    let (access, grants) = authority;
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    let tenant_id = tenant_id(tenant);
    let actor = actor.into_inner();
    let mark_sheet_id = path.into_inner();
    let Ok(scope) = gradebook_access_scope(&access, &grants, actor) else {
        return forbidden();
    };
    if let Some(response) = scope_mark_sheet(pool.get_ref(), tenant_id, mark_sheet_id, scope).await
    {
        return response;
    }
    match GradebookOps::publish(
        pool.get_ref(),
        tenant_id,
        mark_sheet_id,
        actor,
        request_context.into_inner(),
        body.expected_version,
    )
    .await
    {
        Ok(Some(sheet)) => ok(sheet),
        Ok(None) => not_found(),
        Err(error) => operation_error(error),
    }
}

#[post("/mark-sheets/{id}/reopen")]
async fn reopen_mark_sheet(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    actor: web::ReqData<AuditActor>,
    request_context: web::ReqData<RequestContext>,
    authority: (web::ReqData<AccessContext>, web::ReqData<RecordScopeGrants>),
    path: web::Path<Uuid>,
    body: web::Json<ReopenMarkSheetRequest>,
) -> HttpResponse {
    let (access, grants) = authority;
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    let tenant_id = tenant_id(tenant);
    let actor = actor.into_inner();
    let mark_sheet_id = path.into_inner();
    let Ok(scope) = gradebook_access_scope(&access, &grants, actor) else {
        return forbidden();
    };
    if let Some(response) = scope_mark_sheet(pool.get_ref(), tenant_id, mark_sheet_id, scope).await
    {
        return response;
    }
    match GradebookOps::reopen(
        pool.get_ref(),
        tenant_id,
        mark_sheet_id,
        actor,
        request_context.into_inner(),
        &body.0,
    )
    .await
    {
        Ok(Some(sheet)) => ok(sheet),
        Ok(None) => not_found(),
        Err(error) => operation_error(error),
    }
}

#[delete("/mark-sheets/{id}")]
async fn delete_mark_sheet(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    actor: web::ReqData<AuditActor>,
    request_context: web::ReqData<RequestContext>,
    authority: (web::ReqData<AccessContext>, web::ReqData<RecordScopeGrants>),
    path: web::Path<Uuid>,
    query: web::Query<DeleteMarkSheetQuery>,
) -> HttpResponse {
    let (access, grants) = authority;
    if let Some(response) = validation_response(&query.0) {
        return response;
    }
    let tenant_id = tenant_id(tenant);
    let actor = actor.into_inner();
    let mark_sheet_id = path.into_inner();
    let Ok(scope) = gradebook_access_scope(&access, &grants, actor) else {
        return forbidden();
    };
    if let Some(response) = scope_mark_sheet(pool.get_ref(), tenant_id, mark_sheet_id, scope).await
    {
        return response;
    }
    match GradebookOps::delete(
        pool.get_ref(),
        tenant_id,
        mark_sheet_id,
        actor,
        request_context.into_inner(),
        query.expected_version,
    )
    .await
    {
        Ok(true) => ok(serde_json::json!({ "deleted": true })),
        Ok(false) => not_found(),
        Err(error) => operation_error(error),
    }
}

pub fn routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("")
            .wrap(RequirePermission::new("academics"))
            .service(reference_data)
            .service(list_mark_sheets)
            .service(create_mark_sheet)
            .service(read_mark_sheet)
            .service(update_marks)
            .service(submit_mark_sheet)
            .service(publish_mark_sheet)
            .service(reopen_mark_sheet)
            .service(delete_mark_sheet),
    );
}

fn tenant_id(tenant: web::ReqData<TenantId>) -> Uuid {
    tenant.into_inner().into_inner()
}

fn gradebook_access_scope(
    access: &AccessContext,
    grants: &RecordScopeGrants,
    actor: AuditActor,
) -> Result<GradebookAccessScope, ()> {
    if access.has_permission("*") {
        return Ok(GradebookAccessScope::Campus);
    }
    let family = RecordScopeFamilyKey::parse("academics.gradebook").map_err(|_| ())?;
    match grants.effective_scope(&family) {
        Some(EffectiveRecordScope::Campus) => Ok(GradebookAccessScope::Campus),
        Some(EffectiveRecordScope::Assigned | EffectiveRecordScope::SelfAndAssigned) => actor
            .user_id()
            .map(GradebookAccessScope::AssignedTo)
            .ok_or(()),
        Some(EffectiveRecordScope::SelfRecord) | None => Err(()),
    }
}

async fn scope_mark_sheet(
    pool: &PgPool,
    tenant_id: Uuid,
    mark_sheet_id: Uuid,
    scope: GradebookAccessScope,
) -> Option<HttpResponse> {
    match GradebookOps::can_access_mark_sheet(pool, tenant_id, mark_sheet_id, scope).await {
        Ok(true) => None,
        Ok(false) => Some(not_found()),
        Err(_) => Some(internal_error()),
    }
}

fn ok<T: Serialize>(value: T) -> HttpResponse {
    HttpResponse::Ok().json(ApiResponse::from_status(StatusCode::OK, Some(value), None))
}

fn not_found() -> HttpResponse {
    HttpResponse::NotFound().json(ApiResponse::from_status(
        StatusCode::NOT_FOUND,
        None::<()>,
        Some(vec!["Assessment mark sheet not found".to_string()]),
    ))
}

fn forbidden() -> HttpResponse {
    HttpResponse::Forbidden().json(ApiResponse::from_status(
        StatusCode::FORBIDDEN,
        None::<()>,
        Some(vec!["Gradebook record scope is unavailable".to_string()]),
    ))
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

fn operation_error(error: anyhow::Error) -> HttpResponse {
    let diagnostic = format!("{error:#}");
    let message = error.to_string();
    if message.contains("changed")
        || message.contains("already exists")
        || message.contains("already used")
        || message.contains("already been processed")
    {
        return HttpResponse::Conflict().json(ApiResponse::from_status(
            StatusCode::CONFLICT,
            None::<()>,
            Some(vec![message]),
        ));
    }
    let operational = message.starts_with("The ")
        || message.starts_with("This ")
        || message.starts_with("Only ")
        || message.starts_with("Mark ")
        || message.starts_with("An ")
        || message.starts_with("A ")
        || message.starts_with("Each ")
        || message.starts_with("Submitted ");
    if operational {
        HttpResponse::BadRequest().json(ApiResponse::from_status(
            StatusCode::BAD_REQUEST,
            None::<()>,
            Some(vec![message]),
        ))
    } else {
        log::error!("Gradebook operation failed: {diagnostic}");
        internal_error()
    }
}

fn internal_error() -> HttpResponse {
    HttpResponse::InternalServerError().json(ApiResponse::from_status(
        StatusCode::INTERNAL_SERVER_ERROR,
        None::<()>,
        Some(vec![
            "Gradebook could not complete the request.".to_string(),
        ]),
    ))
}

fn bounded_page(page: Option<i64>, per_page: Option<i64>) -> (i64, i64) {
    (
        page.unwrap_or(1).clamp(1, 1_000_000),
        per_page.unwrap_or(25).clamp(1, 100),
    )
}

#[cfg(test)]
mod tests {
    use cp_audit::AuditActor;
    use cp_common::{
        AccessContext, EntitlementSnapshot, LeaseLifecycle, RecordScopeFamilyKey, RecordScopeGrant,
        RecordScopeGrants, RecordScopeKind,
    };
    use uuid::Uuid;

    use super::{GradebookAccessScope, gradebook_access_scope};

    fn access(permissions: &[&str]) -> AccessContext {
        AccessContext {
            role_keys: Vec::new(),
            permissions: permissions
                .iter()
                .map(|value| (*value).to_string())
                .collect(),
            enabled_modules: Vec::new(),
            entitlements: EntitlementSnapshot::new(
                LeaseLifecycle::Active,
                Vec::<(String, cp_common::ModuleEntitlementState)>::new(),
                Vec::<String>::new(),
            )
            .unwrap_or_else(|error| panic!("test entitlement must be valid: {error}")),
        }
    }

    #[test]
    fn wildcard_access_is_campus_scoped_without_persisted_grants() {
        assert_eq!(
            gradebook_access_scope(
                &access(&["*"]),
                &RecordScopeGrants::empty(),
                AuditActor::person(Uuid::new_v4()),
            ),
            Ok(GradebookAccessScope::Campus)
        );
    }

    #[test]
    fn assigned_scope_is_bound_to_the_authenticated_person() {
        let user_id = Uuid::new_v4();
        let family = RecordScopeFamilyKey::parse("academics.gradebook")
            .unwrap_or_else(|error| panic!("test family must be valid: {error}"));
        let grants = RecordScopeGrants::from_grants([RecordScopeGrant::new(
            family,
            RecordScopeKind::Assigned,
        )]);
        assert_eq!(
            gradebook_access_scope(
                &access(&["academics:view"]),
                &grants,
                AuditActor::person(user_id),
            ),
            Ok(GradebookAccessScope::AssignedTo(user_id))
        );
    }

    #[test]
    fn missing_gradebook_scope_denies_non_owner_access() {
        assert!(
            gradebook_access_scope(
                &access(&["academics:view"]),
                &RecordScopeGrants::empty(),
                AuditActor::person(Uuid::new_v4()),
            )
            .is_err()
        );
    }
}

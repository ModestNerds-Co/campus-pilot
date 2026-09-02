//! Authenticated Attendance HTTP routes over typed module operations.
//!
//! Authentication is mounted by the application. This scope applies the exact
//! licensed Attendance operation from the shared product catalogue.

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

use crate::dtos::{
    AcknowledgeAttendanceExceptionRequest, AttendanceAccessScope, AttendanceExceptionListQuery,
    AttendanceLessonSessionListQuery, CancelAttendanceLessonSessionRequest,
    CreateAttendanceRegisterRequest, DeleteAttendanceRegisterQuery, LearnerAttendanceHistoryQuery,
    OpenAttendanceLessonSessionRequest, PaginatedAttendanceExceptionsResponse,
    PaginatedAttendanceLessonSessionsResponse, PaginatedAttendanceRegistersResponse,
    ReopenAttendanceExceptionRequest, ReopenAttendanceRegisterRequest,
    ResolveAttendanceExceptionRequest, SubmitAttendanceRegisterRequest,
    SyncAttendanceLessonSessionsRequest, UpdateAttendanceMarksRequest,
};
use crate::{AttendanceOps, AttendanceRegisterListQuery};

type AttendanceAuthority = (
    web::ReqData<AuditActor>,
    web::ReqData<AccessContext>,
    web::ReqData<RecordScopeGrants>,
);

#[get("/references")]
async fn reference_data(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: AttendanceAuthority,
) -> HttpResponse {
    let Some(scope) = attendance_scope(authority) else {
        return forbidden();
    };
    match AttendanceOps::reference_data(pool.get_ref(), tenant_id(tenant), scope).await {
        Ok(data) => ok(data),
        Err(_) => internal_error(),
    }
}

#[get("/registers")]
async fn list_registers(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: AttendanceAuthority,
    query: web::Query<AttendanceRegisterListQuery>,
) -> HttpResponse {
    let Some(scope) = attendance_scope(authority) else {
        return forbidden();
    };
    let (page, per_page) = bounded_page(query.page, query.per_page);
    match AttendanceOps::list(pool.get_ref(), tenant_id(tenant), &query.0, scope).await {
        Ok((registers, total)) => HttpResponse::Ok().json(ApiResponse::with_pagination(
            StatusCode::OK,
            Some(PaginatedAttendanceRegistersResponse { registers }),
            PaginationMeta::new(page as u32, per_page as u32, total),
            None,
        )),
        Err(error) => operation_error(error),
    }
}

#[post("/registers")]
async fn create_register(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: AttendanceAuthority,
    request_context: web::ReqData<RequestContext>,
    body: web::Json<CreateAttendanceRegisterRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    let (actor, access, grants) = authority;
    let actor = actor.into_inner();
    let Some(scope) = access_scope(&access, &grants, actor) else {
        return forbidden();
    };
    match AttendanceOps::create(
        pool.get_ref(),
        tenant_id(tenant),
        actor,
        request_context.into_inner(),
        &body.0,
        scope,
    )
    .await
    {
        Ok(register) => HttpResponse::Created().json(ApiResponse::from_status(
            StatusCode::CREATED,
            Some(register),
            None,
        )),
        Err(error) => operation_error(error),
    }
}

#[get("/registers/{id}")]
async fn read_register(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: AttendanceAuthority,
    path: web::Path<Uuid>,
) -> HttpResponse {
    let Some(scope) = attendance_scope(authority) else {
        return forbidden();
    };
    match AttendanceOps::get(pool.get_ref(), tenant_id(tenant), path.into_inner(), scope).await {
        Ok(Some(register)) => ok(register),
        Ok(None) => not_found(),
        Err(_) => internal_error(),
    }
}

#[get("/learners/{id}/history")]
async fn learner_history(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: AttendanceAuthority,
    path: web::Path<Uuid>,
    query: web::Query<LearnerAttendanceHistoryQuery>,
) -> HttpResponse {
    let Some(scope) = attendance_scope(authority) else {
        return forbidden();
    };
    let (page, per_page) = bounded_page(query.page, query.per_page);
    match AttendanceOps::learner_history(
        pool.get_ref(),
        tenant_id(tenant),
        path.into_inner(),
        &query.0,
        scope,
    )
    .await
    {
        Ok(Some((history, total))) => HttpResponse::Ok().json(ApiResponse::with_pagination(
            StatusCode::OK,
            Some(history),
            PaginationMeta::new(page as u32, per_page as u32, total),
            None,
        )),
        Ok(None) => learner_not_found(),
        Err(error) => operation_error(error),
    }
}

#[put("/registers/{id}/marks")]
async fn update_marks(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: AttendanceAuthority,
    request_context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<UpdateAttendanceMarksRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    let (actor, access, grants) = authority;
    let actor = actor.into_inner();
    let Some(scope) = access_scope(&access, &grants, actor) else {
        return forbidden();
    };
    match AttendanceOps::update_marks(
        pool.get_ref(),
        tenant_id(tenant),
        path.into_inner(),
        actor,
        request_context.into_inner(),
        &body.0,
        scope,
    )
    .await
    {
        Ok(Some(register)) => ok(register),
        Ok(None) => not_found(),
        Err(error) => operation_error(error),
    }
}

#[post("/registers/{id}/submit")]
async fn submit_register(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: AttendanceAuthority,
    request_context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<SubmitAttendanceRegisterRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    let (actor, access, grants) = authority;
    let actor = actor.into_inner();
    let Some(scope) = access_scope(&access, &grants, actor) else {
        return forbidden();
    };
    match AttendanceOps::submit(
        pool.get_ref(),
        tenant_id(tenant),
        path.into_inner(),
        actor,
        request_context.into_inner(),
        body.expected_version,
        scope,
    )
    .await
    {
        Ok(Some(register)) => ok(register),
        Ok(None) => not_found(),
        Err(error) => operation_error(error),
    }
}

#[post("/registers/{id}/reopen")]
async fn reopen_register(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: AttendanceAuthority,
    request_context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<ReopenAttendanceRegisterRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    let (actor, access, grants) = authority;
    let actor = actor.into_inner();
    let Some(scope) = access_scope(&access, &grants, actor) else {
        return forbidden();
    };
    match AttendanceOps::reopen(
        pool.get_ref(),
        tenant_id(tenant),
        path.into_inner(),
        actor,
        request_context.into_inner(),
        &body.0,
        scope,
    )
    .await
    {
        Ok(Some(register)) => ok(register),
        Ok(None) => not_found(),
        Err(error) => operation_error(error),
    }
}

#[delete("/registers/{id}")]
async fn delete_register(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: AttendanceAuthority,
    request_context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    query: web::Query<DeleteAttendanceRegisterQuery>,
) -> HttpResponse {
    if let Some(response) = validation_response(&query.0) {
        return response;
    }
    let (actor, access, grants) = authority;
    let actor = actor.into_inner();
    let Some(scope) = access_scope(&access, &grants, actor) else {
        return forbidden();
    };
    match AttendanceOps::delete(
        pool.get_ref(),
        tenant_id(tenant),
        path.into_inner(),
        actor,
        request_context.into_inner(),
        query.expected_version,
        scope,
    )
    .await
    {
        Ok(true) => ok(serde_json::json!({ "deleted": true })),
        Ok(false) => not_found(),
        Err(error) => operation_error(error),
    }
}

#[get("/lesson-sessions")]
async fn list_lesson_sessions(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: AttendanceAuthority,
    query: web::Query<AttendanceLessonSessionListQuery>,
) -> HttpResponse {
    let Some(scope) = attendance_scope(authority) else {
        return forbidden();
    };
    let (page, per_page) = bounded_page(query.page, query.per_page);
    match AttendanceOps::list_lesson_sessions(pool.get_ref(), tenant_id(tenant), &query.0, scope)
        .await
    {
        Ok((sessions, total)) => HttpResponse::Ok().json(ApiResponse::with_pagination(
            StatusCode::OK,
            Some(PaginatedAttendanceLessonSessionsResponse { sessions }),
            PaginationMeta::new(page as u32, per_page as u32, total),
            None,
        )),
        Err(error) => operation_error(error),
    }
}

#[get("/lesson-sessions/{id}")]
async fn read_lesson_session(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: AttendanceAuthority,
    path: web::Path<Uuid>,
) -> HttpResponse {
    let Some(scope) = attendance_scope(authority) else {
        return forbidden();
    };
    match AttendanceOps::get_lesson_session(
        pool.get_ref(),
        tenant_id(tenant),
        path.into_inner(),
        scope,
    )
    .await
    {
        Ok(Some(session)) => ok(session),
        Ok(None) => lesson_session_not_found(),
        Err(error) => operation_error(error),
    }
}

#[post("/lesson-sessions/sync")]
async fn sync_lesson_sessions(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: AttendanceAuthority,
    request_context: web::ReqData<RequestContext>,
    body: web::Json<SyncAttendanceLessonSessionsRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    let (actor, access, grants) = authority;
    let actor = actor.into_inner();
    let Some(scope @ AttendanceAccessScope::Campus) = access_scope(&access, &grants, actor) else {
        return forbidden();
    };
    match AttendanceOps::sync_lesson_sessions(
        pool.get_ref(),
        tenant_id(tenant),
        actor,
        request_context.into_inner(),
        &body.0,
        scope,
    )
    .await
    {
        Ok(result) => ok(result),
        Err(error) => operation_error(error),
    }
}

#[post("/lesson-sessions/{id}/open")]
async fn open_lesson_session(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: AttendanceAuthority,
    request_context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<OpenAttendanceLessonSessionRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    let (actor, access, grants) = authority;
    let actor = actor.into_inner();
    let Some(scope) = access_scope(&access, &grants, actor) else {
        return forbidden();
    };
    match AttendanceOps::open_lesson_session(
        pool.get_ref(),
        tenant_id(tenant),
        path.into_inner(),
        actor,
        request_context.into_inner(),
        &body.0,
        scope,
    )
    .await
    {
        Ok(Some(session)) => ok(session),
        Ok(None) => lesson_session_not_found(),
        Err(error) => operation_error(error),
    }
}

#[post("/lesson-sessions/{id}/cancel")]
async fn cancel_lesson_session(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: AttendanceAuthority,
    request_context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<CancelAttendanceLessonSessionRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    let (actor, access, grants) = authority;
    let actor = actor.into_inner();
    let Some(scope @ AttendanceAccessScope::Campus) = access_scope(&access, &grants, actor) else {
        return forbidden();
    };
    match AttendanceOps::cancel_lesson_session(
        pool.get_ref(),
        tenant_id(tenant),
        path.into_inner(),
        actor,
        request_context.into_inner(),
        &body.0,
        scope,
    )
    .await
    {
        Ok(Some(session)) => ok(session),
        Ok(None) => lesson_session_not_found(),
        Err(error) => operation_error(error),
    }
}

#[get("/exceptions")]
async fn list_exceptions(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: AttendanceAuthority,
    query: web::Query<AttendanceExceptionListQuery>,
) -> HttpResponse {
    let Some(scope @ AttendanceAccessScope::Campus) = attendance_scope(authority) else {
        return forbidden();
    };
    let (page, per_page) = bounded_page(query.page, query.per_page);
    match AttendanceOps::list_exceptions(pool.get_ref(), tenant_id(tenant), &query.0, scope).await {
        Ok((exceptions, total)) => HttpResponse::Ok().json(ApiResponse::with_pagination(
            StatusCode::OK,
            Some(PaginatedAttendanceExceptionsResponse { exceptions }),
            PaginationMeta::new(page as u32, per_page as u32, total),
            None,
        )),
        Err(error) => operation_error(error),
    }
}

#[get("/exceptions/{id}")]
async fn read_exception(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: AttendanceAuthority,
    path: web::Path<Uuid>,
) -> HttpResponse {
    let Some(scope @ AttendanceAccessScope::Campus) = attendance_scope(authority) else {
        return forbidden();
    };
    match AttendanceOps::get_exception(pool.get_ref(), tenant_id(tenant), path.into_inner(), scope)
        .await
    {
        Ok(Some(exception)) => ok(exception),
        Ok(None) => exception_not_found(),
        Err(error) => operation_error(error),
    }
}

#[post("/exceptions/{id}/acknowledge")]
async fn acknowledge_exception(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: AttendanceAuthority,
    request_context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<AcknowledgeAttendanceExceptionRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    let (actor, access, grants) = authority;
    let actor = actor.into_inner();
    let Some(scope @ AttendanceAccessScope::Campus) = access_scope(&access, &grants, actor) else {
        return forbidden();
    };
    match AttendanceOps::acknowledge_exception(
        pool.get_ref(),
        tenant_id(tenant),
        path.into_inner(),
        actor,
        request_context.into_inner(),
        &body.0,
        scope,
    )
    .await
    {
        Ok(Some(exception)) => ok(exception),
        Ok(None) => exception_not_found(),
        Err(error) => operation_error(error),
    }
}

#[post("/exceptions/{id}/resolve")]
async fn resolve_exception(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: AttendanceAuthority,
    request_context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<ResolveAttendanceExceptionRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    let (actor, access, grants) = authority;
    let actor = actor.into_inner();
    let Some(scope @ AttendanceAccessScope::Campus) = access_scope(&access, &grants, actor) else {
        return forbidden();
    };
    match AttendanceOps::resolve_exception(
        pool.get_ref(),
        tenant_id(tenant),
        path.into_inner(),
        actor,
        request_context.into_inner(),
        &body.0,
        scope,
    )
    .await
    {
        Ok(Some(exception)) => ok(exception),
        Ok(None) => exception_not_found(),
        Err(error) => operation_error(error),
    }
}

#[post("/exceptions/{id}/reopen")]
async fn reopen_exception(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: AttendanceAuthority,
    request_context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<ReopenAttendanceExceptionRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    let (actor, access, grants) = authority;
    let actor = actor.into_inner();
    let Some(scope @ AttendanceAccessScope::Campus) = access_scope(&access, &grants, actor) else {
        return forbidden();
    };
    match AttendanceOps::reopen_exception(
        pool.get_ref(),
        tenant_id(tenant),
        path.into_inner(),
        actor,
        request_context.into_inner(),
        &body.0,
        scope,
    )
    .await
    {
        Ok(Some(exception)) => ok(exception),
        Ok(None) => exception_not_found(),
        Err(error) => operation_error(error),
    }
}

pub fn routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("")
            .wrap(RequirePermission::new("attendance"))
            .service(reference_data)
            .service(list_registers)
            .service(create_register)
            .service(read_register)
            .service(learner_history)
            .service(update_marks)
            .service(submit_register)
            .service(reopen_register)
            .service(delete_register)
            .service(list_lesson_sessions)
            .service(sync_lesson_sessions)
            .service(read_lesson_session)
            .service(open_lesson_session)
            .service(cancel_lesson_session)
            .service(list_exceptions)
            .service(read_exception)
            .service(acknowledge_exception)
            .service(resolve_exception)
            .service(reopen_exception),
    );
}

fn attendance_scope(authority: AttendanceAuthority) -> Option<AttendanceAccessScope> {
    let (actor, access, grants) = authority;
    access_scope(&access, &grants, actor.into_inner())
}

fn access_scope(
    _access: &AccessContext,
    grants: &RecordScopeGrants,
    actor: AuditActor,
) -> Option<AttendanceAccessScope> {
    let family = RecordScopeFamilyKey::parse("attendance.registers").ok()?;
    match grants.effective_scope(&family) {
        Some(EffectiveRecordScope::Campus) => Some(AttendanceAccessScope::Campus),
        Some(EffectiveRecordScope::Assigned | EffectiveRecordScope::SelfAndAssigned) => {
            actor.user_id().map(AttendanceAccessScope::AssignedTo)
        }
        Some(EffectiveRecordScope::SelfRecord) | None => None,
    }
}

fn tenant_id(tenant: web::ReqData<TenantId>) -> Uuid {
    tenant.into_inner().into_inner()
}

fn ok<T: Serialize>(value: T) -> HttpResponse {
    HttpResponse::Ok().json(ApiResponse::from_status(StatusCode::OK, Some(value), None))
}

fn not_found() -> HttpResponse {
    HttpResponse::NotFound().json(ApiResponse::from_status(
        StatusCode::NOT_FOUND,
        None::<()>,
        Some(vec!["Attendance register not found".to_string()]),
    ))
}

fn learner_not_found() -> HttpResponse {
    HttpResponse::NotFound().json(ApiResponse::from_status(
        StatusCode::NOT_FOUND,
        None::<()>,
        Some(vec!["Learner attendance history not found".to_string()]),
    ))
}

fn lesson_session_not_found() -> HttpResponse {
    HttpResponse::NotFound().json(ApiResponse::from_status(
        StatusCode::NOT_FOUND,
        None::<()>,
        Some(vec!["Attendance lesson session not found".to_string()]),
    ))
}

fn exception_not_found() -> HttpResponse {
    HttpResponse::NotFound().json(ApiResponse::from_status(
        StatusCode::NOT_FOUND,
        None::<()>,
        Some(vec!["Attendance exception not found".to_string()]),
    ))
}

fn forbidden() -> HttpResponse {
    HttpResponse::Forbidden().json(ApiResponse::from_status(
        StatusCode::FORBIDDEN,
        None::<()>,
        Some(vec!["Attendance record scope is unavailable".to_string()]),
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
        || message.starts_with("An empty ")
        || message.starts_with("Attendance ");
    if operational {
        HttpResponse::BadRequest().json(ApiResponse::from_status(
            StatusCode::BAD_REQUEST,
            None::<()>,
            Some(vec![message]),
        ))
    } else {
        internal_error()
    }
}

fn internal_error() -> HttpResponse {
    HttpResponse::InternalServerError().json(ApiResponse::from_status(
        StatusCode::INTERNAL_SERVER_ERROR,
        None::<()>,
        Some(vec![
            "Attendance could not complete the request.".to_string(),
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
        AccessContext, EntitlementSnapshot, LeaseLifecycle, ModuleEntitlementState,
        RecordScopeFamilyKey, RecordScopeGrant, RecordScopeGrants, RecordScopeKind,
    };
    use uuid::Uuid;

    use super::access_scope;
    use crate::AttendanceAccessScope;

    fn access(permissions: &[&str]) -> AccessContext {
        AccessContext {
            role_keys: Vec::new(),
            permissions: permissions
                .iter()
                .map(|value| (*value).to_string())
                .collect(),
            enabled_modules: vec!["attendance".to_string()],
            entitlements: EntitlementSnapshot::new(
                LeaseLifecycle::Active,
                [("attendance".to_string(), ModuleEntitlementState::Enabled)],
                [],
            )
            .unwrap_or_else(|_| unreachable!()),
        }
    }

    fn grants(kind: RecordScopeKind) -> RecordScopeGrants {
        RecordScopeGrants::from_grants([RecordScopeGrant::new(
            RecordScopeFamilyKey::parse("attendance.registers").unwrap_or_else(|_| unreachable!()),
            kind,
        )])
    }

    #[test]
    fn wildcard_permission_never_creates_attendance_record_scope() {
        let actor = AuditActor::person(Uuid::new_v4());
        assert_eq!(
            access_scope(&access(&["*"]), &RecordScopeGrants::empty(), actor),
            None
        );
    }

    #[test]
    fn assigned_and_campus_grants_keep_distinct_visibility() {
        let user_id = Uuid::new_v4();
        let actor = AuditActor::person(user_id);
        assert_eq!(
            access_scope(
                &access(&["attendance:view"]),
                &grants(RecordScopeKind::Assigned),
                actor,
            ),
            Some(AttendanceAccessScope::AssignedTo(user_id))
        );
        assert_eq!(
            access_scope(
                &access(&["attendance:view"]),
                &grants(RecordScopeKind::Campus),
                actor,
            ),
            Some(AttendanceAccessScope::Campus)
        );
    }
}

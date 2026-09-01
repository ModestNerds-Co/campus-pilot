//! Authenticated Academic Progress and Reporting HTTP routes.
//!
//! Authentication is mounted by the application. This scope applies the exact
//! Academics operation plus Gradebook, SIS, Attendance, and HR dependencies.

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
    AcademicReportBatchListQuery, AcademicReportingAccessScope, AcademicReportingOps,
    CreateGradingSchemeRequest, DeleteAcademicReportQuery, DeleteGradingSchemeQuery,
    GenerateAcademicReportRequest, ReopenAcademicReportRequest, TransitionAcademicReportRequest,
    UpdateGradingSchemeRequest, UpdateReportCardReviewRequest,
    UpdateReportCardTeacherCommentRequest,
};

type ReportingRouteAuthority = (
    web::ReqData<AuditActor>,
    web::ReqData<AccessContext>,
    web::ReqData<RecordScopeGrants>,
);

#[get("/references")]
async fn reference_data(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: ReportingRouteAuthority,
) -> HttpResponse {
    let (actor, access, grants) = authority;
    let Ok(scope) = reporting_access_scope(&access, &grants, actor.into_inner()) else {
        return forbidden();
    };
    match AcademicReportingOps::reference_data(pool.get_ref(), tenant_id(tenant), scope).await {
        Ok(data) => ok(data),
        Err(_) => internal_error(),
    }
}

#[get("/grading-schemes")]
async fn list_grading_schemes(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: ReportingRouteAuthority,
) -> HttpResponse {
    let (actor, access, grants) = authority;
    let Ok(scope) = reporting_access_scope(&access, &grants, actor.into_inner()) else {
        return forbidden();
    };
    if matches!(scope, AcademicReportingAccessScope::SelfFor(_)) {
        return forbidden();
    }
    match AcademicReportingOps::list_grading_schemes(pool.get_ref(), tenant_id(tenant), None).await
    {
        Ok(data) => ok(data),
        Err(_) => internal_error(),
    }
}

#[post("/grading-schemes")]
async fn create_grading_scheme(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    actor: web::ReqData<AuditActor>,
    request_context: web::ReqData<RequestContext>,
    authority: (web::ReqData<AccessContext>, web::ReqData<RecordScopeGrants>),
    body: web::Json<CreateGradingSchemeRequest>,
) -> HttpResponse {
    let (access, grants) = authority;
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    let actor_value = actor.into_inner();
    if !campus_reporting_scope(&access, &grants, actor_value) {
        return forbidden();
    }
    match AcademicReportingOps::create_grading_scheme(
        pool.get_ref(),
        tenant_id(tenant),
        actor_value,
        request_context.into_inner(),
        &body.0,
    )
    .await
    {
        Ok(scheme) => HttpResponse::Created().json(ApiResponse::from_status(
            StatusCode::CREATED,
            Some(scheme),
            None,
        )),
        Err(error) => operation_error(error),
    }
}

#[get("/grading-schemes/{id}")]
async fn read_grading_scheme(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: ReportingRouteAuthority,
    path: web::Path<Uuid>,
) -> HttpResponse {
    let (actor, access, grants) = authority;
    let Ok(scope) = reporting_access_scope(&access, &grants, actor.into_inner()) else {
        return forbidden();
    };
    if matches!(scope, AcademicReportingAccessScope::SelfFor(_)) {
        return forbidden();
    }
    match AcademicReportingOps::get_grading_scheme(
        pool.get_ref(),
        tenant_id(tenant),
        path.into_inner(),
    )
    .await
    {
        Ok(Some(scheme)) => ok(scheme),
        Ok(None) => grading_scheme_not_found(),
        Err(_) => internal_error(),
    }
}

#[put("/grading-schemes/{id}")]
async fn update_grading_scheme(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    actor: web::ReqData<AuditActor>,
    request_context: web::ReqData<RequestContext>,
    authority: (web::ReqData<AccessContext>, web::ReqData<RecordScopeGrants>),
    path: web::Path<Uuid>,
    body: web::Json<UpdateGradingSchemeRequest>,
) -> HttpResponse {
    let (access, grants) = authority;
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    let actor_value = actor.into_inner();
    if !campus_reporting_scope(&access, &grants, actor_value) {
        return forbidden();
    }
    match AcademicReportingOps::update_grading_scheme(
        pool.get_ref(),
        tenant_id(tenant),
        path.into_inner(),
        actor_value,
        request_context.into_inner(),
        &body.0,
    )
    .await
    {
        Ok(Some(scheme)) => ok(scheme),
        Ok(None) => grading_scheme_not_found(),
        Err(error) => operation_error(error),
    }
}

#[post("/grading-schemes/{id}/retire")]
async fn retire_grading_scheme(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    actor: web::ReqData<AuditActor>,
    request_context: web::ReqData<RequestContext>,
    authority: (web::ReqData<AccessContext>, web::ReqData<RecordScopeGrants>),
    path: web::Path<Uuid>,
    body: web::Json<TransitionAcademicReportRequest>,
) -> HttpResponse {
    let (access, grants) = authority;
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    let actor_value = actor.into_inner();
    if !campus_reporting_scope(&access, &grants, actor_value) {
        return forbidden();
    }
    match AcademicReportingOps::retire_grading_scheme(
        pool.get_ref(),
        tenant_id(tenant),
        path.into_inner(),
        actor_value,
        request_context.into_inner(),
        body.expected_version,
    )
    .await
    {
        Ok(Some(scheme)) => ok(scheme),
        Ok(None) => grading_scheme_not_found(),
        Err(error) => operation_error(error),
    }
}

#[delete("/grading-schemes/{id}")]
async fn delete_grading_scheme(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    actor: web::ReqData<AuditActor>,
    request_context: web::ReqData<RequestContext>,
    authority: (web::ReqData<AccessContext>, web::ReqData<RecordScopeGrants>),
    path: web::Path<Uuid>,
    query: web::Query<DeleteGradingSchemeQuery>,
) -> HttpResponse {
    let (access, grants) = authority;
    if let Some(response) = validation_response(&query.0) {
        return response;
    }
    let actor_value = actor.into_inner();
    if !campus_reporting_scope(&access, &grants, actor_value) {
        return forbidden();
    }
    match AcademicReportingOps::delete_grading_scheme(
        pool.get_ref(),
        tenant_id(tenant),
        path.into_inner(),
        actor_value,
        request_context.into_inner(),
        query.expected_version,
    )
    .await
    {
        Ok(true) => ok(serde_json::json!({ "deleted": true })),
        Ok(false) => grading_scheme_not_found(),
        Err(error) => operation_error(error),
    }
}

#[get("/report-batches")]
async fn list_report_batches(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: ReportingRouteAuthority,
    query: web::Query<AcademicReportBatchListQuery>,
) -> HttpResponse {
    let (actor, access, grants) = authority;
    let Ok(scope) = reporting_access_scope(&access, &grants, actor.into_inner()) else {
        return forbidden();
    };
    let (page, per_page) = bounded_page(query.page, query.per_page);
    match AcademicReportingOps::list_report_batches(
        pool.get_ref(),
        tenant_id(tenant),
        &query.0,
        scope,
    )
    .await
    {
        Ok((data, total)) => HttpResponse::Ok().json(ApiResponse::with_pagination(
            StatusCode::OK,
            Some(data),
            PaginationMeta::new(page as u32, per_page as u32, total),
            None,
        )),
        Err(_) => internal_error(),
    }
}

#[post("/report-batches")]
async fn generate_report_batch(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    actor: web::ReqData<AuditActor>,
    request_context: web::ReqData<RequestContext>,
    authority: (web::ReqData<AccessContext>, web::ReqData<RecordScopeGrants>),
    body: web::Json<GenerateAcademicReportRequest>,
) -> HttpResponse {
    let (access, grants) = authority;
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    let actor_value = actor.into_inner();
    let Ok(scope) = reporting_access_scope(&access, &grants, actor_value) else {
        return forbidden();
    };
    match AcademicReportingOps::generate_report_batch(
        pool.get_ref(),
        tenant_id(tenant),
        actor_value,
        request_context.into_inner(),
        scope,
        &body.0,
    )
    .await
    {
        Ok(report) => HttpResponse::Created().json(ApiResponse::from_status(
            StatusCode::CREATED,
            Some(report),
            None,
        )),
        Err(error) => operation_error(error),
    }
}

#[get("/report-batches/{id}")]
async fn read_report_batch(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: ReportingRouteAuthority,
    path: web::Path<Uuid>,
) -> HttpResponse {
    let (actor, access, grants) = authority;
    let Ok(scope) = reporting_access_scope(&access, &grants, actor.into_inner()) else {
        return forbidden();
    };
    match AcademicReportingOps::get_report_batch(
        pool.get_ref(),
        tenant_id(tenant),
        path.into_inner(),
        scope,
    )
    .await
    {
        Ok(Some(report)) => ok(report),
        Ok(None) => report_not_found(),
        Err(_) => internal_error(),
    }
}

#[put("/report-cards/{id}/teacher-comment")]
async fn update_teacher_comment(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    actor: web::ReqData<AuditActor>,
    request_context: web::ReqData<RequestContext>,
    authority: (web::ReqData<AccessContext>, web::ReqData<RecordScopeGrants>),
    path: web::Path<Uuid>,
    body: web::Json<UpdateReportCardTeacherCommentRequest>,
) -> HttpResponse {
    let (access, grants) = authority;
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    let actor_value = actor.into_inner();
    let Ok(scope) = reporting_access_scope(&access, &grants, actor_value) else {
        return forbidden();
    };
    match AcademicReportingOps::update_teacher_comment(
        pool.get_ref(),
        tenant_id(tenant),
        path.into_inner(),
        actor_value,
        request_context.into_inner(),
        scope,
        &body.0,
    )
    .await
    {
        Ok(Some(report)) => ok(report),
        Ok(None) => report_not_found(),
        Err(error) => operation_error(error),
    }
}

#[put("/report-cards/{id}/review")]
async fn update_report_review(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    actor: web::ReqData<AuditActor>,
    request_context: web::ReqData<RequestContext>,
    authority: (web::ReqData<AccessContext>, web::ReqData<RecordScopeGrants>),
    path: web::Path<Uuid>,
    body: web::Json<UpdateReportCardReviewRequest>,
) -> HttpResponse {
    let (access, grants) = authority;
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    let actor_value = actor.into_inner();
    if !campus_reporting_scope(&access, &grants, actor_value) {
        return forbidden();
    }
    match AcademicReportingOps::update_report_review(
        pool.get_ref(),
        tenant_id(tenant),
        path.into_inner(),
        actor_value,
        request_context.into_inner(),
        &body.0,
    )
    .await
    {
        Ok(Some(report)) => ok(report),
        Ok(None) => report_not_found(),
        Err(error) => operation_error(error),
    }
}

#[post("/report-batches/{id}/review")]
async fn review_report_batch(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    actor: web::ReqData<AuditActor>,
    request_context: web::ReqData<RequestContext>,
    authority: (web::ReqData<AccessContext>, web::ReqData<RecordScopeGrants>),
    path: web::Path<Uuid>,
    body: web::Json<TransitionAcademicReportRequest>,
) -> HttpResponse {
    transition_report(
        pool,
        tenant,
        actor,
        request_context,
        authority,
        path.into_inner(),
        &body.0,
        "review",
    )
    .await
}

#[post("/report-batches/{id}/publish")]
async fn publish_report_batch(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    actor: web::ReqData<AuditActor>,
    request_context: web::ReqData<RequestContext>,
    authority: (web::ReqData<AccessContext>, web::ReqData<RecordScopeGrants>),
    path: web::Path<Uuid>,
    body: web::Json<TransitionAcademicReportRequest>,
) -> HttpResponse {
    transition_report(
        pool,
        tenant,
        actor,
        request_context,
        authority,
        path.into_inner(),
        &body.0,
        "publish",
    )
    .await
}

#[post("/report-batches/{id}/reopen")]
async fn reopen_report_batch(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    actor: web::ReqData<AuditActor>,
    request_context: web::ReqData<RequestContext>,
    authority: (web::ReqData<AccessContext>, web::ReqData<RecordScopeGrants>),
    path: web::Path<Uuid>,
    body: web::Json<ReopenAcademicReportRequest>,
) -> HttpResponse {
    let (access, grants) = authority;
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    let actor_value = actor.into_inner();
    if !campus_reporting_scope(&access, &grants, actor_value) {
        return forbidden();
    }
    match AcademicReportingOps::reopen_report_batch(
        pool.get_ref(),
        tenant_id(tenant),
        path.into_inner(),
        actor_value,
        request_context.into_inner(),
        &body.0,
    )
    .await
    {
        Ok(Some(report)) => ok(report),
        Ok(None) => report_not_found(),
        Err(error) => operation_error(error),
    }
}

#[delete("/report-batches/{id}")]
async fn delete_report_batch(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    actor: web::ReqData<AuditActor>,
    request_context: web::ReqData<RequestContext>,
    authority: (web::ReqData<AccessContext>, web::ReqData<RecordScopeGrants>),
    path: web::Path<Uuid>,
    query: web::Query<DeleteAcademicReportQuery>,
) -> HttpResponse {
    let (access, grants) = authority;
    if let Some(response) = validation_response(&query.0) {
        return response;
    }
    let actor_value = actor.into_inner();
    if !campus_reporting_scope(&access, &grants, actor_value) {
        return forbidden();
    }
    match AcademicReportingOps::delete_report_batch(
        pool.get_ref(),
        tenant_id(tenant),
        path.into_inner(),
        actor_value,
        request_context.into_inner(),
        query.expected_version,
    )
    .await
    {
        Ok(true) => ok(serde_json::json!({ "deleted": true })),
        Ok(false) => report_not_found(),
        Err(error) => operation_error(error),
    }
}

#[get("/learners/{id}/transcript")]
async fn learner_transcript(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: ReportingRouteAuthority,
    path: web::Path<Uuid>,
) -> HttpResponse {
    let (actor, access, grants) = authority;
    let Ok(scope) = reporting_access_scope(&access, &grants, actor.into_inner()) else {
        return forbidden();
    };
    match AcademicReportingOps::learner_transcript(
        pool.get_ref(),
        tenant_id(tenant),
        path.into_inner(),
        scope,
    )
    .await
    {
        Ok(Some(transcript)) => ok(transcript),
        Ok(None) => transcript_not_found(),
        Err(_) => internal_error(),
    }
}

pub fn routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("")
            .wrap(RequirePermission::new("academics"))
            .service(reference_data)
            .service(list_grading_schemes)
            .service(create_grading_scheme)
            .service(read_grading_scheme)
            .service(update_grading_scheme)
            .service(retire_grading_scheme)
            .service(delete_grading_scheme)
            .service(list_report_batches)
            .service(generate_report_batch)
            .service(read_report_batch)
            .service(update_teacher_comment)
            .service(update_report_review)
            .service(review_report_batch)
            .service(publish_report_batch)
            .service(reopen_report_batch)
            .service(delete_report_batch)
            .service(learner_transcript),
    );
}

#[allow(
    clippy::too_many_arguments,
    reason = "Actix extractors keep authority explicit"
)]
async fn transition_report(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    actor: web::ReqData<AuditActor>,
    request_context: web::ReqData<RequestContext>,
    authority: (web::ReqData<AccessContext>, web::ReqData<RecordScopeGrants>),
    id: Uuid,
    body: &TransitionAcademicReportRequest,
    action: &str,
) -> HttpResponse {
    let (access, grants) = authority;
    if let Some(response) = validation_response(body) {
        return response;
    }
    let actor_value = actor.into_inner();
    if !campus_reporting_scope(&access, &grants, actor_value) {
        return forbidden();
    }
    let result = if action == "review" {
        AcademicReportingOps::review_report_batch(
            pool.get_ref(),
            tenant_id(tenant),
            id,
            actor_value,
            request_context.into_inner(),
            body.expected_version,
        )
        .await
    } else {
        AcademicReportingOps::publish_report_batch(
            pool.get_ref(),
            tenant_id(tenant),
            id,
            actor_value,
            request_context.into_inner(),
            body.expected_version,
        )
        .await
    };
    match result {
        Ok(Some(report)) => ok(report),
        Ok(None) => report_not_found(),
        Err(error) => operation_error(error),
    }
}

fn reporting_access_scope(
    access: &AccessContext,
    grants: &RecordScopeGrants,
    actor: AuditActor,
) -> Result<AcademicReportingAccessScope, ()> {
    if access.has_permission("*") {
        return Ok(AcademicReportingAccessScope::Campus);
    }
    let family = RecordScopeFamilyKey::parse("academics.reporting").map_err(|_| ())?;
    let user_id = actor.user_id().ok_or(())?;
    match grants.effective_scope(&family) {
        Some(EffectiveRecordScope::Campus) => Ok(AcademicReportingAccessScope::Campus),
        Some(EffectiveRecordScope::Assigned) => {
            Ok(AcademicReportingAccessScope::AssignedTo(user_id))
        }
        Some(EffectiveRecordScope::SelfRecord) => {
            Ok(AcademicReportingAccessScope::SelfFor(user_id))
        }
        Some(EffectiveRecordScope::SelfAndAssigned) => {
            Ok(AcademicReportingAccessScope::SelfAndAssigned(user_id))
        }
        None => Err(()),
    }
}

fn campus_reporting_scope(
    access: &AccessContext,
    grants: &RecordScopeGrants,
    actor: AuditActor,
) -> bool {
    matches!(
        reporting_access_scope(access, grants, actor),
        Ok(AcademicReportingAccessScope::Campus)
    )
}

fn tenant_id(tenant: web::ReqData<TenantId>) -> Uuid {
    tenant.into_inner().into_inner()
}

fn ok<T: Serialize>(value: T) -> HttpResponse {
    HttpResponse::Ok().json(ApiResponse::from_status(StatusCode::OK, Some(value), None))
}

fn report_not_found() -> HttpResponse {
    not_found("Academic report not found")
}

fn grading_scheme_not_found() -> HttpResponse {
    not_found("Grading scheme not found")
}

fn transcript_not_found() -> HttpResponse {
    not_found("Learner transcript not found")
}

fn not_found(message: &str) -> HttpResponse {
    HttpResponse::NotFound().json(ApiResponse::from_status(
        StatusCode::NOT_FOUND,
        None::<()>,
        Some(vec![message.to_string()]),
    ))
}

fn forbidden() -> HttpResponse {
    HttpResponse::Forbidden().json(ApiResponse::from_status(
        StatusCode::FORBIDDEN,
        None::<()>,
        Some(vec!["This academic record is unavailable".to_string()]),
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
        || message.contains("already retired")
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
        || message.starts_with("Self-service ")
        || message.starts_with("Source ")
        || message.starts_with("Resolve ")
        || message.starts_with("Grade ")
        || message.starts_with("An academic ");
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
            "Academic reporting could not complete the request".to_string(),
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

    use super::{AcademicReportingAccessScope, reporting_access_scope};

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

    fn grants(kind: RecordScopeKind) -> RecordScopeGrants {
        let family = RecordScopeFamilyKey::parse("academics.reporting")
            .unwrap_or_else(|error| panic!("test family must be valid: {error}"));
        RecordScopeGrants::from_grants([RecordScopeGrant::new(family, kind)])
    }

    #[test]
    fn wildcard_reporting_access_is_campus_scoped() {
        assert_eq!(
            reporting_access_scope(
                &access(&["*"]),
                &RecordScopeGrants::empty(),
                AuditActor::person(Uuid::new_v4()),
            ),
            Ok(AcademicReportingAccessScope::Campus)
        );
    }

    #[test]
    fn self_and_assigned_reporting_scopes_remain_distinct() {
        let user_id = Uuid::new_v4();
        let combined = RecordScopeGrants::from_grants([
            RecordScopeGrant::new(
                RecordScopeFamilyKey::parse("academics.reporting")
                    .unwrap_or_else(|error| panic!("{error}")),
                RecordScopeKind::SelfRecord,
            ),
            RecordScopeGrant::new(
                RecordScopeFamilyKey::parse("academics.reporting")
                    .unwrap_or_else(|error| panic!("{error}")),
                RecordScopeKind::Assigned,
            ),
        ]);
        assert_eq!(
            reporting_access_scope(
                &access(&["academics:view"]),
                &combined,
                AuditActor::person(user_id),
            ),
            Ok(AcademicReportingAccessScope::SelfAndAssigned(user_id))
        );
        assert_eq!(
            reporting_access_scope(
                &access(&["academics:view"]),
                &grants(RecordScopeKind::SelfRecord),
                AuditActor::person(user_id),
            ),
            Ok(AcademicReportingAccessScope::SelfFor(user_id))
        );
    }

    #[test]
    fn missing_reporting_scope_denies_non_owner_access() {
        assert!(
            reporting_access_scope(
                &access(&["academics:view"]),
                &RecordScopeGrants::empty(),
                AuditActor::person(Uuid::new_v4()),
            )
            .is_err()
        );
    }
}

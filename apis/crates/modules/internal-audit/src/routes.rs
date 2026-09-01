//! Authenticated, licensed, and record-scoped Internal Audit HTTP routes.

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
    CloseRequest, CreateEngagementRequest, CreateFindingRequest, CreatePlanRequest,
    EngagementsPage, EvidencePage, FindingsPage, InternalAuditAccessScope, InternalAuditListQuery,
    InternalAuditOps, LinkEvidenceRequest, PlansPage, UpdateEngagementRequest,
    UpdateFindingRequest, UpdateNumberingPolicyRequest, UpdatePlanRequest, VersionRequest,
};

type Authority = (web::ReqData<AccessContext>, web::ReqData<RecordScopeGrants>);

#[get("/numbering-policy")]
async fn numbering_policy(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
) -> HttpResponse {
    if !has_plan_scope(&authority.0, &authority.1) {
        return forbidden();
    }
    value_or_error(InternalAuditOps::numbering_policy(&pool, tenant_id(tenant)).await)
}

#[put("/numbering-policy")]
async fn update_numbering_policy(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    context: web::ReqData<RequestContext>,
    body: web::Json<UpdateNumberingPolicyRequest>,
) -> HttpResponse {
    if !has_plan_scope(&authority.0, &authority.1) {
        return forbidden();
    }
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    value_or_error(
        InternalAuditOps::update_numbering_policy(
            &pool,
            tenant_id(tenant),
            actor.into_inner(),
            context.into_inner(),
            &body,
        )
        .await,
    )
}

#[get("/plans")]
async fn list_plans(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    query: web::Query<InternalAuditListQuery>,
) -> HttpResponse {
    if !has_plan_scope(&authority.0, &authority.1) {
        return forbidden();
    }
    let (page, per_page) = bounded_page(&query);
    match InternalAuditOps::list_plans(&pool, tenant_id(tenant), &query).await {
        Ok((plans, total)) => paginated(PlansPage { plans }, page, per_page, total),
        Err(error) => operation_error(error),
    }
}

#[post("/plans")]
async fn create_plan(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    context: web::ReqData<RequestContext>,
    body: web::Json<CreatePlanRequest>,
) -> HttpResponse {
    if !has_plan_scope(&authority.0, &authority.1) {
        return forbidden();
    }
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    created_or_error(
        InternalAuditOps::create_plan(
            &pool,
            tenant_id(tenant),
            actor.into_inner(),
            context.into_inner(),
            &body,
        )
        .await,
    )
}

#[get("/plans/{id}")]
async fn read_plan(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    path: web::Path<Uuid>,
) -> HttpResponse {
    if !has_plan_scope(&authority.0, &authority.1) {
        return forbidden();
    }
    found(
        InternalAuditOps::get_plan(&pool, tenant_id(tenant), path.into_inner()).await,
        "Audit plan",
    )
}

#[put("/plans/{id}")]
async fn update_plan(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<UpdatePlanRequest>,
) -> HttpResponse {
    if !has_plan_scope(&authority.0, &authority.1) {
        return forbidden();
    }
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    updated_or_error(
        InternalAuditOps::update_plan(
            &pool,
            tenant_id(tenant),
            path.into_inner(),
            actor.into_inner(),
            context.into_inner(),
            &body,
        )
        .await,
        "Audit plan",
    )
}

#[delete("/plans/{id}")]
async fn delete_plan(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    query: web::Query<VersionRequest>,
) -> HttpResponse {
    if !has_plan_scope(&authority.0, &authority.1) {
        return forbidden();
    }
    if let Some(response) = validation_response(&query.0) {
        return response;
    }
    deleted_or_error(
        InternalAuditOps::delete_plan(
            &pool,
            tenant_id(tenant),
            path.into_inner(),
            actor.into_inner(),
            context.into_inner(),
            query.expected_version,
        )
        .await,
        "Audit plan",
    )
}

#[post("/plans/{id}/approve")]
async fn approve_plan(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<VersionRequest>,
) -> HttpResponse {
    if !has_plan_scope(&authority.0, &authority.1) {
        return forbidden();
    }
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    updated_or_error(
        InternalAuditOps::approve_plan(
            &pool,
            tenant_id(tenant),
            path.into_inner(),
            actor.into_inner(),
            context.into_inner(),
            body.expected_version,
        )
        .await,
        "Audit plan",
    )
}

#[post("/plans/{id}/close")]
async fn close_plan(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<CloseRequest>,
) -> HttpResponse {
    if !has_plan_scope(&authority.0, &authority.1) {
        return forbidden();
    }
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    updated_or_error(
        InternalAuditOps::close_plan(
            &pool,
            tenant_id(tenant),
            path.into_inner(),
            actor.into_inner(),
            context.into_inner(),
            &body,
        )
        .await,
        "Audit plan",
    )
}

#[get("/auditor-candidates")]
async fn auditor_candidates(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    query: web::Query<InternalAuditListQuery>,
) -> HttpResponse {
    if records_scope(&authority.0, &authority.1, *actor).is_err() {
        return forbidden();
    }
    value_or_error(
        InternalAuditOps::auditor_candidates(&pool, tenant_id(tenant), query.search.as_deref())
            .await,
    )
}

#[get("/engagements")]
async fn list_engagements(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    query: web::Query<InternalAuditListQuery>,
) -> HttpResponse {
    let Ok(scope) = records_scope(&authority.0, &authority.1, *actor) else {
        return forbidden();
    };
    let (page, per_page) = bounded_page(&query);
    match InternalAuditOps::list_engagements(&pool, tenant_id(tenant), scope, &query).await {
        Ok((engagements, total)) => {
            paginated(EngagementsPage { engagements }, page, per_page, total)
        }
        Err(error) => operation_error(error),
    }
}

#[post("/engagements")]
async fn create_engagement(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    context: web::ReqData<RequestContext>,
    body: web::Json<CreateEngagementRequest>,
) -> HttpResponse {
    let Ok(scope) = records_scope(&authority.0, &authority.1, *actor) else {
        return forbidden();
    };
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    created_or_error(
        InternalAuditOps::create_engagement(
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

#[get("/engagements/{id}")]
async fn read_engagement(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    path: web::Path<Uuid>,
) -> HttpResponse {
    let Ok(scope) = records_scope(&authority.0, &authority.1, *actor) else {
        return forbidden();
    };
    found(
        InternalAuditOps::get_engagement(&pool, tenant_id(tenant), scope, path.into_inner()).await,
        "Audit engagement",
    )
}

#[put("/engagements/{id}")]
async fn update_engagement(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<UpdateEngagementRequest>,
) -> HttpResponse {
    let Ok(scope) = records_scope(&authority.0, &authority.1, *actor) else {
        return forbidden();
    };
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    updated_or_error(
        InternalAuditOps::update_engagement(
            &pool,
            tenant_id(tenant),
            scope,
            path.into_inner(),
            actor.into_inner(),
            context.into_inner(),
            &body,
        )
        .await,
        "Audit engagement",
    )
}

#[delete("/engagements/{id}")]
async fn delete_engagement(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    query: web::Query<VersionRequest>,
) -> HttpResponse {
    let Ok(scope) = records_scope(&authority.0, &authority.1, *actor) else {
        return forbidden();
    };
    if let Some(response) = validation_response(&query.0) {
        return response;
    }
    deleted_or_error(
        InternalAuditOps::delete_engagement(
            &pool,
            tenant_id(tenant),
            scope,
            path.into_inner(),
            actor.into_inner(),
            context.into_inner(),
            query.expected_version,
        )
        .await,
        "Audit engagement",
    )
}

#[post("/engagements/{id}/start")]
async fn start_engagement(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<VersionRequest>,
) -> HttpResponse {
    transition_response(pool, tenant, authority, actor, context, path, body, "start").await
}

#[post("/engagements/{id}/begin-reporting")]
async fn begin_reporting(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<VersionRequest>,
) -> HttpResponse {
    transition_response(
        pool,
        tenant,
        authority,
        actor,
        context,
        path,
        body,
        "reporting",
    )
    .await
}

#[post("/engagements/{id}/close")]
async fn close_engagement(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<CloseRequest>,
) -> HttpResponse {
    let Ok(scope) = records_scope(&authority.0, &authority.1, *actor) else {
        return forbidden();
    };
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    updated_or_error(
        InternalAuditOps::close_engagement(
            &pool,
            tenant_id(tenant),
            scope,
            path.into_inner(),
            actor.into_inner(),
            context.into_inner(),
            &body,
        )
        .await,
        "Audit engagement",
    )
}

#[get("/engagements/{id}/evidence")]
async fn list_evidence(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    path: web::Path<Uuid>,
) -> HttpResponse {
    let Ok(scope) = records_scope(&authority.0, &authority.1, *actor) else {
        return forbidden();
    };
    value_or_error(
        InternalAuditOps::list_evidence(&pool, tenant_id(tenant), scope, path.into_inner())
            .await
            .map(|evidence| EvidencePage { evidence }),
    )
}

#[post("/engagements/{id}/evidence")]
async fn link_evidence(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<LinkEvidenceRequest>,
) -> HttpResponse {
    let Ok(scope) = records_scope(&authority.0, &authority.1, *actor) else {
        return forbidden();
    };
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    let restricted = authority.0.has_permission("*")
        || authority.0.has_permission("document_registry:restricted");
    match InternalAuditOps::link_evidence(
        &pool,
        tenant_id(tenant),
        scope,
        restricted,
        path.into_inner(),
        actor.into_inner(),
        context.into_inner(),
        &body,
    )
    .await
    {
        Ok(Some(value)) => created(value),
        Ok(None) => not_found("Audit engagement"),
        Err(error) => operation_error(error),
    }
}

#[get("/findings")]
async fn list_findings(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    query: web::Query<InternalAuditListQuery>,
) -> HttpResponse {
    let Ok(scope) = records_scope(&authority.0, &authority.1, *actor) else {
        return forbidden();
    };
    let (page, per_page) = bounded_page(&query);
    match InternalAuditOps::list_findings(&pool, tenant_id(tenant), scope, &query).await {
        Ok((findings, total)) => paginated(FindingsPage { findings }, page, per_page, total),
        Err(error) => operation_error(error),
    }
}

#[post("/engagements/{id}/findings")]
async fn create_finding(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<CreateFindingRequest>,
) -> HttpResponse {
    let Ok(scope) = records_scope(&authority.0, &authority.1, *actor) else {
        return forbidden();
    };
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    match InternalAuditOps::create_finding(
        &pool,
        tenant_id(tenant),
        scope,
        path.into_inner(),
        actor.into_inner(),
        context.into_inner(),
        &body,
    )
    .await
    {
        Ok(Some(value)) => created(value),
        Ok(None) => not_found("Audit engagement"),
        Err(error) => operation_error(error),
    }
}

#[get("/findings/{id}")]
async fn read_finding(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    path: web::Path<Uuid>,
) -> HttpResponse {
    let Ok(scope) = records_scope(&authority.0, &authority.1, *actor) else {
        return forbidden();
    };
    found(
        InternalAuditOps::get_finding(&pool, tenant_id(tenant), scope, path.into_inner()).await,
        "Audit finding",
    )
}

#[put("/findings/{id}")]
async fn update_finding(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<UpdateFindingRequest>,
) -> HttpResponse {
    let Ok(scope) = records_scope(&authority.0, &authority.1, *actor) else {
        return forbidden();
    };
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    updated_or_error(
        InternalAuditOps::update_finding(
            &pool,
            tenant_id(tenant),
            scope,
            path.into_inner(),
            actor.into_inner(),
            context.into_inner(),
            &body,
        )
        .await,
        "Audit finding",
    )
}

#[delete("/findings/{id}")]
async fn delete_finding(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    query: web::Query<VersionRequest>,
) -> HttpResponse {
    let Ok(scope) = records_scope(&authority.0, &authority.1, *actor) else {
        return forbidden();
    };
    if let Some(response) = validation_response(&query.0) {
        return response;
    }
    deleted_or_error(
        InternalAuditOps::delete_finding(
            &pool,
            tenant_id(tenant),
            scope,
            path.into_inner(),
            actor.into_inner(),
            context.into_inner(),
            query.expected_version,
        )
        .await,
        "Audit finding",
    )
}

#[post("/findings/{id}/issue")]
async fn issue_finding(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<VersionRequest>,
) -> HttpResponse {
    let Ok(scope) = records_scope(&authority.0, &authority.1, *actor) else {
        return forbidden();
    };
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    updated_or_error(
        InternalAuditOps::issue_finding(
            &pool,
            tenant_id(tenant),
            scope,
            path.into_inner(),
            actor.into_inner(),
            context.into_inner(),
            body.expected_version,
        )
        .await,
        "Audit finding",
    )
}

pub fn routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("")
            .wrap(RequirePermission::new("internal_audit"))
            .service(numbering_policy)
            .service(update_numbering_policy)
            .service(list_plans)
            .service(create_plan)
            .service(read_plan)
            .service(update_plan)
            .service(delete_plan)
            .service(approve_plan)
            .service(close_plan)
            .service(auditor_candidates)
            .service(list_engagements)
            .service(create_engagement)
            .service(read_engagement)
            .service(update_engagement)
            .service(delete_engagement)
            .service(start_engagement)
            .service(begin_reporting)
            .service(close_engagement)
            .service(list_evidence)
            .service(link_evidence)
            .service(list_findings)
            .service(create_finding)
            .service(read_finding)
            .service(update_finding)
            .service(delete_finding)
            .service(issue_finding),
    );
}

#[allow(clippy::too_many_arguments)]
async fn transition_response(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<VersionRequest>,
    action: &str,
) -> HttpResponse {
    let Ok(scope) = records_scope(&authority.0, &authority.1, *actor) else {
        return forbidden();
    };
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    let id = path.into_inner();
    let result = if action == "start" {
        InternalAuditOps::start_engagement(
            &pool,
            tenant_id(tenant),
            scope,
            id,
            actor.into_inner(),
            context.into_inner(),
            body.expected_version,
        )
        .await
    } else {
        InternalAuditOps::begin_reporting(
            &pool,
            tenant_id(tenant),
            scope,
            id,
            actor.into_inner(),
            context.into_inner(),
            body.expected_version,
        )
        .await
    };
    updated_or_error(result, "Audit engagement")
}

fn records_scope(
    access: &AccessContext,
    grants: &RecordScopeGrants,
    actor: AuditActor,
) -> Result<InternalAuditAccessScope, ()> {
    if access.has_permission("*") {
        return Ok(InternalAuditAccessScope::Campus);
    }
    let family = RecordScopeFamilyKey::parse("internal_audit.records").map_err(|_| ())?;
    match grants.effective_scope(&family) {
        Some(EffectiveRecordScope::Campus) => Ok(InternalAuditAccessScope::Campus),
        Some(EffectiveRecordScope::Assigned | EffectiveRecordScope::SelfAndAssigned) => {
            let user_id = actor.user_id().ok_or(())?;
            Ok(InternalAuditAccessScope::AssignedTo(user_id))
        }
        Some(EffectiveRecordScope::SelfRecord) | None => Err(()),
    }
}

fn has_plan_scope(access: &AccessContext, grants: &RecordScopeGrants) -> bool {
    if access.has_permission("*") {
        return true;
    }
    let Ok(family) = RecordScopeFamilyKey::parse("internal_audit.plans") else {
        return false;
    };
    matches!(
        grants.effective_scope(&family),
        Some(EffectiveRecordScope::Campus)
    )
}

fn tenant_id(value: web::ReqData<TenantId>) -> Uuid {
    value.into_inner().into_inner()
}

fn bounded_page(query: &InternalAuditListQuery) -> (i64, i64) {
    (
        query.page.unwrap_or(1).clamp(1, 1_000_000),
        query.per_page.unwrap_or(25).clamp(1, 100),
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

fn deleted_or_error(result: anyhow::Result<bool>, label: &str) -> HttpResponse {
    match result {
        Ok(true) => ok(serde_json::json!({"deleted":true})),
        Ok(false) => not_found(label),
        Err(error) => operation_error(error),
    }
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

fn found<T: Serialize>(result: anyhow::Result<Option<T>>, label: &str) -> HttpResponse {
    updated_or_error(result, label)
}

fn forbidden() -> HttpResponse {
    HttpResponse::Forbidden().json(ApiResponse::from_status(
        StatusCode::FORBIDDEN,
        None::<()>,
        Some(vec![
            "Internal Audit access is outside your assigned scope".to_string(),
        ]),
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
    if message.contains("changed") || message.contains("already") {
        return HttpResponse::Conflict().json(ApiResponse::from_status(
            StatusCode::CONFLICT,
            None::<()>,
            Some(vec![message]),
        ));
    }
    let operational = [
        "An ",
        "Only ",
        "Close ",
        "Issue ",
        "Link ",
        "Evidence ",
        "Findings ",
        "Engagements ",
        "The selected ",
        "The governed ",
        "Internal Audit numbering ",
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
            Some(vec![
                "Internal Audit could not complete the request".to_string(),
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

    use super::{has_plan_scope, records_scope};

    #[test]
    fn assigned_auditors_do_not_gain_campus_record_scope() {
        let family = RecordScopeFamilyKey::parse("internal_audit.records")
            .unwrap_or_else(|_| unreachable!());
        let grants = RecordScopeGrants::from_grants([RecordScopeGrant::new(
            family,
            RecordScopeKind::Assigned,
        )]);
        let access = AccessContext {
            role_keys: vec!["internal_auditor".to_string()],
            permissions: vec!["internal_audit:view".to_string()],
            enabled_modules: vec!["internal_audit".to_string()],
            entitlements: EntitlementSnapshot::new(
                LeaseLifecycle::Active,
                [(
                    "internal_audit".to_string(),
                    ModuleEntitlementState::Enabled,
                )],
                [],
            )
            .unwrap_or_else(|_| unreachable!()),
        };
        let account_id = Uuid::new_v4();
        assert_eq!(
            records_scope(&access, &grants, AuditActor::person(account_id),),
            Ok(crate::InternalAuditAccessScope::AssignedTo(account_id))
        );
    }

    #[test]
    fn plan_scope_requires_an_explicit_campus_grant() {
        let access = AccessContext {
            role_keys: vec!["internal_auditor".to_string()],
            permissions: vec!["internal_audit:view".to_string()],
            enabled_modules: vec!["internal_audit".to_string()],
            entitlements: EntitlementSnapshot::new(
                LeaseLifecycle::Active,
                [(
                    "internal_audit".to_string(),
                    ModuleEntitlementState::Enabled,
                )],
                [],
            )
            .unwrap_or_else(|_| unreachable!()),
        };
        assert!(!has_plan_scope(&access, &RecordScopeGrants::empty()));
    }
}

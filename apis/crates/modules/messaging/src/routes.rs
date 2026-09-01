//! Authenticated Communication routes over licensed, scoped operations.

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
    AnnouncementListQuery, AnnouncementsPage, CommunicationAccessScope, CommunicationOps,
    CreateAnnouncementRequest, DeleteAnnouncementQuery, InboxListQuery, InboxPage,
    ReasonedVersionRequest, UpdateAnnouncementRequest, VersionRequest,
};

type Authority = (web::ReqData<AccessContext>, web::ReqData<RecordScopeGrants>);

#[get("/references")]
async fn references(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
) -> HttpResponse {
    let Ok(scope) = communication_scope(&authority.0, &authority.1, actor.into_inner()) else {
        return forbidden();
    };
    match CommunicationOps::reference_data(pool.get_ref(), tenant_id(tenant), scope).await {
        Ok(data) => ok(data),
        Err(_) => internal_error(),
    }
}

#[get("/announcements")]
async fn list_announcements(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    query: web::Query<AnnouncementListQuery>,
) -> HttpResponse {
    let Ok(scope) = communication_scope(&authority.0, &authority.1, actor.into_inner()) else {
        return forbidden();
    };
    let (page, per_page) = bounded_page(query.page, query.per_page);
    match CommunicationOps::list(pool.get_ref(), tenant_id(tenant), scope, &query).await {
        Ok((announcements, total)) => HttpResponse::Ok().json(ApiResponse::with_pagination(
            StatusCode::OK,
            Some(AnnouncementsPage { announcements }),
            PaginationMeta::new(page as u32, per_page as u32, total),
            None,
        )),
        Err(error) => operation_error(error),
    }
}

#[post("/announcements")]
async fn create_announcement(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    request_context: web::ReqData<RequestContext>,
    body: web::Json<CreateAnnouncementRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    let actor_value = actor.into_inner();
    let Ok(scope) = communication_scope(&authority.0, &authority.1, actor_value) else {
        return forbidden();
    };
    match CommunicationOps::create(
        pool.get_ref(),
        tenant_id(tenant),
        scope,
        actor_value,
        request_context.into_inner(),
        &body,
    )
    .await
    {
        Ok(value) => HttpResponse::Created().json(ApiResponse::from_status(
            StatusCode::CREATED,
            Some(value),
            None,
        )),
        Err(error) => operation_error(error),
    }
}

#[get("/announcements/{id}")]
async fn read_announcement(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    path: web::Path<Uuid>,
) -> HttpResponse {
    let Ok(scope) = communication_scope(&authority.0, &authority.1, actor.into_inner()) else {
        return forbidden();
    };
    match CommunicationOps::get(pool.get_ref(), tenant_id(tenant), path.into_inner(), scope).await {
        Ok(Some(value)) => ok(value),
        Ok(None) => not_found("Announcement not found"),
        Err(_) => internal_error(),
    }
}

#[put("/announcements/{id}")]
async fn update_announcement(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    request_context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<UpdateAnnouncementRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    let actor_value = actor.into_inner();
    let Ok(scope) = communication_scope(&authority.0, &authority.1, actor_value) else {
        return forbidden();
    };
    match CommunicationOps::update(
        pool.get_ref(),
        tenant_id(tenant),
        path.into_inner(),
        scope,
        actor_value,
        request_context.into_inner(),
        &body,
    )
    .await
    {
        Ok(Some(value)) => ok(value),
        Ok(None) => not_found("Announcement not found"),
        Err(error) => operation_error(error),
    }
}

#[get("/announcements/{id}/audience-preview")]
async fn preview_audience(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    path: web::Path<Uuid>,
) -> HttpResponse {
    let Ok(scope) = communication_scope(&authority.0, &authority.1, actor.into_inner()) else {
        return forbidden();
    };
    match CommunicationOps::audience_preview(
        pool.get_ref(),
        tenant_id(tenant),
        path.into_inner(),
        scope,
    )
    .await
    {
        Ok(Some(value)) => ok(value),
        Ok(None) => not_found("Announcement not found"),
        Err(error) => operation_error(error),
    }
}

#[post("/announcements/{id}/submit")]
async fn submit_announcement(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    request_context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<VersionRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    let actor_value = actor.into_inner();
    let Ok(scope) = communication_scope(&authority.0, &authority.1, actor_value) else {
        return forbidden();
    };
    match CommunicationOps::submit(
        pool.get_ref(),
        tenant_id(tenant),
        path.into_inner(),
        scope,
        actor_value,
        request_context.into_inner(),
        body.expected_version,
    )
    .await
    {
        Ok(Some(value)) => ok(value),
        Ok(None) => not_found("Announcement not found"),
        Err(error) => operation_error(error),
    }
}

#[post("/announcements/{id}/reopen")]
async fn reopen_announcement(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    request_context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<ReasonedVersionRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    let actor_value = actor.into_inner();
    let Ok(scope) = communication_scope(&authority.0, &authority.1, actor_value) else {
        return forbidden();
    };
    match CommunicationOps::reopen(
        pool.get_ref(),
        tenant_id(tenant),
        path.into_inner(),
        scope,
        actor_value,
        request_context.into_inner(),
        &body,
    )
    .await
    {
        Ok(Some(value)) => ok(value),
        Ok(None) => not_found("Announcement not found"),
        Err(error) => operation_error(error),
    }
}

#[post("/announcements/{id}/publish")]
async fn publish_announcement(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    request_context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<VersionRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    let actor_value = actor.into_inner();
    if !campus_scope(&authority.0, &authority.1, actor_value) {
        return forbidden();
    }
    match CommunicationOps::publish(
        pool.get_ref(),
        tenant_id(tenant),
        path.into_inner(),
        actor_value,
        request_context.into_inner(),
        body.expected_version,
    )
    .await
    {
        Ok(Some(value)) => ok(value),
        Ok(None) => not_found("Announcement not found"),
        Err(error) => operation_error(error),
    }
}

#[post("/announcements/{id}/cancel")]
async fn cancel_announcement(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    request_context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<ReasonedVersionRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    let actor_value = actor.into_inner();
    if !campus_scope(&authority.0, &authority.1, actor_value) {
        return forbidden();
    }
    match CommunicationOps::cancel(
        pool.get_ref(),
        tenant_id(tenant),
        path.into_inner(),
        actor_value,
        request_context.into_inner(),
        body.expected_version,
        &body.reason,
    )
    .await
    {
        Ok(Some(value)) => ok(value),
        Ok(None) => not_found("Announcement not found"),
        Err(error) => operation_error(error),
    }
}

#[delete("/announcements/{id}")]
async fn delete_announcement(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    request_context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    query: web::Query<DeleteAnnouncementQuery>,
) -> HttpResponse {
    if let Some(response) = validation_response(&query.0) {
        return response;
    }
    let actor_value = actor.into_inner();
    let Ok(scope) = communication_scope(&authority.0, &authority.1, actor_value) else {
        return forbidden();
    };
    match CommunicationOps::delete(
        pool.get_ref(),
        tenant_id(tenant),
        path.into_inner(),
        scope,
        actor_value,
        request_context.into_inner(),
        query.expected_version,
    )
    .await
    {
        Ok(true) => ok(serde_json::json!({"deleted": true})),
        Ok(false) => not_found("Announcement not found"),
        Err(error) => operation_error(error),
    }
}

#[get("/announcements/{id}/deliveries")]
async fn delivery_history(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    path: web::Path<Uuid>,
) -> HttpResponse {
    if !campus_scope(&authority.0, &authority.1, actor.into_inner()) {
        return forbidden();
    }
    match CommunicationOps::deliveries(pool.get_ref(), tenant_id(tenant), path.into_inner()).await {
        Ok(value) => ok(value),
        Err(_) => internal_error(),
    }
}

#[get("/inbox")]
async fn inbox(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    actor: web::ReqData<AuditActor>,
    query: web::Query<InboxListQuery>,
) -> HttpResponse {
    let Some(user_id) = actor.user_id() else {
        return forbidden();
    };
    let (page, per_page) = bounded_page(query.page, query.per_page);
    match CommunicationOps::inbox(pool.get_ref(), tenant_id(tenant), user_id, &query).await {
        Ok((messages, total)) => HttpResponse::Ok().json(ApiResponse::with_pagination(
            StatusCode::OK,
            Some(InboxPage { messages }),
            PaginationMeta::new(page as u32, per_page as u32, total),
            None,
        )),
        Err(_) => internal_error(),
    }
}

#[get("/inbox/{id}")]
async fn read_inbox_message(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    actor: web::ReqData<AuditActor>,
    path: web::Path<Uuid>,
) -> HttpResponse {
    let Some(user_id) = actor.user_id() else {
        return forbidden();
    };
    match CommunicationOps::inbox_message(
        pool.get_ref(),
        tenant_id(tenant),
        user_id,
        path.into_inner(),
    )
    .await
    {
        Ok(Some(value)) => ok(value),
        Ok(None) => not_found("Message not found"),
        Err(_) => internal_error(),
    }
}

#[post("/inbox/{id}/read")]
async fn mark_inbox_read(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    actor: web::ReqData<AuditActor>,
    request_context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
) -> HttpResponse {
    let actor_value = actor.into_inner();
    let Some(user_id) = actor_value.user_id() else {
        return forbidden();
    };
    match CommunicationOps::mark_read(
        pool.get_ref(),
        tenant_id(tenant),
        user_id,
        path.into_inner(),
        actor_value,
        request_context.into_inner(),
    )
    .await
    {
        Ok(Some(value)) => ok(value),
        Ok(None) => not_found("Message not found"),
        Err(error) => operation_error(error),
    }
}

pub fn routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("")
            .wrap(RequirePermission::new("messaging"))
            .service(references)
            .service(list_announcements)
            .service(create_announcement)
            .service(read_announcement)
            .service(update_announcement)
            .service(preview_audience)
            .service(submit_announcement)
            .service(reopen_announcement)
            .service(publish_announcement)
            .service(cancel_announcement)
            .service(delete_announcement)
            .service(delivery_history)
            .service(inbox)
            .service(read_inbox_message)
            .service(mark_inbox_read),
    );
}

fn communication_scope(
    access: &AccessContext,
    grants: &RecordScopeGrants,
    actor: AuditActor,
) -> Result<CommunicationAccessScope, ()> {
    if access.has_permission("*") {
        return Ok(CommunicationAccessScope::Campus);
    }
    let family = RecordScopeFamilyKey::parse("messaging.announcements").map_err(|_| ())?;
    let user_id = actor.user_id().ok_or(())?;
    match grants.effective_scope(&family) {
        Some(EffectiveRecordScope::Campus) => Ok(CommunicationAccessScope::Campus),
        Some(EffectiveRecordScope::Assigned | EffectiveRecordScope::SelfAndAssigned) => {
            Ok(CommunicationAccessScope::AssignedTo(user_id))
        }
        Some(EffectiveRecordScope::SelfRecord) => Ok(CommunicationAccessScope::SelfFor(user_id)),
        None => Err(()),
    }
}
fn campus_scope(access: &AccessContext, grants: &RecordScopeGrants, actor: AuditActor) -> bool {
    matches!(
        communication_scope(access, grants, actor),
        Ok(CommunicationAccessScope::Campus)
    )
}
fn tenant_id(value: web::ReqData<TenantId>) -> Uuid {
    value.into_inner().into_inner()
}
fn bounded_page(page: Option<i64>, per_page: Option<i64>) -> (i64, i64) {
    (
        page.unwrap_or(1).max(1),
        per_page.unwrap_or(25).clamp(1, 100),
    )
}
fn ok<T: Serialize>(value: T) -> HttpResponse {
    HttpResponse::Ok().json(ApiResponse::from_status(StatusCode::OK, Some(value), None))
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
        Some(vec!["This communication record is unavailable".to_string()]),
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
    if message.contains("changed") {
        return HttpResponse::Conflict().json(ApiResponse::from_status(
            StatusCode::CONFLICT,
            None::<()>,
            Some(vec![message]),
        ));
    }
    if message.starts_with("This ")
        || message.starts_with("The ")
        || message.starts_with("At least ")
        || message.starts_with("Each ")
        || message.ends_with(" is required")
    {
        return HttpResponse::BadRequest().json(ApiResponse::from_status(
            StatusCode::BAD_REQUEST,
            None::<()>,
            Some(vec![message]),
        ));
    }
    internal_error()
}
fn internal_error() -> HttpResponse {
    HttpResponse::InternalServerError().json(ApiResponse::from_status(
        StatusCode::INTERNAL_SERVER_ERROR,
        None::<()>,
        Some(vec![
            "Communication could not complete the request.".to_string(),
        ]),
    ))
}

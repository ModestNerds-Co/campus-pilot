//! Authenticated, licensed, permission-authoritative Activities routes.

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

use crate::ops::GroupTransition;
use crate::{
    ActivitiesOps, ActivitiesScope, ActivityCatalogQuery, ActivityCatalogStatus,
    ActivityGroupQuery, ActivityGroupsPage, ActivityReferenceQuery, ActivitySessionQuery,
    ActivitySessionsPage, ActivityTransitionRequest, AddActivityLeaderRequest,
    AddActivityMembershipRequest, ArchiveActivityCatalogItemRequest, CancelActivitySessionRequest,
    CompleteActivitySessionRequest, CreateActivityCatalogItemRequest, CreateActivityGroupRequest,
    CreateActivitySessionRequest, EndActivityLeaderRequest, EndActivityMembershipRequest,
    MarkActivityParticipationRequest, UpdateActivityCatalogItemRequest, UpdateActivityGroupRequest,
    UpdateActivityMembershipRequest, UpdateActivitySessionRequest,
};

#[get("/catalog")]
async fn list_catalog(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    access: web::ReqData<AccessContext>,
    query: web::Query<ActivityCatalogQuery>,
) -> HttpResponse {
    if !authorised(&access, "activities:view") {
        return forbidden("view the activity catalog");
    }
    let query = ActivityCatalogQuery {
        search: query.search.clone(),
        category: query.category,
        status: if authorised(&access, "activities:manage") {
            query.status
        } else {
            Some(ActivityCatalogStatus::Active)
        },
    };
    value_or_error(ActivitiesOps::list_catalog(&pool, tenant_id(tenant), &query).await)
}

#[post("/catalog")]
async fn create_catalog_item(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    access: web::ReqData<AccessContext>,
    actor: web::ReqData<AuditActor>,
    context: web::ReqData<RequestContext>,
    body: web::Json<CreateActivityCatalogItemRequest>,
) -> HttpResponse {
    if !authorised(&access, "activities:manage") {
        return forbidden("create catalog activities");
    }
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    created_or_error(
        ActivitiesOps::create_catalog_item(
            &pool,
            tenant_id(tenant),
            actor.into_inner(),
            context.into_inner(),
            &body,
        )
        .await,
    )
}

#[get("/catalog/{id}")]
async fn read_catalog_item(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    access: web::ReqData<AccessContext>,
    path: web::Path<Uuid>,
) -> HttpResponse {
    if !authorised(&access, "activities:view") {
        return forbidden("view catalog activities");
    }
    found(
        ActivitiesOps::get_catalog_item(&pool, tenant_id(tenant), path.into_inner()).await,
        "Activity",
    )
}

#[put("/catalog/{id}")]
async fn update_catalog_item(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    access: web::ReqData<AccessContext>,
    actor: web::ReqData<AuditActor>,
    context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<UpdateActivityCatalogItemRequest>,
) -> HttpResponse {
    if !authorised(&access, "activities:manage") {
        return forbidden("change catalog activities");
    }
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    updated_or_error(
        ActivitiesOps::update_catalog_item(
            &pool,
            tenant_id(tenant),
            path.into_inner(),
            actor.into_inner(),
            context.into_inner(),
            &body,
        )
        .await,
        "Activity",
    )
}

#[post("/catalog/{id}/archive")]
async fn archive_catalog_item(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    access: web::ReqData<AccessContext>,
    actor: web::ReqData<AuditActor>,
    context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<ArchiveActivityCatalogItemRequest>,
) -> HttpResponse {
    if !authorised(&access, "activities:manage") {
        return forbidden("archive catalog activities");
    }
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    updated_or_error(
        ActivitiesOps::archive_catalog_item(
            &pool,
            tenant_id(tenant),
            path.into_inner(),
            actor.into_inner(),
            context.into_inner(),
            &body,
        )
        .await,
        "Activity",
    )
}

#[get("/references")]
async fn reference_data(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    access: web::ReqData<AccessContext>,
    query: web::Query<ActivityReferenceQuery>,
) -> HttpResponse {
    if !authorised(&access, "activities:manage") {
        return forbidden("search learner and employee references");
    }
    value_or_error(ActivitiesOps::reference_data(&pool, tenant_id(tenant), &query).await)
}

#[get("/groups")]
async fn list_groups(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    access: web::ReqData<AccessContext>,
    grants: web::ReqData<RecordScopeGrants>,
    actor: web::ReqData<AuditActor>,
    query: web::Query<ActivityGroupQuery>,
) -> HttpResponse {
    if !authorised(&access, "activities:view") {
        return forbidden("view activity groups");
    }
    let Some(scope) = record_scope(&access, &grants, actor.into_inner(), "activities.groups")
    else {
        return forbidden("view activity groups outside your record scope");
    };
    let (page, per_page) = bounded_page(query.page, query.per_page);
    match ActivitiesOps::list_groups(&pool, tenant_id(tenant), scope, &query).await {
        Ok((groups, total)) => paginated(ActivityGroupsPage { groups }, page, per_page, total),
        Err(error) => operation_error(error),
    }
}

#[post("/groups")]
async fn create_group(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    access: web::ReqData<AccessContext>,
    actor: web::ReqData<AuditActor>,
    context: web::ReqData<RequestContext>,
    body: web::Json<CreateActivityGroupRequest>,
) -> HttpResponse {
    if !authorised(&access, "activities:manage") {
        return forbidden("create activity groups");
    }
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    created_or_error(
        ActivitiesOps::create_group(
            &pool,
            tenant_id(tenant),
            actor.into_inner(),
            context.into_inner(),
            &body,
        )
        .await,
    )
}

#[get("/groups/{id}")]
async fn read_group(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    access: web::ReqData<AccessContext>,
    grants: web::ReqData<RecordScopeGrants>,
    actor: web::ReqData<AuditActor>,
    path: web::Path<Uuid>,
) -> HttpResponse {
    if !authorised(&access, "activities:view") {
        return forbidden("view activity groups");
    }
    let Some(scope) = record_scope(&access, &grants, actor.into_inner(), "activities.groups")
    else {
        return forbidden("view activity groups outside your record scope");
    };
    found(
        ActivitiesOps::get_group(&pool, tenant_id(tenant), scope, path.into_inner()).await,
        "Activity group",
    )
}

#[put("/groups/{id}")]
async fn update_group(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    access: web::ReqData<AccessContext>,
    actor: web::ReqData<AuditActor>,
    context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<UpdateActivityGroupRequest>,
) -> HttpResponse {
    if !authorised(&access, "activities:manage") {
        return forbidden("change activity groups");
    }
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    updated_or_error(
        ActivitiesOps::update_group(
            &pool,
            tenant_id(tenant),
            path.into_inner(),
            actor.into_inner(),
            context.into_inner(),
            &body,
        )
        .await,
        "Activity group",
    )
}

async fn group_transition(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    access: web::ReqData<AccessContext>,
    actor: web::ReqData<AuditActor>,
    context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<ActivityTransitionRequest>,
    transition: GroupTransition,
) -> HttpResponse {
    if !authorised(&access, "activities:manage") {
        return forbidden("change the activity group lifecycle");
    }
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    updated_or_error(
        ActivitiesOps::transition_group(
            &pool,
            tenant_id(tenant),
            path.into_inner(),
            actor.into_inner(),
            context.into_inner(),
            &body,
            transition,
        )
        .await,
        "Activity group",
    )
}

#[post("/groups/{id}/activate")]
async fn activate_group(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    access: web::ReqData<AccessContext>,
    actor: web::ReqData<AuditActor>,
    context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<ActivityTransitionRequest>,
) -> HttpResponse {
    group_transition(
        pool,
        tenant,
        access,
        actor,
        context,
        path,
        body,
        GroupTransition::Activate,
    )
    .await
}

#[post("/groups/{id}/close")]
async fn close_group(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    access: web::ReqData<AccessContext>,
    actor: web::ReqData<AuditActor>,
    context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<ActivityTransitionRequest>,
) -> HttpResponse {
    group_transition(
        pool,
        tenant,
        access,
        actor,
        context,
        path,
        body,
        GroupTransition::Close,
    )
    .await
}

#[post("/groups/{id}/cancel")]
async fn cancel_group(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    access: web::ReqData<AccessContext>,
    actor: web::ReqData<AuditActor>,
    context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<ActivityTransitionRequest>,
) -> HttpResponse {
    group_transition(
        pool,
        tenant,
        access,
        actor,
        context,
        path,
        body,
        GroupTransition::Cancel,
    )
    .await
}

#[post("/groups/{id}/leaders")]
async fn add_leader(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    access: web::ReqData<AccessContext>,
    actor: web::ReqData<AuditActor>,
    context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<AddActivityLeaderRequest>,
) -> HttpResponse {
    if !authorised(&access, "activities:manage") {
        return forbidden("assign activity leaders");
    }
    updated_or_error(
        ActivitiesOps::add_leader(
            &pool,
            tenant_id(tenant),
            path.into_inner(),
            actor.into_inner(),
            context.into_inner(),
            &body,
        )
        .await,
        "Activity group",
    )
}

#[post("/groups/{group_id}/leaders/{leader_id}/end")]
async fn end_leader(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    access: web::ReqData<AccessContext>,
    actor: web::ReqData<AuditActor>,
    context: web::ReqData<RequestContext>,
    path: web::Path<(Uuid, Uuid)>,
    body: web::Json<EndActivityLeaderRequest>,
) -> HttpResponse {
    if !authorised(&access, "activities:manage") {
        return forbidden("end activity leader assignments");
    }
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    let (group_id, leader_id) = path.into_inner();
    updated_or_error(
        ActivitiesOps::end_leader(
            &pool,
            tenant_id(tenant),
            group_id,
            leader_id,
            actor.into_inner(),
            context.into_inner(),
            &body,
        )
        .await,
        "Activity leader",
    )
}

#[post("/groups/{id}/members")]
async fn add_member(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    access: web::ReqData<AccessContext>,
    actor: web::ReqData<AuditActor>,
    context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<AddActivityMembershipRequest>,
) -> HttpResponse {
    if !authorised(&access, "activities:manage") {
        return forbidden("add activity members");
    }
    updated_or_error(
        ActivitiesOps::add_membership(
            &pool,
            tenant_id(tenant),
            path.into_inner(),
            actor.into_inner(),
            context.into_inner(),
            &body,
        )
        .await,
        "Activity group",
    )
}

#[put("/groups/{group_id}/members/{membership_id}")]
async fn update_member(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    access: web::ReqData<AccessContext>,
    actor: web::ReqData<AuditActor>,
    context: web::ReqData<RequestContext>,
    path: web::Path<(Uuid, Uuid)>,
    body: web::Json<UpdateActivityMembershipRequest>,
) -> HttpResponse {
    if !authorised(&access, "activities:manage") {
        return forbidden("change activity membership consent");
    }
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    let (group_id, membership_id) = path.into_inner();
    updated_or_error(
        ActivitiesOps::update_membership(
            &pool,
            tenant_id(tenant),
            group_id,
            membership_id,
            actor.into_inner(),
            context.into_inner(),
            &body,
        )
        .await,
        "Activity membership",
    )
}

#[post("/groups/{group_id}/members/{membership_id}/end")]
async fn end_member(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    access: web::ReqData<AccessContext>,
    actor: web::ReqData<AuditActor>,
    context: web::ReqData<RequestContext>,
    path: web::Path<(Uuid, Uuid)>,
    body: web::Json<EndActivityMembershipRequest>,
) -> HttpResponse {
    if !authorised(&access, "activities:manage") {
        return forbidden("end activity memberships");
    }
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    let (group_id, membership_id) = path.into_inner();
    updated_or_error(
        ActivitiesOps::end_membership(
            &pool,
            tenant_id(tenant),
            group_id,
            membership_id,
            actor.into_inner(),
            context.into_inner(),
            &body,
        )
        .await,
        "Activity membership",
    )
}

#[get("/sessions")]
async fn list_sessions(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    access: web::ReqData<AccessContext>,
    grants: web::ReqData<RecordScopeGrants>,
    actor: web::ReqData<AuditActor>,
    query: web::Query<ActivitySessionQuery>,
) -> HttpResponse {
    if !authorised(&access, "activities:view") {
        return forbidden("view activity sessions");
    }
    let Some(scope) = record_scope(&access, &grants, actor.into_inner(), "activities.sessions")
    else {
        return forbidden("view activity sessions outside your record scope");
    };
    let (page, per_page) = bounded_page(query.page, query.per_page);
    match ActivitiesOps::list_sessions(&pool, tenant_id(tenant), scope, &query).await {
        Ok((sessions, total)) => {
            paginated(ActivitySessionsPage { sessions }, page, per_page, total)
        }
        Err(error) => operation_error(error),
    }
}

#[post("/sessions")]
async fn create_session(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    access: web::ReqData<AccessContext>,
    actor: web::ReqData<AuditActor>,
    context: web::ReqData<RequestContext>,
    body: web::Json<CreateActivitySessionRequest>,
) -> HttpResponse {
    if !authorised(&access, "activities:operate") {
        return forbidden("create activity sessions");
    }
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    let actor = actor.into_inner();
    let Some(scope) = session_write_scope(&access, actor) else {
        return forbidden("create sessions outside your assigned groups");
    };
    created_or_error(
        ActivitiesOps::create_session(
            &pool,
            tenant_id(tenant),
            scope,
            actor,
            context.into_inner(),
            &body,
        )
        .await,
    )
}

#[get("/sessions/{id}")]
async fn read_session(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    access: web::ReqData<AccessContext>,
    grants: web::ReqData<RecordScopeGrants>,
    actor: web::ReqData<AuditActor>,
    path: web::Path<Uuid>,
) -> HttpResponse {
    if !authorised(&access, "activities:view") {
        return forbidden("view activity sessions");
    }
    let Some(scope) = record_scope(&access, &grants, actor.into_inner(), "activities.sessions")
    else {
        return forbidden("view sessions outside your record scope");
    };
    found(
        ActivitiesOps::get_session(&pool, tenant_id(tenant), scope, path.into_inner()).await,
        "Activity session",
    )
}

#[put("/sessions/{id}")]
async fn update_session(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    access: web::ReqData<AccessContext>,
    actor: web::ReqData<AuditActor>,
    context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<UpdateActivitySessionRequest>,
) -> HttpResponse {
    if !authorised(&access, "activities:operate") {
        return forbidden("change activity sessions");
    }
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    let actor = actor.into_inner();
    let Some(scope) = session_write_scope(&access, actor) else {
        return forbidden("change sessions outside your assigned groups");
    };
    updated_or_error(
        ActivitiesOps::update_session(
            &pool,
            tenant_id(tenant),
            scope,
            path.into_inner(),
            actor,
            context.into_inner(),
            &body,
        )
        .await,
        "Activity session",
    )
}

#[put("/sessions/{session_id}/participation/{membership_id}")]
async fn mark_participation(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    access: web::ReqData<AccessContext>,
    actor: web::ReqData<AuditActor>,
    context: web::ReqData<RequestContext>,
    path: web::Path<(Uuid, Uuid)>,
    body: web::Json<MarkActivityParticipationRequest>,
) -> HttpResponse {
    if !authorised(&access, "activities:operate") {
        return forbidden("mark activity participation");
    }
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    let actor = actor.into_inner();
    let Some(scope) = session_write_scope(&access, actor) else {
        return forbidden("mark sessions outside your assigned groups");
    };
    let (session_id, membership_id) = path.into_inner();
    updated_or_error(
        ActivitiesOps::mark_participation(
            &pool,
            tenant_id(tenant),
            scope,
            session_id,
            membership_id,
            actor,
            context.into_inner(),
            &body,
        )
        .await,
        "Activity session",
    )
}

#[post("/sessions/{id}/complete")]
async fn complete_session(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    access: web::ReqData<AccessContext>,
    actor: web::ReqData<AuditActor>,
    context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<CompleteActivitySessionRequest>,
) -> HttpResponse {
    if !authorised(&access, "activities:operate") {
        return forbidden("complete activity sessions");
    }
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    let actor = actor.into_inner();
    let Some(scope) = session_write_scope(&access, actor) else {
        return forbidden("complete sessions outside your assigned groups");
    };
    updated_or_error(
        ActivitiesOps::complete_session(
            &pool,
            tenant_id(tenant),
            scope,
            path.into_inner(),
            actor,
            context.into_inner(),
            &body,
        )
        .await,
        "Activity session",
    )
}

#[post("/sessions/{id}/cancel")]
async fn cancel_session(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    access: web::ReqData<AccessContext>,
    actor: web::ReqData<AuditActor>,
    context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<CancelActivitySessionRequest>,
) -> HttpResponse {
    if !authorised(&access, "activities:operate") {
        return forbidden("cancel activity sessions");
    }
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    let actor = actor.into_inner();
    let Some(scope) = session_write_scope(&access, actor) else {
        return forbidden("cancel sessions outside your assigned groups");
    };
    updated_or_error(
        ActivitiesOps::cancel_session(
            &pool,
            tenant_id(tenant),
            scope,
            path.into_inner(),
            actor,
            context.into_inner(),
            &body,
        )
        .await,
        "Activity session",
    )
}

pub fn routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("")
            .wrap(RequirePermission::new("activities"))
            .service(list_catalog)
            .service(create_catalog_item)
            .service(read_catalog_item)
            .service(update_catalog_item)
            .service(archive_catalog_item)
            .service(reference_data)
            .service(list_groups)
            .service(create_group)
            .service(read_group)
            .service(update_group)
            .service(activate_group)
            .service(close_group)
            .service(cancel_group)
            .service(add_leader)
            .service(end_leader)
            .service(add_member)
            .service(update_member)
            .service(end_member)
            .service(list_sessions)
            .service(create_session)
            .service(read_session)
            .service(update_session)
            .service(mark_participation)
            .service(complete_session)
            .service(cancel_session),
    );
}

fn record_scope(
    access: &AccessContext,
    grants: &RecordScopeGrants,
    actor: AuditActor,
    family: &str,
) -> Option<ActivitiesScope> {
    if access.has_permission("*") {
        return Some(ActivitiesScope::Campus);
    }
    let family = RecordScopeFamilyKey::parse(family).ok()?;
    match grants.effective_scope(&family) {
        Some(EffectiveRecordScope::Campus) => Some(ActivitiesScope::Campus),
        Some(EffectiveRecordScope::SelfRecord) => actor.user_id().map(ActivitiesScope::SelfAccount),
        Some(EffectiveRecordScope::Assigned) => {
            actor.user_id().map(ActivitiesScope::AssignedAccount)
        }
        Some(EffectiveRecordScope::SelfAndAssigned) => {
            actor.user_id().map(ActivitiesScope::SelfAndAssigned)
        }
        None => None,
    }
}

fn session_write_scope(access: &AccessContext, actor: AuditActor) -> Option<ActivitiesScope> {
    if access.has_permission("*") || access.has_permission("activities:manage") {
        return Some(ActivitiesScope::Campus);
    }
    if access.has_permission("activities:operate") {
        return actor.user_id().map(ActivitiesScope::AssignedAccount);
    }
    None
}

fn authorised(access: &AccessContext, permission: &str) -> bool {
    access.has_permission("*") || access.has_permission(permission)
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
            "Your current Activities access does not allow you to {action}"
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
        "The ",
        "This ",
        "Activity ",
        "Activities ",
        "Assign ",
        "Add ",
        "Close ",
        "Complete ",
        "Mark ",
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
                "Activities could not complete the request".to_string(),
            ]),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::{record_scope, session_write_scope};
    use crate::ActivitiesScope;
    use cp_audit::AuditActor;
    use cp_common::{
        AccessContext, EntitlementSnapshot, LeaseLifecycle, ModuleEntitlementState,
        RecordScopeFamilyKey, RecordScopeGrant, RecordScopeGrants, RecordScopeKind,
    };
    use uuid::Uuid;

    fn access(permissions: &[&str]) -> AccessContext {
        AccessContext {
            role_keys: vec![],
            permissions: permissions.iter().map(|value| value.to_string()).collect(),
            enabled_modules: vec!["activities".to_string()],
            entitlements: EntitlementSnapshot::new(
                LeaseLifecycle::Active,
                vec![("activities".to_string(), ModuleEntitlementState::Enabled)],
                vec![],
            )
            .unwrap(),
        }
    }
    fn grants(kind: RecordScopeKind) -> RecordScopeGrants {
        RecordScopeGrants::from_grants([RecordScopeGrant::new(
            RecordScopeFamilyKey::parse("activities.groups").unwrap(),
            kind,
        )])
    }
    #[test]
    fn activities_scopes_remain_role_and_person_bound() {
        let user_id = Uuid::new_v4();
        let actor = AuditActor::person(user_id);
        assert_eq!(
            record_scope(
                &access(&["activities:view"]),
                &grants(RecordScopeKind::SelfRecord),
                actor,
                "activities.groups"
            ),
            Some(ActivitiesScope::SelfAccount(user_id))
        );
        assert_eq!(
            record_scope(
                &access(&["activities:view"]),
                &grants(RecordScopeKind::Assigned),
                actor,
                "activities.groups"
            ),
            Some(ActivitiesScope::AssignedAccount(user_id))
        );
        assert_eq!(
            record_scope(
                &access(&["*"]),
                &RecordScopeGrants::empty(),
                actor,
                "activities.groups"
            ),
            Some(ActivitiesScope::Campus)
        );
        assert_eq!(
            session_write_scope(&access(&["activities:view"]), actor),
            None
        );
        assert_eq!(
            session_write_scope(&access(&["activities:view", "activities:operate"]), actor),
            Some(ActivitiesScope::AssignedAccount(user_id))
        );
        assert_eq!(
            session_write_scope(
                &access(&["activities:view", "activities:operate", "activities:manage"]),
                actor
            ),
            Some(ActivitiesScope::Campus)
        );
    }
}

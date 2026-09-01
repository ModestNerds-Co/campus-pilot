//! Authenticated, licensed, and record-scoped Hostel HTTP routes.

use actix_web::http::StatusCode;
use actix_web::{HttpResponse, get, post, put, web};
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
    ActivateAllocationRequest, AllocationPreviewRequest, AllocationsPage, CancelAllocationRequest,
    CreateAllocationRequest, CreatePastoralRecordRequest, CreateResidenceRequest,
    CreateRoomRequest, EndAllocationRequest, HostelAccessScope, HostelListQuery, HostelOps,
    PastoralRecordsPage, ResidencesPage, ResolvePastoralRecordRequest, RoomsPage,
    TransferAllocationPreviewRequest, TransferAllocationRequest, UpdatePastoralRecordRequest,
    UpdateResidenceRequest, UpdateRoomRequest,
};

type Authority = (web::ReqData<AccessContext>, web::ReqData<RecordScopeGrants>);

#[get("/references")]
async fn references(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    query: web::Query<HostelListQuery>,
) -> HttpResponse {
    if !is_campus_scope(&authority, actor.into_inner(), "hostel.occupancy") {
        return forbidden();
    }
    value_or_error(
        HostelOps::reference_data(
            pool.get_ref(),
            tenant_id(tenant),
            trimmed(query.search.as_deref()),
        )
        .await,
    )
}

#[get("/residences")]
async fn list_residences(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    query: web::Query<HostelListQuery>,
) -> HttpResponse {
    if !is_campus_scope(&authority, actor.into_inner(), "hostel.occupancy") {
        return forbidden();
    }
    let (page, per_page) = bounded_page(&query);
    match HostelOps::list_residences(pool.get_ref(), tenant_id(tenant), &query).await {
        Ok((residences, total)) => paginated(ResidencesPage { residences }, page, per_page, total),
        Err(error) => operation_error(error),
    }
}

#[post("/residences")]
async fn create_residence(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    context: web::ReqData<RequestContext>,
    body: web::Json<CreateResidenceRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    if !is_campus_scope(&authority, actor.clone().into_inner(), "hostel.occupancy") {
        return forbidden();
    }
    created_or_error(
        HostelOps::create_residence(
            pool.get_ref(),
            tenant_id(tenant),
            actor.into_inner(),
            context.into_inner(),
            &body,
        )
        .await,
    )
}

#[get("/residences/{id}")]
async fn read_residence(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    path: web::Path<Uuid>,
) -> HttpResponse {
    if !is_campus_scope(&authority, actor.into_inner(), "hostel.occupancy") {
        return forbidden();
    }
    found(
        HostelOps::get_residence(pool.get_ref(), tenant_id(tenant), path.into_inner()).await,
        "Residence",
    )
}

#[put("/residences/{id}")]
async fn update_residence(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<UpdateResidenceRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    if !is_campus_scope(&authority, actor.clone().into_inner(), "hostel.occupancy") {
        return forbidden();
    }
    updated_or_error(
        HostelOps::update_residence(
            pool.get_ref(),
            tenant_id(tenant),
            path.into_inner(),
            actor.into_inner(),
            context.into_inner(),
            &body,
        )
        .await,
        "Residence",
    )
}

#[get("/rooms")]
async fn list_rooms(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    query: web::Query<HostelListQuery>,
) -> HttpResponse {
    if !is_campus_scope(&authority, actor.into_inner(), "hostel.occupancy") {
        return forbidden();
    }
    let (page, per_page) = bounded_page(&query);
    match HostelOps::list_rooms(pool.get_ref(), tenant_id(tenant), &query).await {
        Ok((rooms, total)) => paginated(RoomsPage { rooms }, page, per_page, total),
        Err(error) => operation_error(error),
    }
}

#[post("/rooms")]
async fn create_room(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    context: web::ReqData<RequestContext>,
    body: web::Json<CreateRoomRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    if !is_campus_scope(&authority, actor.clone().into_inner(), "hostel.occupancy") {
        return forbidden();
    }
    created_or_error(
        HostelOps::create_room(
            pool.get_ref(),
            tenant_id(tenant),
            actor.into_inner(),
            context.into_inner(),
            &body,
        )
        .await,
    )
}

#[get("/rooms/{id}")]
async fn read_room(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    path: web::Path<Uuid>,
) -> HttpResponse {
    if !is_campus_scope(&authority, actor.into_inner(), "hostel.occupancy") {
        return forbidden();
    }
    found(
        HostelOps::get_room(pool.get_ref(), tenant_id(tenant), path.into_inner()).await,
        "Room",
    )
}

#[put("/rooms/{id}")]
async fn update_room(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<UpdateRoomRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    if !is_campus_scope(&authority, actor.clone().into_inner(), "hostel.occupancy") {
        return forbidden();
    }
    updated_or_error(
        HostelOps::update_room(
            pool.get_ref(),
            tenant_id(tenant),
            path.into_inner(),
            actor.into_inner(),
            context.into_inner(),
            &body,
        )
        .await,
        "Room",
    )
}

#[post("/allocations/preview")]
async fn allocation_preview(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    body: web::Json<AllocationPreviewRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    if !is_campus_scope(&authority, actor.into_inner(), "hostel.occupancy") {
        return forbidden();
    }
    value_or_error(HostelOps::allocation_preview(pool.get_ref(), tenant_id(tenant), &body).await)
}

#[get("/allocations")]
async fn list_allocations(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    query: web::Query<HostelListQuery>,
) -> HttpResponse {
    let Ok(scope) = hostel_scope(&authority, actor.into_inner(), "hostel.occupancy") else {
        return forbidden();
    };
    let (page, per_page) = bounded_page(&query);
    match HostelOps::list_allocations(pool.get_ref(), tenant_id(tenant), scope, &query).await {
        Ok((allocations, total)) => {
            paginated(AllocationsPage { allocations }, page, per_page, total)
        }
        Err(error) => operation_error(error),
    }
}

#[post("/allocations")]
async fn create_allocation(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    context: web::ReqData<RequestContext>,
    body: web::Json<CreateAllocationRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    if !is_campus_scope(&authority, actor.clone().into_inner(), "hostel.occupancy") {
        return forbidden();
    }
    created_or_error(
        HostelOps::create_allocation(
            pool.get_ref(),
            tenant_id(tenant),
            actor.into_inner(),
            context.into_inner(),
            &body,
        )
        .await,
    )
}

#[get("/allocations/{id}")]
async fn read_allocation(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    path: web::Path<Uuid>,
) -> HttpResponse {
    let Ok(scope) = hostel_scope(&authority, actor.into_inner(), "hostel.occupancy") else {
        return forbidden();
    };
    found(
        HostelOps::get_allocation(pool.get_ref(), tenant_id(tenant), path.into_inner(), scope)
            .await,
        "Allocation",
    )
}

#[post("/allocations/{id}/activate")]
async fn activate_allocation(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<ActivateAllocationRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    if !is_campus_scope(&authority, actor.clone().into_inner(), "hostel.occupancy") {
        return forbidden();
    }
    updated_or_error(
        HostelOps::activate_allocation(
            pool.get_ref(),
            tenant_id(tenant),
            path.into_inner(),
            actor.into_inner(),
            context.into_inner(),
            &body,
        )
        .await,
        "Allocation",
    )
}

#[post("/allocations/{id}/end")]
async fn end_allocation(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<EndAllocationRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    if !is_campus_scope(&authority, actor.clone().into_inner(), "hostel.occupancy") {
        return forbidden();
    }
    updated_or_error(
        HostelOps::end_allocation(
            pool.get_ref(),
            tenant_id(tenant),
            path.into_inner(),
            actor.into_inner(),
            context.into_inner(),
            &body,
        )
        .await,
        "Allocation",
    )
}

#[post("/allocations/{id}/cancel")]
async fn cancel_allocation(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<CancelAllocationRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    if !is_campus_scope(&authority, actor.clone().into_inner(), "hostel.occupancy") {
        return forbidden();
    }
    updated_or_error(
        HostelOps::cancel_allocation(
            pool.get_ref(),
            tenant_id(tenant),
            path.into_inner(),
            actor.into_inner(),
            context.into_inner(),
            &body,
        )
        .await,
        "Allocation",
    )
}

#[post("/allocations/{id}/transfer-preview")]
async fn transfer_preview(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    path: web::Path<Uuid>,
    body: web::Json<TransferAllocationPreviewRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    if !is_campus_scope(&authority, actor.into_inner(), "hostel.occupancy") {
        return forbidden();
    }
    value_or_error(
        HostelOps::transfer_preview(pool.get_ref(), tenant_id(tenant), path.into_inner(), &body)
            .await,
    )
}

#[post("/allocations/{id}/transfer")]
async fn transfer_allocation(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<TransferAllocationRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    if !is_campus_scope(&authority, actor.clone().into_inner(), "hostel.occupancy") {
        return forbidden();
    }
    updated_or_error(
        HostelOps::transfer_allocation(
            pool.get_ref(),
            tenant_id(tenant),
            path.into_inner(),
            actor.into_inner(),
            context.into_inner(),
            &body,
        )
        .await,
        "Allocation",
    )
}

#[get("/pastoral-records")]
async fn list_pastoral_records(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    query: web::Query<HostelListQuery>,
) -> HttpResponse {
    if !is_campus_scope(&authority, actor.into_inner(), "hostel.pastoral") {
        return forbidden();
    }
    let (page, per_page) = bounded_page(&query);
    match HostelOps::list_pastoral_records(pool.get_ref(), tenant_id(tenant), &query).await {
        Ok((pastoral_records, total)) => paginated(
            PastoralRecordsPage { pastoral_records },
            page,
            per_page,
            total,
        ),
        Err(error) => operation_error(error),
    }
}

#[post("/pastoral-records")]
async fn create_pastoral_record(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    context: web::ReqData<RequestContext>,
    body: web::Json<CreatePastoralRecordRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    if !is_campus_scope(&authority, actor.clone().into_inner(), "hostel.pastoral") {
        return forbidden();
    }
    created_or_error(
        HostelOps::create_pastoral_record(
            pool.get_ref(),
            tenant_id(tenant),
            actor.into_inner(),
            context.into_inner(),
            &body,
        )
        .await,
    )
}

#[get("/pastoral-records/{id}")]
async fn read_pastoral_record(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    path: web::Path<Uuid>,
) -> HttpResponse {
    if !is_campus_scope(&authority, actor.into_inner(), "hostel.pastoral") {
        return forbidden();
    }
    found(
        HostelOps::get_pastoral_record(pool.get_ref(), tenant_id(tenant), path.into_inner()).await,
        "Pastoral record",
    )
}

#[put("/pastoral-records/{id}")]
async fn update_pastoral_record(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<UpdatePastoralRecordRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    if !is_campus_scope(&authority, actor.clone().into_inner(), "hostel.pastoral") {
        return forbidden();
    }
    updated_or_error(
        HostelOps::update_pastoral_record(
            pool.get_ref(),
            tenant_id(tenant),
            path.into_inner(),
            actor.into_inner(),
            context.into_inner(),
            &body,
        )
        .await,
        "Pastoral record",
    )
}

#[post("/pastoral-records/{id}/resolve")]
async fn resolve_pastoral_record(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<ResolvePastoralRecordRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    if !is_campus_scope(&authority, actor.clone().into_inner(), "hostel.pastoral") {
        return forbidden();
    }
    updated_or_error(
        HostelOps::resolve_pastoral_record(
            pool.get_ref(),
            tenant_id(tenant),
            path.into_inner(),
            actor.into_inner(),
            context.into_inner(),
            &body,
        )
        .await,
        "Pastoral record",
    )
}

pub fn routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("")
            .wrap(RequirePermission::new("hostel"))
            .service(references)
            .service(list_residences)
            .service(create_residence)
            .service(read_residence)
            .service(update_residence)
            .service(list_rooms)
            .service(create_room)
            .service(read_room)
            .service(update_room)
            .service(allocation_preview)
            .service(list_allocations)
            .service(create_allocation)
            .service(read_allocation)
            .service(activate_allocation)
            .service(end_allocation)
            .service(cancel_allocation)
            .service(transfer_preview)
            .service(transfer_allocation)
            .service(list_pastoral_records)
            .service(create_pastoral_record)
            .service(read_pastoral_record)
            .service(update_pastoral_record)
            .service(resolve_pastoral_record),
    );
}

fn hostel_scope(
    authority: &Authority,
    actor: AuditActor,
    family: &str,
) -> Result<HostelAccessScope, ()> {
    if authority.0.has_permission("*") {
        return Ok(HostelAccessScope::Campus);
    }
    let family = RecordScopeFamilyKey::parse(family).map_err(|_| ())?;
    let account_id = actor.user_id().ok_or(())?;
    match authority.1.effective_scope(&family) {
        Some(EffectiveRecordScope::Campus) => Ok(HostelAccessScope::Campus),
        Some(
            EffectiveRecordScope::SelfRecord
            | EffectiveRecordScope::Assigned
            | EffectiveRecordScope::SelfAndAssigned,
        ) => Ok(HostelAccessScope::SelfFor(account_id)),
        None => Err(()),
    }
}
fn is_campus_scope(authority: &Authority, actor: AuditActor, family: &str) -> bool {
    matches!(
        hostel_scope(authority, actor, family),
        Ok(HostelAccessScope::Campus)
    )
}
fn tenant_id(value: web::ReqData<TenantId>) -> Uuid {
    value.into_inner().into_inner()
}
fn bounded_page(query: &HostelListQuery) -> (i64, i64) {
    (
        query.page.unwrap_or(1).max(1),
        query.per_page.unwrap_or(25).clamp(1, 100),
    )
}
fn trimmed(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
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
fn value_or_error<T: Serialize>(value: anyhow::Result<T>) -> HttpResponse {
    match value {
        Ok(value) => ok(value),
        Err(error) => operation_error(error),
    }
}
fn created_or_error<T: Serialize>(value: anyhow::Result<T>) -> HttpResponse {
    match value {
        Ok(value) => HttpResponse::Created().json(ApiResponse::from_status(
            StatusCode::CREATED,
            Some(value),
            None,
        )),
        Err(error) => operation_error(error),
    }
}
fn updated_or_error<T: Serialize>(value: anyhow::Result<Option<T>>, label: &str) -> HttpResponse {
    match value {
        Ok(Some(value)) => ok(value),
        Ok(None) => not_found(label),
        Err(error) => operation_error(error),
    }
}
fn found<T: Serialize>(value: anyhow::Result<Option<T>>, label: &str) -> HttpResponse {
    updated_or_error(value, label)
}
fn ok<T: Serialize>(value: T) -> HttpResponse {
    HttpResponse::Ok().json(ApiResponse::from_status(StatusCode::OK, Some(value), None))
}
fn not_found(label: &str) -> HttpResponse {
    HttpResponse::NotFound().json(ApiResponse::from_status(
        StatusCode::NOT_FOUND,
        None::<()>,
        Some(vec![format!("{label} not found")]),
    ))
}
fn forbidden() -> HttpResponse {
    HttpResponse::Forbidden().json(ApiResponse::from_status(
        StatusCode::FORBIDDEN,
        None::<()>,
        Some(vec![
            "This Hostel record scope is not available for this account".to_string(),
        ]),
    ))
}
fn operation_error(error: anyhow::Error) -> HttpResponse {
    let message = error.to_string();
    if message.contains("changed") || message.contains("already") || message.contains("capacity") {
        return HttpResponse::Conflict().json(ApiResponse::from_status(
            StatusCode::CONFLICT,
            None::<()>,
            Some(vec![message]),
        ));
    }
    if message.starts_with("The ")
        || message.starts_with("This ")
        || message.starts_with("A ")
        || message.starts_with("An ")
        || message.starts_with("Only ")
        || message.starts_with("Choose ")
        || message.starts_with("Room ")
        || message.starts_with("Residence ")
    {
        return HttpResponse::BadRequest().json(ApiResponse::from_status(
            StatusCode::BAD_REQUEST,
            None::<()>,
            Some(vec![message]),
        ));
    }
    HttpResponse::InternalServerError().json(ApiResponse::from_status(
        StatusCode::INTERNAL_SERVER_ERROR,
        None::<()>,
        Some(vec!["Hostel could not complete the request.".to_string()]),
    ))
}

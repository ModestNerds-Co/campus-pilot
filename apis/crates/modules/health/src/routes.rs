//! Authenticated, licensed, and record-scoped Health HTTP routes.

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
    CloseVisitRequest, CreateCareItemRequest, CreateFollowUpRequest, CreateMedicationPlanRequest,
    CreatePatientRequest, CreateVisitRequest, FollowUpsPage, HealthAccessScope, HealthListQuery,
    HealthOps, MedicationAdministrationsPage, MedicationPlansPage, PatientsPage,
    RecordMedicationAdministrationRequest, UpdateCareItemRequest, UpdateFollowUpRequest,
    UpdateMedicationPlanRequest, UpdatePatientRequest, VisitsPage,
};

type Authority = (web::ReqData<AccessContext>, web::ReqData<RecordScopeGrants>);

#[get("/references")]
async fn references(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    query: web::Query<HealthListQuery>,
) -> HttpResponse {
    if !is_campus_scope(&authority, actor.into_inner(), "health.patients") {
        return forbidden();
    }
    value_or_error(
        HealthOps::reference_data(
            pool.get_ref(),
            tenant_id(tenant),
            trimmed(query.search.as_deref()),
        )
        .await,
    )
}

#[get("/patients")]
async fn list_patients(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    query: web::Query<HealthListQuery>,
) -> HttpResponse {
    let Ok(scope) = health_scope(&authority, actor.into_inner(), "health.patients") else {
        return forbidden();
    };
    let (page, per_page) = bounded_page(&query);
    match HealthOps::list_patients(pool.get_ref(), tenant_id(tenant), scope, &query).await {
        Ok((patients, total)) => paginated(PatientsPage { patients }, page, per_page, total),
        Err(error) => operation_error(error),
    }
}

#[post("/patients")]
async fn create_patient(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    context: web::ReqData<RequestContext>,
    body: web::Json<CreatePatientRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    if !is_campus_scope(&authority, actor.clone().into_inner(), "health.patients") {
        return forbidden();
    }
    created_or_error(
        HealthOps::create_patient(
            pool.get_ref(),
            tenant_id(tenant),
            actor.into_inner(),
            context.into_inner(),
            &body,
        )
        .await,
    )
}

#[get("/patients/{id}")]
async fn read_patient(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    path: web::Path<Uuid>,
) -> HttpResponse {
    let Ok(scope) = health_scope(&authority, actor.into_inner(), "health.patients") else {
        return forbidden();
    };
    found(
        HealthOps::get_patient(pool.get_ref(), tenant_id(tenant), path.into_inner(), scope).await,
        "Health patient",
    )
}

#[put("/patients/{id}")]
async fn update_patient(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<UpdatePatientRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    if !is_campus_scope(&authority, actor.clone().into_inner(), "health.patients") {
        return forbidden();
    }
    updated_or_error(
        HealthOps::update_patient(
            pool.get_ref(),
            tenant_id(tenant),
            path.into_inner(),
            actor.into_inner(),
            context.into_inner(),
            &body,
        )
        .await,
        "Health patient",
    )
}

#[post("/patients/{id}/care-items")]
async fn create_care_item(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<CreateCareItemRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    if !is_campus_scope(&authority, actor.clone().into_inner(), "health.care") {
        return forbidden();
    }
    created_or_error(
        HealthOps::create_care_item(
            pool.get_ref(),
            tenant_id(tenant),
            path.into_inner(),
            actor.into_inner(),
            context.into_inner(),
            &body,
        )
        .await,
    )
}

#[put("/care-items/{id}")]
async fn update_care_item(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<UpdateCareItemRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    if !is_campus_scope(&authority, actor.clone().into_inner(), "health.care") {
        return forbidden();
    }
    updated_or_error(
        HealthOps::update_care_item(
            pool.get_ref(),
            tenant_id(tenant),
            path.into_inner(),
            actor.into_inner(),
            context.into_inner(),
            &body,
        )
        .await,
        "Care item",
    )
}

#[get("/visits")]
async fn list_visits(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    query: web::Query<HealthListQuery>,
) -> HttpResponse {
    let Ok(scope) = health_scope(&authority, actor.into_inner(), "health.care") else {
        return forbidden();
    };
    let (page, per_page) = bounded_page(&query);
    match HealthOps::list_visits(pool.get_ref(), tenant_id(tenant), scope, &query).await {
        Ok((visits, total)) => paginated(VisitsPage { visits }, page, per_page, total),
        Err(error) => operation_error(error),
    }
}

#[post("/visits")]
async fn create_visit(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    context: web::ReqData<RequestContext>,
    body: web::Json<CreateVisitRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    if !is_campus_scope(&authority, actor.clone().into_inner(), "health.care") {
        return forbidden();
    }
    created_or_error(
        HealthOps::create_visit(
            pool.get_ref(),
            tenant_id(tenant),
            actor.into_inner(),
            context.into_inner(),
            &body,
        )
        .await,
    )
}

#[get("/visits/{id}")]
async fn read_visit(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    path: web::Path<Uuid>,
) -> HttpResponse {
    let Ok(scope) = health_scope(&authority, actor.into_inner(), "health.care") else {
        return forbidden();
    };
    found(
        HealthOps::get_visit(pool.get_ref(), tenant_id(tenant), path.into_inner(), scope).await,
        "Clinic visit",
    )
}

#[post("/visits/{id}/close")]
async fn close_visit(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<CloseVisitRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    if !is_campus_scope(&authority, actor.clone().into_inner(), "health.care") {
        return forbidden();
    }
    updated_or_error(
        HealthOps::close_visit(
            pool.get_ref(),
            tenant_id(tenant),
            path.into_inner(),
            actor.into_inner(),
            context.into_inner(),
            &body,
        )
        .await,
        "Clinic visit",
    )
}

#[get("/medication-plans")]
async fn list_medication_plans(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    query: web::Query<HealthListQuery>,
) -> HttpResponse {
    let Ok(scope) = health_scope(&authority, actor.into_inner(), "health.care") else {
        return forbidden();
    };
    let (page, per_page) = bounded_page(&query);
    match HealthOps::list_medication_plans(pool.get_ref(), tenant_id(tenant), scope, &query).await {
        Ok((medication_plans, total)) => paginated(
            MedicationPlansPage { medication_plans },
            page,
            per_page,
            total,
        ),
        Err(error) => operation_error(error),
    }
}

#[post("/medication-plans")]
async fn create_medication_plan(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    context: web::ReqData<RequestContext>,
    body: web::Json<CreateMedicationPlanRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    if !is_campus_scope(&authority, actor.clone().into_inner(), "health.care") {
        return forbidden();
    }
    created_or_error(
        HealthOps::create_medication_plan(
            pool.get_ref(),
            tenant_id(tenant),
            actor.into_inner(),
            context.into_inner(),
            &body,
        )
        .await,
    )
}

#[put("/medication-plans/{id}")]
async fn update_medication_plan(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<UpdateMedicationPlanRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    if !is_campus_scope(&authority, actor.clone().into_inner(), "health.care") {
        return forbidden();
    }
    updated_or_error(
        HealthOps::update_medication_plan(
            pool.get_ref(),
            tenant_id(tenant),
            path.into_inner(),
            actor.into_inner(),
            context.into_inner(),
            &body,
        )
        .await,
        "Medication plan",
    )
}

#[get("/medication-administrations")]
async fn list_medication_administrations(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    query: web::Query<HealthListQuery>,
) -> HttpResponse {
    let Ok(scope) = health_scope(&authority, actor.into_inner(), "health.care") else {
        return forbidden();
    };
    let (page, per_page) = bounded_page(&query);
    match HealthOps::list_medication_administrations(
        pool.get_ref(),
        tenant_id(tenant),
        scope,
        &query,
    )
    .await
    {
        Ok((administrations, total)) => paginated(
            MedicationAdministrationsPage { administrations },
            page,
            per_page,
            total,
        ),
        Err(error) => operation_error(error),
    }
}

#[post("/medication-plans/{id}/administrations")]
async fn record_medication_administration(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<RecordMedicationAdministrationRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    if !is_campus_scope(&authority, actor.clone().into_inner(), "health.care") {
        return forbidden();
    }
    created_or_error(
        HealthOps::record_medication_administration(
            pool.get_ref(),
            tenant_id(tenant),
            path.into_inner(),
            actor.into_inner(),
            context.into_inner(),
            &body,
        )
        .await,
    )
}

#[get("/follow-ups")]
async fn list_follow_ups(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    query: web::Query<HealthListQuery>,
) -> HttpResponse {
    let Ok(scope) = health_scope(&authority, actor.into_inner(), "health.care") else {
        return forbidden();
    };
    let (page, per_page) = bounded_page(&query);
    match HealthOps::list_follow_ups(pool.get_ref(), tenant_id(tenant), scope, &query).await {
        Ok((follow_ups, total)) => paginated(FollowUpsPage { follow_ups }, page, per_page, total),
        Err(error) => operation_error(error),
    }
}

#[post("/follow-ups")]
async fn create_follow_up(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    context: web::ReqData<RequestContext>,
    body: web::Json<CreateFollowUpRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    if !is_campus_scope(&authority, actor.clone().into_inner(), "health.care") {
        return forbidden();
    }
    created_or_error(
        HealthOps::create_follow_up(
            pool.get_ref(),
            tenant_id(tenant),
            actor.into_inner(),
            context.into_inner(),
            &body,
        )
        .await,
    )
}

#[put("/follow-ups/{id}")]
async fn update_follow_up(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<UpdateFollowUpRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    if !is_campus_scope(&authority, actor.clone().into_inner(), "health.care") {
        return forbidden();
    }
    updated_or_error(
        HealthOps::update_follow_up(
            pool.get_ref(),
            tenant_id(tenant),
            path.into_inner(),
            actor.into_inner(),
            context.into_inner(),
            &body,
        )
        .await,
        "Health follow-up",
    )
}

pub fn routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("")
            .wrap(RequirePermission::new("health"))
            .service(references)
            .service(list_patients)
            .service(create_patient)
            .service(read_patient)
            .service(update_patient)
            .service(create_care_item)
            .service(update_care_item)
            .service(list_visits)
            .service(create_visit)
            .service(read_visit)
            .service(close_visit)
            .service(list_medication_plans)
            .service(create_medication_plan)
            .service(update_medication_plan)
            .service(list_medication_administrations)
            .service(record_medication_administration)
            .service(list_follow_ups)
            .service(create_follow_up)
            .service(update_follow_up),
    );
}

fn health_scope(
    authority: &Authority,
    actor: AuditActor,
    family: &str,
) -> Result<HealthAccessScope, ()> {
    if authority.0.has_permission("*") {
        return Ok(HealthAccessScope::Campus);
    }
    let family = RecordScopeFamilyKey::parse(family).map_err(|_| ())?;
    let account_id = actor.user_id().ok_or(())?;
    match authority.1.effective_scope(&family) {
        Some(EffectiveRecordScope::Campus) => Ok(HealthAccessScope::Campus),
        Some(
            EffectiveRecordScope::SelfRecord
            | EffectiveRecordScope::Assigned
            | EffectiveRecordScope::SelfAndAssigned,
        ) => Ok(HealthAccessScope::SelfFor(account_id)),
        None => Err(()),
    }
}
fn is_campus_scope(authority: &Authority, actor: AuditActor, family: &str) -> bool {
    matches!(
        health_scope(authority, actor, family),
        Ok(HealthAccessScope::Campus)
    )
}
fn tenant_id(value: web::ReqData<TenantId>) -> Uuid {
    value.into_inner().into_inner()
}
fn bounded_page(query: &HealthListQuery) -> (i64, i64) {
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
            "This Health record scope is not available for this account".to_string(),
        ]),
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
    if message.starts_with("The ")
        || message.starts_with("This ")
        || message.starts_with("A ")
        || message.starts_with("An ")
        || message.starts_with("Care ")
        || message.starts_with("Clinic ")
        || message.starts_with("Medication ")
        || message.starts_with("Follow-up ")
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
        Some(vec![
            "Health services could not complete the request.".to_string(),
        ]),
    ))
}

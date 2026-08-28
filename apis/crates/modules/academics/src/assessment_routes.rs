//! Authenticated HTTP routes for Academics assessment structures.

use actix_web::http::StatusCode;
use actix_web::{HttpResponse, delete, get, post, put, web};
use cp_common::{ApiResponse, PaginationMeta, TenantId, flatten_validation_errors};
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;
use validator::Validate;

use crate::{
    assessments::{
        AssessmentComponentListQuery, AssessmentComponentOps, AssessmentCycleListQuery,
        AssessmentCycleOps, CreateAssessmentComponentRequest, CreateAssessmentCycleRequest,
        PaginatedAssessmentComponentsResponse, PaginatedAssessmentCyclesResponse,
        UpdateAssessmentComponentRequest, UpdateAssessmentCycleRequest,
    },
    ops::DeleteOutcome,
};

#[get("/assessment-cycles")]
async fn list_assessment_cycles(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    query: web::Query<AssessmentCycleListQuery>,
) -> Result<HttpResponse, actix_web::Error> {
    let (page, per_page) = bounded_page(query.page, query.per_page);
    let (assessment_cycles, total) = AssessmentCycleOps::list(
        pool.get_ref(),
        tenant_id(tenant),
        page,
        per_page,
        trimmed(query.search.as_deref()),
        query.status.map(|value| value.as_str()),
        query.academic_term_id,
    )
    .await
    .map_err(actix_web::error::ErrorInternalServerError)?;
    Ok(paginated(
        PaginatedAssessmentCyclesResponse { assessment_cycles },
        page,
        per_page,
        total,
    ))
}

#[get("/assessment-cycles/{id}")]
async fn read_assessment_cycle(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, actix_web::Error> {
    let record =
        AssessmentCycleOps::get_by_id(pool.get_ref(), tenant_id(tenant), path.into_inner())
            .await
            .map_err(actix_web::error::ErrorInternalServerError)?;
    Ok(found(record, "Assessment cycle"))
}

#[post("/assessment-cycles")]
async fn create_assessment_cycle(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    body: web::Json<CreateAssessmentCycleRequest>,
) -> Result<HttpResponse, actix_web::Error> {
    if let Some(response) = validation_response(&*body) {
        return Ok(response);
    }
    Ok(created_or_error(
        AssessmentCycleOps::create(pool.get_ref(), tenant_id(tenant), &body).await,
    ))
}

#[put("/assessment-cycles/{id}")]
async fn update_assessment_cycle(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    path: web::Path<Uuid>,
    body: web::Json<UpdateAssessmentCycleRequest>,
) -> Result<HttpResponse, actix_web::Error> {
    if let Some(response) = validation_response(&*body) {
        return Ok(response);
    }
    Ok(updated_or_error(
        AssessmentCycleOps::update(pool.get_ref(), tenant_id(tenant), path.into_inner(), &body)
            .await,
        "Assessment cycle",
    ))
}

#[delete("/assessment-cycles/{id}")]
async fn delete_assessment_cycle(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, actix_web::Error> {
    Ok(
        match AssessmentCycleOps::delete(pool.get_ref(), tenant_id(tenant), path.into_inner()).await
        {
            Ok(outcome) => delete_response(
                outcome,
                "Assessment cycle",
                "Remove its assessment components before removing this cycle.",
            ),
            Err(error) => operation_error(error),
        },
    )
}

#[get("/assessment-cycles/{cycle_id}/components")]
async fn list_assessment_components(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    path: web::Path<Uuid>,
    query: web::Query<AssessmentComponentListQuery>,
) -> Result<HttpResponse, actix_web::Error> {
    let (page, per_page) = bounded_page(query.page, query.per_page);
    let (assessment_components, total) = AssessmentComponentOps::list(
        pool.get_ref(),
        tenant_id(tenant),
        path.into_inner(),
        page,
        per_page,
        query.status.map(|value| value.as_str()),
        query.teaching_assignment_id,
    )
    .await
    .map_err(actix_web::error::ErrorInternalServerError)?;
    Ok(paginated(
        PaginatedAssessmentComponentsResponse {
            assessment_components,
        },
        page,
        per_page,
        total,
    ))
}

#[get("/assessment-components/{id}")]
async fn read_assessment_component(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, actix_web::Error> {
    let record =
        AssessmentComponentOps::get_by_id(pool.get_ref(), tenant_id(tenant), path.into_inner())
            .await
            .map_err(actix_web::error::ErrorInternalServerError)?;
    Ok(found(record, "Assessment component"))
}

#[post("/assessment-cycles/{cycle_id}/components")]
async fn create_assessment_component(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    path: web::Path<Uuid>,
    body: web::Json<CreateAssessmentComponentRequest>,
) -> Result<HttpResponse, actix_web::Error> {
    if let Some(response) = validation_response(&*body) {
        return Ok(response);
    }
    Ok(created_or_error(
        AssessmentComponentOps::create(pool.get_ref(), tenant_id(tenant), path.into_inner(), &body)
            .await,
    ))
}

#[put("/assessment-components/{id}")]
async fn update_assessment_component(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    path: web::Path<Uuid>,
    body: web::Json<UpdateAssessmentComponentRequest>,
) -> Result<HttpResponse, actix_web::Error> {
    if let Some(response) = validation_response(&*body) {
        return Ok(response);
    }
    Ok(updated_or_error(
        AssessmentComponentOps::update(pool.get_ref(), tenant_id(tenant), path.into_inner(), &body)
            .await,
        "Assessment component",
    ))
}

#[delete("/assessment-components/{id}")]
async fn delete_assessment_component(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, actix_web::Error> {
    Ok(
        match AssessmentComponentOps::delete(pool.get_ref(), tenant_id(tenant), path.into_inner())
            .await
        {
            Ok(outcome) => delete_response(
                outcome,
                "Assessment component",
                "This assessment component is in use.",
            ),
            Err(error) => operation_error(error),
        },
    )
}

fn tenant_id(tenant: web::ReqData<TenantId>) -> Uuid {
    tenant.into_inner().into_inner()
}

fn bounded_page(page: Option<i64>, per_page: Option<i64>) -> (i64, i64) {
    (
        page.unwrap_or(1).max(1),
        per_page.unwrap_or(25).clamp(1, 100),
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

fn found<T: Serialize>(value: Option<T>, label: &str) -> HttpResponse {
    value.map_or_else(
        || not_found(label),
        |record| {
            HttpResponse::Ok().json(ApiResponse::from_status(StatusCode::OK, Some(record), None))
        },
    )
}

fn not_found(label: &str) -> HttpResponse {
    HttpResponse::NotFound().json(ApiResponse::from_status(
        StatusCode::NOT_FOUND,
        None::<()>,
        Some(vec![format!("{label} not found")]),
    ))
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
        Ok(Some(value)) => {
            HttpResponse::Ok().json(ApiResponse::from_status(StatusCode::OK, Some(value), None))
        }
        Ok(None) => not_found(label),
        Err(error) => operation_error(error),
    }
}

fn delete_response(outcome: DeleteOutcome, label: &str, in_use_message: &str) -> HttpResponse {
    match outcome {
        DeleteOutcome::Deleted => HttpResponse::Ok().json(ApiResponse::from_status(
            StatusCode::OK,
            Some(serde_json::json!({ "deleted": true })),
            None,
        )),
        DeleteOutcome::NotFound => not_found(label),
        DeleteOutcome::InUse => HttpResponse::Conflict().json(ApiResponse::from_status(
            StatusCode::CONFLICT,
            None::<()>,
            Some(vec![in_use_message.to_string()]),
        )),
    }
}

fn operation_error(error: anyhow::Error) -> HttpResponse {
    if let Some(database) = error.root_cause().downcast_ref::<sqlx::Error>()
        && let sqlx::Error::Database(database) = database
        && database.code().as_deref() == Some("23505")
    {
        return HttpResponse::Conflict().json(ApiResponse::from_status(
            StatusCode::CONFLICT,
            None::<()>,
            Some(vec!["That assessment record already exists.".to_string()]),
        ));
    }
    let safe = error.chain().map(ToString::to_string).find(|message| {
        message.starts_with("Academic term")
            || message.starts_with("A closed")
            || message.starts_with("Only a draft")
            || message.starts_with("Assessment cycle")
            || message.starts_with("Assessment components")
            || message.starts_with("Active assessment")
            || message.starts_with("Add at least")
            || message.starts_with("Remove assessment")
            || message.starts_with("Teaching assignment")
            || message.starts_with("An active assessment")
            || message.starts_with("Assessment date")
    });
    if let Some(message) = safe {
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
            "The assessment record could not be saved.".to_string(),
        ]),
    ))
}

pub fn routes(cfg: &mut web::ServiceConfig) {
    cfg.service(list_assessment_cycles)
        .service(read_assessment_cycle)
        .service(create_assessment_cycle)
        .service(update_assessment_cycle)
        .service(delete_assessment_cycle)
        .service(list_assessment_components)
        .service(read_assessment_component)
        .service(create_assessment_component)
        .service(update_assessment_component)
        .service(delete_assessment_component);
}

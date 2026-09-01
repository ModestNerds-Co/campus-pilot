//! Authenticated Attendance HTTP routes over typed module operations.
//!
//! Authentication is mounted by the application. This scope applies the exact
//! licensed Attendance operation from the shared product catalogue.

use actix_web::http::StatusCode;
use actix_web::{HttpResponse, delete, get, post, put, web};
use cp_audit::{AuditActor, RequestContext};
use cp_common::{
    ApiResponse, PaginationMeta, RequirePermission, TenantId, flatten_validation_errors,
};
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;
use validator::Validate;

use crate::dtos::{
    CreateAttendanceRegisterRequest, DeleteAttendanceRegisterQuery,
    PaginatedAttendanceRegistersResponse, ReopenAttendanceRegisterRequest,
    SubmitAttendanceRegisterRequest, UpdateAttendanceMarksRequest,
};
use crate::{AttendanceOps, AttendanceRegisterListQuery};

#[get("/references")]
async fn reference_data(pool: web::Data<PgPool>, tenant: web::ReqData<TenantId>) -> HttpResponse {
    match AttendanceOps::reference_data(pool.get_ref(), tenant_id(tenant)).await {
        Ok(data) => ok(data),
        Err(_) => internal_error(),
    }
}

#[get("/registers")]
async fn list_registers(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    query: web::Query<AttendanceRegisterListQuery>,
) -> HttpResponse {
    let (page, per_page) = bounded_page(query.page, query.per_page);
    match AttendanceOps::list(pool.get_ref(), tenant_id(tenant), &query.0).await {
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
    actor: web::ReqData<AuditActor>,
    request_context: web::ReqData<RequestContext>,
    body: web::Json<CreateAttendanceRegisterRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    match AttendanceOps::create(
        pool.get_ref(),
        tenant_id(tenant),
        actor.into_inner(),
        request_context.into_inner(),
        &body.0,
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
    path: web::Path<Uuid>,
) -> HttpResponse {
    match AttendanceOps::get(pool.get_ref(), tenant_id(tenant), path.into_inner()).await {
        Ok(Some(register)) => ok(register),
        Ok(None) => not_found(),
        Err(_) => internal_error(),
    }
}

#[put("/registers/{id}/marks")]
async fn update_marks(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    actor: web::ReqData<AuditActor>,
    request_context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<UpdateAttendanceMarksRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    match AttendanceOps::update_marks(
        pool.get_ref(),
        tenant_id(tenant),
        path.into_inner(),
        actor.into_inner(),
        request_context.into_inner(),
        &body.0,
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
    actor: web::ReqData<AuditActor>,
    request_context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<SubmitAttendanceRegisterRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    match AttendanceOps::submit(
        pool.get_ref(),
        tenant_id(tenant),
        path.into_inner(),
        actor.into_inner(),
        request_context.into_inner(),
        body.expected_version,
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
    actor: web::ReqData<AuditActor>,
    request_context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<ReopenAttendanceRegisterRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    match AttendanceOps::reopen(
        pool.get_ref(),
        tenant_id(tenant),
        path.into_inner(),
        actor.into_inner(),
        request_context.into_inner(),
        &body.0,
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
    actor: web::ReqData<AuditActor>,
    request_context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    query: web::Query<DeleteAttendanceRegisterQuery>,
) -> HttpResponse {
    if let Some(response) = validation_response(&query.0) {
        return response;
    }
    match AttendanceOps::delete(
        pool.get_ref(),
        tenant_id(tenant),
        path.into_inner(),
        actor.into_inner(),
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
            .wrap(RequirePermission::new("attendance"))
            .service(reference_data)
            .service(list_registers)
            .service(create_register)
            .service(read_register)
            .service(update_marks)
            .service(submit_register)
            .service(reopen_register)
            .service(delete_register),
    );
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

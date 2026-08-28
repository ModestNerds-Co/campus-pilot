//
//  cp-procurement
//  routes.rs
//
//  Copyright (c) 2025 Codecraft Solutions. All rights reserved.
//
//! Authenticated Procurement routes with exact operation authorization.

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

use crate::requisitions::{
    CreateRequisitionRequest, DecisionRequest, PaginatedRequisitionsResponse,
    ProcurementReferenceOps, RequesterCandidateQuery, RequisitionListQuery, RequisitionOps,
    RequisitionResponse, UpdateRequisitionRequest, VersionRequest,
};
use crate::suppliers::{
    CreateSupplierRequest, PaginatedSuppliersResponse, SupplierDeleteQuery, SupplierListQuery,
    SupplierOps, UpdateSupplierRequest,
};

#[get("/reference-data")]
async fn read_reference_data(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
) -> HttpResponse {
    match ProcurementReferenceOps::currencies(pool.get_ref(), tenant_id(tenant)).await {
        Ok(value) => ok(value),
        Err(_) => internal_error(),
    }
}

#[get("/requester-candidates")]
async fn list_requester_candidates(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    query: web::Query<RequesterCandidateQuery>,
) -> HttpResponse {
    match ProcurementReferenceOps::requester_candidates(
        pool.get_ref(),
        tenant_id(tenant),
        trimmed(query.search.as_deref()),
    )
    .await
    {
        Ok(value) => ok(value),
        Err(_) => internal_error(),
    }
}

#[get("/suppliers")]
async fn list_suppliers(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    query: web::Query<SupplierListQuery>,
) -> HttpResponse {
    let (page, per_page) = bounded_page(query.page, query.per_page);
    match SupplierOps::list(
        pool.get_ref(),
        tenant_id(tenant),
        page,
        per_page,
        trimmed(query.search.as_deref()),
        trimmed(query.status.as_deref()),
    )
    .await
    {
        Ok((suppliers, total)) => paginated(
            PaginatedSuppliersResponse { suppliers },
            page,
            per_page,
            total,
        ),
        Err(error) => operation_error(error),
    }
}

#[get("/suppliers/{id}")]
async fn read_supplier(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    path: web::Path<Uuid>,
) -> HttpResponse {
    match SupplierOps::get(pool.get_ref(), tenant_id(tenant), path.into_inner()).await {
        Ok(Some(value)) => ok(value),
        Ok(None) => not_found("Supplier"),
        Err(_) => internal_error(),
    }
}

#[post("/suppliers")]
async fn create_supplier(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    actor: web::ReqData<AuditActor>,
    request_context: web::ReqData<RequestContext>,
    body: web::Json<CreateSupplierRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    match SupplierOps::create(
        pool.get_ref(),
        tenant_id(tenant),
        actor.into_inner(),
        request_context.into_inner(),
        &body.0,
    )
    .await
    {
        Ok(value) => created(value),
        Err(error) => operation_error(error),
    }
}

#[put("/suppliers/{id}")]
async fn update_supplier(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    actor: web::ReqData<AuditActor>,
    request_context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<UpdateSupplierRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    match SupplierOps::update(
        pool.get_ref(),
        tenant_id(tenant),
        path.into_inner(),
        actor.into_inner(),
        request_context.into_inner(),
        &body.0,
    )
    .await
    {
        Ok(Some(value)) => ok(value),
        Ok(None) => not_found("Supplier"),
        Err(error) => operation_error(error),
    }
}

#[delete("/suppliers/{id}")]
async fn delete_supplier(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    actor: web::ReqData<AuditActor>,
    request_context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    query: web::Query<SupplierDeleteQuery>,
) -> HttpResponse {
    if let Some(response) = validation_response(&query.0) {
        return response;
    }
    match SupplierOps::delete(
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
        Ok(false) => not_found("Supplier"),
        Err(error) => operation_error(error),
    }
}

#[get("/requisitions")]
async fn list_requisitions(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    query: web::Query<RequisitionListQuery>,
) -> HttpResponse {
    let (page, per_page) = bounded_page(query.page, query.per_page);
    match RequisitionOps::list(
        pool.get_ref(),
        tenant_id(tenant),
        page,
        per_page,
        trimmed(query.search.as_deref()),
        trimmed(query.status.as_deref()),
        query.requester_employee_id,
    )
    .await
    {
        Ok((requisitions, total)) => paginated(
            PaginatedRequisitionsResponse { requisitions },
            page,
            per_page,
            total,
        ),
        Err(error) => operation_error(error),
    }
}

#[get("/requisitions/{id}")]
async fn read_requisition(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    path: web::Path<Uuid>,
) -> HttpResponse {
    match RequisitionOps::get(pool.get_ref(), tenant_id(tenant), path.into_inner()).await {
        Ok(Some(value)) => ok(value),
        Ok(None) => not_found("Requisition"),
        Err(_) => internal_error(),
    }
}

#[post("/requisitions")]
async fn create_requisition(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    actor: web::ReqData<AuditActor>,
    request_context: web::ReqData<RequestContext>,
    body: web::Json<CreateRequisitionRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    match RequisitionOps::create(
        pool.get_ref(),
        tenant_id(tenant),
        actor.into_inner(),
        request_context.into_inner(),
        &body.0,
    )
    .await
    {
        Ok(value) => created(value),
        Err(error) => operation_error(error),
    }
}

#[put("/requisitions/{id}")]
async fn update_requisition(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    actor: web::ReqData<AuditActor>,
    request_context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<UpdateRequisitionRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    updated_requisition(
        RequisitionOps::update(
            pool.get_ref(),
            tenant_id(tenant),
            path.into_inner(),
            actor.into_inner(),
            request_context.into_inner(),
            &body.0,
        )
        .await,
    )
}

#[delete("/requisitions/{id}")]
async fn delete_requisition(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    actor: web::ReqData<AuditActor>,
    request_context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    query: web::Query<VersionRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&query.0) {
        return response;
    }
    match RequisitionOps::delete(
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
        Ok(false) => not_found("Requisition"),
        Err(error) => operation_error(error),
    }
}

#[post("/requisitions/{id}/submit")]
async fn submit_requisition(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    actor: web::ReqData<AuditActor>,
    request_context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<VersionRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    updated_requisition(
        RequisitionOps::submit(
            pool.get_ref(),
            tenant_id(tenant),
            path.into_inner(),
            actor.into_inner(),
            request_context.into_inner(),
            body.expected_version,
        )
        .await,
    )
}

#[post("/requisitions/{id}/approve")]
async fn approve_requisition(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    actor: web::ReqData<AuditActor>,
    request_context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<DecisionRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    updated_requisition(
        RequisitionOps::approve(
            pool.get_ref(),
            tenant_id(tenant),
            path.into_inner(),
            actor.into_inner(),
            request_context.into_inner(),
            &body.0,
        )
        .await,
    )
}

#[post("/requisitions/{id}/reject")]
async fn reject_requisition(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    actor: web::ReqData<AuditActor>,
    request_context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<DecisionRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    updated_requisition(
        RequisitionOps::reject(
            pool.get_ref(),
            tenant_id(tenant),
            path.into_inner(),
            actor.into_inner(),
            request_context.into_inner(),
            &body.0,
        )
        .await,
    )
}

#[post("/requisitions/{id}/cancel")]
async fn cancel_requisition(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    actor: web::ReqData<AuditActor>,
    request_context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<DecisionRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    updated_requisition(
        RequisitionOps::cancel(
            pool.get_ref(),
            tenant_id(tenant),
            path.into_inner(),
            actor.into_inner(),
            request_context.into_inner(),
            &body.0,
        )
        .await,
    )
}

fn updated_requisition(result: anyhow::Result<Option<RequisitionResponse>>) -> HttpResponse {
    match result {
        Ok(Some(value)) => ok(value),
        Ok(None) => not_found("Requisition"),
        Err(error) => operation_error(error),
    }
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

fn not_found(label: &str) -> HttpResponse {
    HttpResponse::NotFound().json(ApiResponse::from_status(
        StatusCode::NOT_FOUND,
        None::<()>,
        Some(vec![format!("{label} was not found.")]),
    ))
}

fn internal_error() -> HttpResponse {
    HttpResponse::InternalServerError().json(ApiResponse::from_status(
        StatusCode::INTERNAL_SERVER_ERROR,
        None::<()>,
        Some(vec![
            "The Procurement record could not be loaded.".to_string(),
        ]),
    ))
}

fn operation_error(error: anyhow::Error) -> HttpResponse {
    let database = error
        .root_cause()
        .downcast_ref::<sqlx::Error>()
        .and_then(|error| {
            if let sqlx::Error::Database(database) = error {
                Some(database)
            } else {
                None
            }
        });
    if database.is_some_and(|database| database.code().as_deref() == Some("23505")) {
        return HttpResponse::Conflict().json(ApiResponse::from_status(
            StatusCode::CONFLICT,
            None::<()>,
            Some(vec![
                "That Procurement identity already exists.".to_string(),
            ]),
        ));
    }
    let message = database
        .map(|database| database.message().to_string())
        .unwrap_or_else(|| error.to_string());
    if message.contains("changed since it was loaded")
        || message.contains("already exists")
        || message.contains("already belongs")
    {
        return HttpResponse::Conflict().json(ApiResponse::from_status(
            StatusCode::CONFLICT,
            None::<()>,
            Some(vec![message]),
        ));
    }
    let operational = [
        "A ",
        "Draft ",
        "Every ",
        "Idempotency ",
        "Only ",
        "Preferred ",
        "Requisition ",
        "Requisitions ",
        "Submitted ",
        "Supplier ",
        "The ",
    ]
    .iter()
    .any(|prefix| message.starts_with(prefix));
    if operational {
        return HttpResponse::BadRequest().json(ApiResponse::from_status(
            StatusCode::BAD_REQUEST,
            None::<()>,
            Some(vec![message]),
        ));
    }
    internal_error()
}

pub fn routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("")
            .wrap(RequirePermission::new("procurement"))
            .service(read_reference_data)
            .service(list_requester_candidates)
            .service(list_suppliers)
            .service(read_supplier)
            .service(create_supplier)
            .service(update_supplier)
            .service(delete_supplier)
            .service(list_requisitions)
            .service(read_requisition)
            .service(create_requisition)
            .service(update_requisition)
            .service(delete_requisition)
            .service(submit_requisition)
            .service(approve_requisition)
            .service(reject_requisition)
            .service(cancel_requisition),
    );
}

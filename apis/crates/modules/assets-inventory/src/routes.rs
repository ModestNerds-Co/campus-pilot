//! Actix routes for the Assets and inventory item and store catalogues.
//!
//! The application owns authentication and module mounting; this scope applies
//! only the independently licensed `assets_inventory` operation permissions.

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
    CreateItemRequest, CreateStoreRequest, DeleteAssetQuery, ItemListQuery, PaginatedItemsResponse,
    PaginatedStoresResponse, StoreListQuery, UpdateItemRequest, UpdateStoreRequest,
};
use crate::ops::{ItemOps, StoreOps};

#[get("/items")]
async fn list_items(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    query: web::Query<ItemListQuery>,
) -> HttpResponse {
    let (page, per_page) = bounded_page(query.page, query.per_page);
    match ItemOps::list(
        pool.get_ref(),
        tenant_id(tenant),
        page,
        per_page,
        query.search.as_deref(),
        query.status.as_deref(),
    )
    .await
    {
        Ok((items, total)) => paginated(PaginatedItemsResponse { items }, page, per_page, total),
        Err(error) => operation_error(error),
    }
}

#[get("/items/{id}")]
async fn read_item(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    path: web::Path<Uuid>,
) -> HttpResponse {
    match ItemOps::get(pool.get_ref(), tenant_id(tenant), path.into_inner()).await {
        Ok(Some(item)) => ok(item),
        Ok(None) => not_found("Item"),
        Err(_) => internal_error(),
    }
}

#[post("/items")]
async fn create_item(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    actor: web::ReqData<AuditActor>,
    request_context: web::ReqData<RequestContext>,
    body: web::Json<CreateItemRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    match ItemOps::create(
        pool.get_ref(),
        tenant_id(tenant),
        actor.into_inner(),
        request_context.into_inner(),
        &body.0,
    )
    .await
    {
        Ok(item) => created(item),
        Err(error) => operation_error(error),
    }
}

#[put("/items/{id}")]
async fn update_item(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    actor: web::ReqData<AuditActor>,
    request_context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<UpdateItemRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    match ItemOps::update(
        pool.get_ref(),
        tenant_id(tenant),
        path.into_inner(),
        actor.into_inner(),
        request_context.into_inner(),
        &body.0,
    )
    .await
    {
        Ok(Some(item)) => ok(item),
        Ok(None) => not_found("Item"),
        Err(error) => operation_error(error),
    }
}

#[delete("/items/{id}")]
async fn delete_item(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    actor: web::ReqData<AuditActor>,
    request_context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    query: web::Query<DeleteAssetQuery>,
) -> HttpResponse {
    if let Some(response) = validation_response(&query.0) {
        return response;
    }
    match ItemOps::delete(
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
        Ok(false) => not_found("Item"),
        Err(error) => operation_error(error),
    }
}

#[get("/stores")]
async fn list_stores(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    query: web::Query<StoreListQuery>,
) -> HttpResponse {
    let (page, per_page) = bounded_page(query.page, query.per_page);
    match StoreOps::list(
        pool.get_ref(),
        tenant_id(tenant),
        page,
        per_page,
        query.search.as_deref(),
        query.status.as_deref(),
    )
    .await
    {
        Ok((stores, total)) => paginated(PaginatedStoresResponse { stores }, page, per_page, total),
        Err(error) => operation_error(error),
    }
}

#[get("/stores/{id}")]
async fn read_store(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    path: web::Path<Uuid>,
) -> HttpResponse {
    match StoreOps::get(pool.get_ref(), tenant_id(tenant), path.into_inner()).await {
        Ok(Some(store)) => ok(store),
        Ok(None) => not_found("Store"),
        Err(_) => internal_error(),
    }
}

#[post("/stores")]
async fn create_store(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    actor: web::ReqData<AuditActor>,
    request_context: web::ReqData<RequestContext>,
    body: web::Json<CreateStoreRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    match StoreOps::create(
        pool.get_ref(),
        tenant_id(tenant),
        actor.into_inner(),
        request_context.into_inner(),
        &body.0,
    )
    .await
    {
        Ok(store) => created(store),
        Err(error) => operation_error(error),
    }
}

#[put("/stores/{id}")]
async fn update_store(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    actor: web::ReqData<AuditActor>,
    request_context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<UpdateStoreRequest>,
) -> HttpResponse {
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    match StoreOps::update(
        pool.get_ref(),
        tenant_id(tenant),
        path.into_inner(),
        actor.into_inner(),
        request_context.into_inner(),
        &body.0,
    )
    .await
    {
        Ok(Some(store)) => ok(store),
        Ok(None) => not_found("Store"),
        Err(error) => operation_error(error),
    }
}

#[delete("/stores/{id}")]
async fn delete_store(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    actor: web::ReqData<AuditActor>,
    request_context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    query: web::Query<DeleteAssetQuery>,
) -> HttpResponse {
    if let Some(response) = validation_response(&query.0) {
        return response;
    }
    match StoreOps::delete(
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
        Ok(false) => not_found("Store"),
        Err(error) => operation_error(error),
    }
}

fn tenant_id(tenant: web::ReqData<TenantId>) -> Uuid {
    tenant.into_inner().into_inner()
}

fn bounded_page(page: Option<i64>, per_page: Option<i64>) -> (i64, i64) {
    (
        page.unwrap_or(1).clamp(1, 1_000_000),
        per_page.unwrap_or(25).clamp(1, 100),
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
            "The Assets and inventory record could not be loaded.".to_string(),
        ]),
    ))
}

fn operation_error(error: anyhow::Error) -> HttpResponse {
    let database = error
        .root_cause()
        .downcast_ref::<sqlx::Error>()
        .and_then(|error| match error {
            sqlx::Error::Database(database) => Some(database),
            _ => None,
        });
    let message = database
        .map(|database| database.message().to_string())
        .unwrap_or_else(|| error.to_string());
    if database.is_some_and(|database| database.code().as_deref() == Some("23505"))
        || message.contains("changed since it was loaded")
        || message.contains("already exists")
        || message.contains("already belongs")
    {
        return HttpResponse::Conflict().json(ApiResponse::from_status(
            StatusCode::CONFLICT,
            None::<()>,
            Some(vec![message]),
        ));
    }
    if message.starts_with("Asset ")
        || message.starts_with("Authenticated ")
        || message.starts_with("Catalogue ")
        || message.starts_with("Idempotency ")
        || message.starts_with("Item ")
        || message.starts_with("Only ")
        || message.starts_with("Store ")
    {
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
            .wrap(RequirePermission::new("assets_inventory"))
            .service(list_items)
            .service(read_item)
            .service(create_item)
            .service(update_item)
            .service(delete_item)
            .service(list_stores)
            .service(read_store)
            .service(create_store)
            .service(update_store)
            .service(delete_store),
    );
}

#[cfg(test)]
mod tests {
    use super::bounded_page;

    #[test]
    fn route_pagination_is_bounded() {
        assert_eq!(bounded_page(None, None), (1, 25));
        assert_eq!(bounded_page(Some(0), Some(101)), (1, 100));
        assert_eq!(bounded_page(Some(i64::MAX), Some(25)), (1_000_000, 25));
    }
}

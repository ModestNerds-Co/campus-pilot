//! Authenticated, licensed, record-scoped Document Registry HTTP routes.

use actix_multipart::Multipart;
use actix_web::http::StatusCode;
use actix_web::{HttpResponse, get, post, put, web};
use chrono::NaiveDate;
use cp_audit::{AuditActor, RequestContext};
use cp_common::{
    AccessContext, ApiResponse, EffectiveRecordScope, PaginationMeta, RecordScopeFamilyKey,
    RecordScopeGrants, RequirePermission, TenantId, flatten_validation_errors,
};
use futures_util::StreamExt;
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;
use validator::Validate;

use crate::{
    ActivityPage, CloseFileRequest, CreateReviewRequest, CreateSeriesRequest, DocumentRegistryOps,
    DocumentStorage, DownloadResponse, ExecuteDestructionRequest, FilesPage, NewRegistryFile,
    ReclassifyFileRequest, RegistryListQuery, ReviewDecisionRequest, ReviewsPage, SeriesPage,
    UpdateFileRequest, UpdateNumberingPolicyRequest, UpdateSeriesRequest,
    storage::MAX_DOCUMENT_BYTES,
};

type Authority = (web::ReqData<AccessContext>, web::ReqData<RecordScopeGrants>);

#[get("/numbering-policy")]
async fn numbering_policy(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
) -> HttpResponse {
    if !allowed(&authority, "document_registry:view") {
        return forbidden();
    }
    value_or_error(DocumentRegistryOps::numbering_policy(&pool, tenant_id(tenant)).await)
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
    if !allowed(&authority, "document_registry:manage") {
        return forbidden();
    }
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    updated_or_error(
        DocumentRegistryOps::update_numbering_policy(
            &pool,
            tenant_id(tenant),
            actor.into_inner(),
            context.into_inner(),
            &body,
        )
        .await,
        "Numbering policy",
    )
}

#[get("/series")]
async fn list_series(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    query: web::Query<RegistryListQuery>,
) -> HttpResponse {
    if !allowed(&authority, "document_registry:view") {
        return forbidden();
    }
    let (page, per_page) = bounded_page(&query);
    match DocumentRegistryOps::list_series(&pool, tenant_id(tenant), &query).await {
        Ok((series, total)) => paginated(SeriesPage { series }, page, per_page, total),
        Err(error) => operation_error(error),
    }
}
#[post("/series")]
async fn create_series(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    context: web::ReqData<RequestContext>,
    body: web::Json<CreateSeriesRequest>,
) -> HttpResponse {
    if !allowed(&authority, "document_registry:manage") {
        return forbidden();
    }
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    created_or_error(
        DocumentRegistryOps::create_series(
            &pool,
            tenant_id(tenant),
            actor.into_inner(),
            context.into_inner(),
            &body,
        )
        .await,
    )
}
#[get("/series/{id}")]
async fn read_series(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    path: web::Path<Uuid>,
) -> HttpResponse {
    if !allowed(&authority, "document_registry:view") {
        return forbidden();
    }
    found(
        DocumentRegistryOps::get_series(&pool, tenant_id(tenant), path.into_inner()).await,
        "Classification",
    )
}
#[put("/series/{id}")]
async fn update_series(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<UpdateSeriesRequest>,
) -> HttpResponse {
    if !allowed(&authority, "document_registry:manage") {
        return forbidden();
    }
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    updated_or_error(
        DocumentRegistryOps::update_series(
            &pool,
            tenant_id(tenant),
            path.into_inner(),
            actor.into_inner(),
            context.into_inner(),
            &body,
        )
        .await,
        "Classification",
    )
}

#[get("/files")]
async fn list_files(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    query: web::Query<RegistryListQuery>,
) -> HttpResponse {
    if !allowed(&authority, "document_registry:view") {
        return forbidden();
    }
    let (page, per_page) = bounded_page(&query);
    let restricted = allowed(&authority, "document_registry:restricted");
    match DocumentRegistryOps::list_files(&pool, tenant_id(tenant), &query, restricted).await {
        Ok((files, total)) => paginated(FilesPage { files }, page, per_page, total),
        Err(error) => operation_error(error),
    }
}

#[post("/files")]
async fn create_file(
    pool: web::Data<PgPool>,
    storage: web::Data<DocumentStorage>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    context: web::ReqData<RequestContext>,
    mut payload: Multipart,
) -> HttpResponse {
    if !allowed(&authority, "document_registry:create") {
        return forbidden();
    }
    let mut series_id = None;
    let mut title = None;
    let mut description = None;
    let mut document_date = None;
    let mut sensitivity = None;
    let mut original_file_name = None;
    let mut media_type = None;
    let mut bytes = None;
    while let Some(item) = payload.next().await {
        let mut field = match item {
            Ok(field) => field,
            Err(_) => return bad_request("The document upload is malformed"),
        };
        let disposition = field.content_disposition();
        let name = disposition
            .and_then(|v| v.get_name())
            .unwrap_or_default()
            .to_string();
        if name == "file" {
            if bytes.is_some() {
                return bad_request("Upload one document at a time");
            }
            original_file_name = disposition
                .and_then(|v| v.get_filename())
                .map(ToOwned::to_owned);
            media_type = field.content_type().map(ToString::to_string);
            bytes = match read_field(&mut field, MAX_DOCUMENT_BYTES).await {
                Ok(value) => Some(value),
                Err(message) => return bad_request(message),
            };
            continue;
        }
        let raw = match read_field(&mut field, 4096).await {
            Ok(value) => value,
            Err(message) => return bad_request(message),
        };
        let value = match String::from_utf8(raw) {
            Ok(value) => value.trim().to_string(),
            Err(_) => return bad_request("The document metadata is invalid"),
        };
        match name.as_str() {
            "series_id" => series_id = Uuid::parse_str(&value).ok(),
            "title" => title = Some(value),
            "description" => description = non_empty(value),
            "document_date" => {
                document_date = if value.is_empty() {
                    None
                } else {
                    match NaiveDate::parse_from_str(&value, "%Y-%m-%d") {
                        Ok(date) => Some(date),
                        Err(_) => return bad_request("Document date is invalid"),
                    }
                }
            }
            "sensitivity" => sensitivity = non_empty(value),
            _ => return bad_request("The document upload contains an unknown field"),
        }
    }
    let Some(series_id) = series_id else {
        return bad_request("Choose a classification");
    };
    let Some(title) = title.filter(|v| !v.trim().is_empty()) else {
        return bad_request("Document title is required");
    };
    let Some(original_file_name) = original_file_name else {
        return bad_request("Choose a PDF, JPEG, or PNG document");
    };
    let Some(media_type) = media_type else {
        return bad_request("The document file type is missing");
    };
    let Some(bytes) = bytes else {
        return bad_request("Choose a PDF, JPEG, or PNG document");
    };
    if sensitivity.as_deref() == Some("restricted")
        && !allowed(&authority, "document_registry:restricted")
    {
        return forbidden();
    }
    created_or_error(
        DocumentRegistryOps::create_file(
            &pool,
            &storage,
            tenant_id(tenant),
            actor.into_inner(),
            context.into_inner(),
            NewRegistryFile {
                series_id,
                title,
                description,
                document_date,
                sensitivity,
                original_file_name,
                media_type,
                bytes,
            },
        )
        .await,
    )
}

#[get("/files/{id}")]
async fn read_file(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    path: web::Path<Uuid>,
) -> HttpResponse {
    if !allowed(&authority, "document_registry:view") {
        return forbidden();
    }
    found(
        DocumentRegistryOps::get_file(
            &pool,
            tenant_id(tenant),
            path.into_inner(),
            allowed(&authority, "document_registry:restricted"),
        )
        .await,
        "Document",
    )
}
#[put("/files/{id}")]
async fn update_file(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<UpdateFileRequest>,
) -> HttpResponse {
    if !allowed(&authority, "document_registry:edit")
        || (body.sensitivity == "restricted"
            && !allowed(&authority, "document_registry:restricted"))
    {
        return forbidden();
    }
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    updated_or_error(
        DocumentRegistryOps::update_file(
            &pool,
            tenant_id(tenant),
            path.into_inner(),
            actor.into_inner(),
            context.into_inner(),
            &body,
        )
        .await,
        "Document",
    )
}
#[post("/files/{id}/reclassify")]
async fn reclassify_file(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<ReclassifyFileRequest>,
) -> HttpResponse {
    if !allowed(&authority, "document_registry:classify")
        || (body.sensitivity.as_deref() == Some("restricted")
            && !allowed(&authority, "document_registry:restricted"))
    {
        return forbidden();
    }
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    updated_or_error(
        DocumentRegistryOps::reclassify_file(
            &pool,
            tenant_id(tenant),
            path.into_inner(),
            actor.into_inner(),
            context.into_inner(),
            &body,
        )
        .await,
        "Document",
    )
}
#[post("/files/{id}/close")]
async fn close_file(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<CloseFileRequest>,
) -> HttpResponse {
    if !allowed(&authority, "document_registry:close") {
        return forbidden();
    }
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    updated_or_error(
        DocumentRegistryOps::close_file(
            &pool,
            tenant_id(tenant),
            path.into_inner(),
            actor.into_inner(),
            context.into_inner(),
            &body,
        )
        .await,
        "Document",
    )
}
#[get("/files/{id}/activity")]
async fn file_activity(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    path: web::Path<Uuid>,
) -> HttpResponse {
    if !allowed(&authority, "document_registry:view") {
        return forbidden();
    }
    value_or_error(
        DocumentRegistryOps::activity(
            &pool,
            tenant_id(tenant),
            path.into_inner(),
            allowed(&authority, "document_registry:restricted"),
        )
        .await
        .map(|activity| ActivityPage { activity }),
    )
}
#[get("/files/{id}/download")]
async fn download_file(
    pool: web::Data<PgPool>,
    storage: web::Data<DocumentStorage>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    path: web::Path<Uuid>,
) -> HttpResponse {
    if !allowed(&authority, "document_registry:view") {
        return forbidden();
    }
    match DocumentRegistryOps::object_key(
        &pool,
        tenant_id(tenant),
        path.into_inner(),
        allowed(&authority, "document_registry:restricted"),
    )
    .await
    {
        Ok(Some(key)) => match storage.download_url(&key, 60).await {
            Ok(url) => ok(DownloadResponse {
                url,
                expires_in_seconds: 60,
            }),
            Err(error) => operation_error(error),
        },
        Ok(None) => not_found("Document"),
        Err(error) => operation_error(error),
    }
}

#[get("/retention-due")]
async fn retention_due(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
) -> HttpResponse {
    if !allowed(&authority, "document_registry:dispose") {
        return forbidden();
    }
    value_or_error(
        DocumentRegistryOps::retention_due(
            &pool,
            tenant_id(tenant),
            allowed(&authority, "document_registry:restricted"),
        )
        .await
        .map(|files| FilesPage { files }),
    )
}
#[get("/disposition-reviews")]
async fn list_reviews(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    query: web::Query<RegistryListQuery>,
) -> HttpResponse {
    if !allowed(&authority, "document_registry:dispose") {
        return forbidden();
    }
    let (page, per_page) = bounded_page(&query);
    match DocumentRegistryOps::list_reviews(
        &pool,
        tenant_id(tenant),
        &query,
        allowed(&authority, "document_registry:restricted"),
    )
    .await
    {
        Ok((reviews, total)) => paginated(ReviewsPage { reviews }, page, per_page, total),
        Err(error) => operation_error(error),
    }
}
#[get("/disposition-reviews/{id}")]
async fn read_review(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    path: web::Path<Uuid>,
) -> HttpResponse {
    if !allowed(&authority, "document_registry:dispose") {
        return forbidden();
    }
    found(
        DocumentRegistryOps::get_review(
            &pool,
            tenant_id(tenant),
            path.into_inner(),
            allowed(&authority, "document_registry:restricted"),
        )
        .await,
        "Disposition review",
    )
}
#[post("/files/{id}/disposition-reviews")]
async fn create_review(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<CreateReviewRequest>,
) -> HttpResponse {
    if !allowed(&authority, "document_registry:dispose") {
        return forbidden();
    }
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    created_or_error(
        DocumentRegistryOps::create_review(
            &pool,
            tenant_id(tenant),
            path.into_inner(),
            actor.into_inner(),
            context.into_inner(),
            &body,
        )
        .await,
    )
}
#[post("/disposition-reviews/{id}/approve")]
async fn approve_review(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<ReviewDecisionRequest>,
) -> HttpResponse {
    if !allowed(&authority, "document_registry:manage") {
        return forbidden();
    }
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    updated_or_error(
        DocumentRegistryOps::decide_review(
            &pool,
            tenant_id(tenant),
            path.into_inner(),
            actor.into_inner(),
            context.into_inner(),
            &body,
            true,
        )
        .await,
        "Disposition review",
    )
}
#[post("/disposition-reviews/{id}/reject")]
async fn reject_review(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<ReviewDecisionRequest>,
) -> HttpResponse {
    if !allowed(&authority, "document_registry:manage") {
        return forbidden();
    }
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    updated_or_error(
        DocumentRegistryOps::decide_review(
            &pool,
            tenant_id(tenant),
            path.into_inner(),
            actor.into_inner(),
            context.into_inner(),
            &body,
            false,
        )
        .await,
        "Disposition review",
    )
}
#[post("/disposition-reviews/{id}/execute")]
#[allow(clippy::too_many_arguments)]
async fn execute_review(
    pool: web::Data<PgPool>,
    storage: web::Data<DocumentStorage>,
    tenant: web::ReqData<TenantId>,
    authority: Authority,
    actor: web::ReqData<AuditActor>,
    context: web::ReqData<RequestContext>,
    path: web::Path<Uuid>,
    body: web::Json<ExecuteDestructionRequest>,
) -> HttpResponse {
    if !allowed(&authority, "document_registry:manage") {
        return forbidden();
    }
    if let Some(response) = validation_response(&body.0) {
        return response;
    }
    updated_or_error(
        DocumentRegistryOps::execute_destruction(
            &pool,
            &storage,
            tenant_id(tenant),
            path.into_inner(),
            actor.into_inner(),
            context.into_inner(),
            &body,
        )
        .await,
        "Disposition review",
    )
}

pub fn routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("")
            .wrap(RequirePermission::new("document_registry"))
            .service(numbering_policy)
            .service(update_numbering_policy)
            .service(list_series)
            .service(create_series)
            .service(read_series)
            .service(update_series)
            .service(list_files)
            .service(create_file)
            .service(read_file)
            .service(update_file)
            .service(reclassify_file)
            .service(close_file)
            .service(file_activity)
            .service(download_file)
            .service(retention_due)
            .service(list_reviews)
            .service(read_review)
            .service(create_review)
            .service(approve_review)
            .service(reject_review)
            .service(execute_review),
    );
}

fn allowed(authority: &Authority, permission: &str) -> bool {
    if !matches!(
        authority.1.effective_scope(
            &RecordScopeFamilyKey::parse("document_registry.records").expect("static scope")
        ),
        Some(EffectiveRecordScope::Campus)
    ) {
        return false;
    }
    authority.0.has_permission("*") || authority.0.has_permission(permission)
}
fn tenant_id(value: web::ReqData<TenantId>) -> Uuid {
    value.into_inner().into_inner()
}
fn bounded_page(query: &RegistryListQuery) -> (i64, i64) {
    (
        query.page.unwrap_or(1).max(1),
        query.per_page.unwrap_or(25).clamp(1, 100),
    )
}
fn non_empty(value: String) -> Option<String> {
    if value.trim().is_empty() {
        None
    } else {
        Some(value)
    }
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
        Ok(None) => conflict(&format!(
            "{label} changed since it was loaded, or is no longer available"
        )),
        Err(error) => operation_error(error),
    }
}
fn found<T: Serialize>(value: anyhow::Result<Option<T>>, label: &str) -> HttpResponse {
    match value {
        Ok(Some(value)) => ok(value),
        Ok(None) => not_found(label),
        Err(error) => operation_error(error),
    }
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
            "This Document Registry action is not available for this account".to_string(),
        ]),
    ))
}
fn bad_request(message: &str) -> HttpResponse {
    HttpResponse::BadRequest().json(ApiResponse::from_status(
        StatusCode::BAD_REQUEST,
        None::<()>,
        Some(vec![message.to_string()]),
    ))
}
fn conflict(message: &str) -> HttpResponse {
    HttpResponse::Conflict().json(ApiResponse::from_status(
        StatusCode::CONFLICT,
        None::<()>,
        Some(vec![message.to_string()]),
    ))
}
fn operation_error(error: anyhow::Error) -> HttpResponse {
    let message = error.to_string();
    if let Some(database) = error.root_cause().downcast_ref::<sqlx::Error>()
        && let sqlx::Error::Database(database) = database
        && database.code().as_deref() == Some("23505")
    {
        return conflict("That Document Registry record already exists");
    }
    if message.contains("changed") || message.contains("already") {
        return conflict(&message);
    }
    if [
        "A ",
        "Choose ",
        "Document ",
        "Next ",
        "Number ",
        "Only ",
        "Permanent ",
        "The ",
    ]
    .iter()
    .any(|prefix| message.starts_with(prefix))
    {
        return bad_request(&message);
    }
    HttpResponse::InternalServerError().json(ApiResponse::from_status(
        StatusCode::INTERNAL_SERVER_ERROR,
        None::<()>,
        Some(vec![
            "Document Registry could not complete the request".to_string(),
        ]),
    ))
}
async fn read_field(
    field: &mut actix_multipart::Field,
    maximum: usize,
) -> Result<Vec<u8>, &'static str> {
    let mut bytes = Vec::new();
    while let Some(chunk) = field.next().await {
        let chunk = chunk.map_err(|_| "The document upload could not be read")?;
        if bytes.len().saturating_add(chunk.len()) > maximum {
            return Err("The document upload exceeds its allowed size");
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

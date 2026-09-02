//! Scoped HTTP workflow for staged Gradebook mark imports.
//!
//! Raw source bytes stay inside the retained import store. Browser and Agent
//! responses contain only normalized destination previews and issue summaries.

use actix_multipart::Multipart;
use actix_web::http::StatusCode;
use actix_web::{HttpResponse, get, post, put, web};
use cp_audit::{AuditActor, RequestContext};
use cp_common::{AccessContext, ApiResponse, PaginationMeta, RecordScopeGrants, TenantId};
use cp_imports::{MAX_SOURCE_BYTES, parse_source};
use futures_util::StreamExt;
use sqlx::PgPool;
use uuid::Uuid;

use crate::imports::{
    CommitMarkImportRequest, GradebookMarkImportListResponse, GradebookMarkImportMapping,
    GradebookMarkImportOps, MarkImportListQuery, MarkImportPreviewQuery, NewGradebookMarkImport,
};
use crate::routes::{gradebook_access_scope, scope_mark_sheet};

type ImportAuthority = (
    web::ReqData<AuditActor>,
    web::ReqData<AccessContext>,
    web::ReqData<RecordScopeGrants>,
);

#[get("/mark-sheets/{mark_sheet_id}/imports")]
async fn list_mark_imports(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: ImportAuthority,
    path: web::Path<Uuid>,
    query: web::Query<MarkImportListQuery>,
) -> HttpResponse {
    let tenant_id = tenant_id(tenant);
    let mark_sheet_id = path.into_inner();
    if let Some(response) =
        authorize_sheet(pool.get_ref(), tenant_id, mark_sheet_id, authority).await
    {
        return response;
    }
    let (page, per_page) = bounded_page(query.page, query.per_page);
    match GradebookMarkImportOps::list(pool.get_ref(), tenant_id, mark_sheet_id, page, per_page)
        .await
    {
        Ok((imports, total)) => HttpResponse::Ok().json(ApiResponse::with_pagination(
            StatusCode::OK,
            Some(GradebookMarkImportListResponse { imports }),
            PaginationMeta::new(page as u32, per_page as u32, total),
            None,
        )),
        Err(_) => internal_error("Mark imports could not be loaded."),
    }
}

#[post("/mark-sheets/{mark_sheet_id}/imports")]
async fn upload_mark_import(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    actor: web::ReqData<AuditActor>,
    request_context: web::ReqData<RequestContext>,
    authority: (web::ReqData<AccessContext>, web::ReqData<RecordScopeGrants>),
    path: web::Path<Uuid>,
    mut payload: Multipart,
) -> HttpResponse {
    let tenant_id = tenant_id(tenant);
    let mark_sheet_id = path.into_inner();
    let scope_authority = (actor.clone(), authority.0, authority.1);
    if let Some(response) =
        authorize_sheet(pool.get_ref(), tenant_id, mark_sheet_id, scope_authority).await
    {
        return response;
    }
    let mut file_name = None;
    let mut content_type = None;
    let mut source_bytes = None;
    while let Some(item) = payload.next().await {
        let mut field = match item {
            Ok(field) => field,
            Err(_) => return bad_request("The import upload is malformed."),
        };
        let disposition = field.content_disposition();
        if disposition
            .and_then(|value| value.get_name())
            .unwrap_or_default()
            != "file"
        {
            return bad_request("The import upload contains an unknown field.");
        }
        if source_bytes.is_some() {
            return bad_request("Upload one import file at a time.");
        }
        file_name = disposition
            .and_then(|value| value.get_filename())
            .map(ToOwned::to_owned);
        content_type = field.content_type().map(ToString::to_string);
        source_bytes = match read_bounded_field(&mut field, MAX_SOURCE_BYTES).await {
            Ok(bytes) => Some(bytes),
            Err(message) => return bad_request(message),
        };
    }
    let (Some(file_name), Some(source_bytes)) = (file_name, source_bytes) else {
        return bad_request("Choose a CSV or XLSX file.");
    };
    let parse_name = file_name.clone();
    let parse_bytes = source_bytes.clone();
    let parsed = match web::block(move || parse_source(&parse_name, &parse_bytes)).await {
        Ok(Ok(parsed)) => parsed,
        Ok(Err(error)) => return bad_request(&error.to_string()),
        Err(_) => return internal_error("The import file could not be read."),
    };
    match GradebookMarkImportOps::create(
        pool.get_ref(),
        tenant_id,
        mark_sheet_id,
        actor.into_inner(),
        request_context.into_inner(),
        NewGradebookMarkImport {
            file_name,
            content_type: content_type.unwrap_or_else(|| "application/octet-stream".to_string()),
            source_bytes,
            parsed,
        },
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

#[get("/mark-sheets/{mark_sheet_id}/imports/{import_id}")]
async fn read_mark_import(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: ImportAuthority,
    path: web::Path<(Uuid, Uuid)>,
) -> HttpResponse {
    let tenant_id = tenant_id(tenant);
    let (mark_sheet_id, import_id) = path.into_inner();
    if let Some(response) =
        authorize_sheet(pool.get_ref(), tenant_id, mark_sheet_id, authority).await
    {
        return response;
    }
    match GradebookMarkImportOps::get(pool.get_ref(), tenant_id, mark_sheet_id, import_id).await {
        Ok(Some(value)) => ok(value),
        Ok(None) => not_found("Mark import"),
        Err(_) => internal_error("Mark import could not be loaded."),
    }
}

#[put("/mark-sheets/{mark_sheet_id}/imports/{import_id}/mapping")]
async fn preview_mark_import(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    actor: web::ReqData<AuditActor>,
    request_context: web::ReqData<RequestContext>,
    authority: (web::ReqData<AccessContext>, web::ReqData<RecordScopeGrants>),
    path: web::Path<(Uuid, Uuid)>,
    mapping: web::Json<GradebookMarkImportMapping>,
) -> HttpResponse {
    let tenant_id = tenant_id(tenant);
    let (mark_sheet_id, import_id) = path.into_inner();
    let scope_authority = (actor.clone(), authority.0, authority.1);
    if let Some(response) =
        authorize_sheet(pool.get_ref(), tenant_id, mark_sheet_id, scope_authority).await
    {
        return response;
    }
    let source = match GradebookMarkImportOps::retained_source(
        pool.get_ref(),
        tenant_id,
        mark_sheet_id,
        import_id,
    )
    .await
    {
        Ok(Some(source)) => source,
        Ok(None) => return not_found("Mark import"),
        Err(_) => return internal_error("The retained mark source could not be loaded."),
    };
    let parse_name = source.file_name;
    let parse_bytes = source.source_bytes;
    let table = match web::block(move || parse_source(&parse_name, &parse_bytes)).await {
        Ok(Ok(parsed)) => parsed.table,
        Ok(Err(error)) => return bad_request(&error.to_string()),
        Err(_) => return internal_error("The retained mark source could not be read."),
    };
    match GradebookMarkImportOps::create_preview(
        pool.get_ref(),
        tenant_id,
        mark_sheet_id,
        actor.into_inner(),
        request_context.into_inner(),
        import_id,
        mapping.into_inner(),
        &table,
    )
    .await
    {
        Ok(value) => ok(value),
        Err(error) => operation_error(error),
    }
}

#[get("/mark-sheets/{mark_sheet_id}/imports/{import_id}/preview")]
async fn read_mark_import_preview(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    authority: ImportAuthority,
    path: web::Path<(Uuid, Uuid)>,
    query: web::Query<MarkImportPreviewQuery>,
) -> HttpResponse {
    let tenant_id = tenant_id(tenant);
    let (mark_sheet_id, import_id) = path.into_inner();
    if let Some(response) =
        authorize_sheet(pool.get_ref(), tenant_id, mark_sheet_id, authority).await
    {
        return response;
    }
    let (page, per_page) = bounded_page(query.page, query.per_page);
    match GradebookMarkImportOps::preview(
        pool.get_ref(),
        tenant_id,
        mark_sheet_id,
        import_id,
        page,
        per_page,
    )
    .await
    {
        Ok(Some(value)) => ok(value),
        Ok(None) => not_found("Mark import preview"),
        Err(_) => internal_error("Mark import preview could not be loaded."),
    }
}

#[post("/mark-sheets/{mark_sheet_id}/imports/{import_id}/commit")]
async fn commit_mark_import(
    pool: web::Data<PgPool>,
    tenant: web::ReqData<TenantId>,
    actor: web::ReqData<AuditActor>,
    request_context: web::ReqData<RequestContext>,
    authority: (web::ReqData<AccessContext>, web::ReqData<RecordScopeGrants>),
    path: web::Path<(Uuid, Uuid)>,
    body: web::Json<CommitMarkImportRequest>,
) -> HttpResponse {
    let tenant_id = tenant_id(tenant);
    let (mark_sheet_id, import_id) = path.into_inner();
    let scope_authority = (actor.clone(), authority.0, authority.1);
    if let Some(response) =
        authorize_sheet(pool.get_ref(), tenant_id, mark_sheet_id, scope_authority).await
    {
        return response;
    }
    match GradebookMarkImportOps::commit(
        pool.get_ref(),
        tenant_id,
        mark_sheet_id,
        actor.into_inner(),
        request_context.into_inner(),
        import_id,
        body.preview_id,
    )
    .await
    {
        Ok(value) => ok(value),
        Err(error) => operation_error(error),
    }
}

pub fn routes(cfg: &mut web::ServiceConfig) {
    cfg.service(list_mark_imports)
        .service(upload_mark_import)
        .service(read_mark_import)
        .service(preview_mark_import)
        .service(read_mark_import_preview)
        .service(commit_mark_import);
}

async fn authorize_sheet(
    pool: &PgPool,
    tenant_id: Uuid,
    mark_sheet_id: Uuid,
    authority: ImportAuthority,
) -> Option<HttpResponse> {
    let (actor, access, grants) = authority;
    let Ok(scope) = gradebook_access_scope(&access, &grants, actor.into_inner()) else {
        return Some(forbidden());
    };
    scope_mark_sheet(pool, tenant_id, mark_sheet_id, scope).await
}

async fn read_bounded_field(
    field: &mut actix_multipart::Field,
    maximum: usize,
) -> Result<Vec<u8>, &'static str> {
    let mut bytes = Vec::new();
    while let Some(chunk) = field.next().await {
        let chunk = chunk.map_err(|_| "The import upload could not be read.")?;
        if bytes.len().saturating_add(chunk.len()) > maximum {
            return Err("The import file exceeds the 5 MB limit.");
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
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

fn ok<T: serde::Serialize>(value: T) -> HttpResponse {
    HttpResponse::Ok().json(ApiResponse::from_status(StatusCode::OK, Some(value), None))
}

fn bad_request(message: &str) -> HttpResponse {
    HttpResponse::BadRequest().json(ApiResponse::from_status(
        StatusCode::BAD_REQUEST,
        None::<()>,
        Some(vec![message.to_string()]),
    ))
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
        Some(vec!["Gradebook record scope is unavailable".to_string()]),
    ))
}

fn operation_error(error: anyhow::Error) -> HttpResponse {
    let diagnostic = format!("{error:#}");
    let message = error.to_string();
    if message.contains("changed") || message.contains("committed") || message.contains("no longer")
    {
        return HttpResponse::Conflict().json(ApiResponse::from_status(
            StatusCode::CONFLICT,
            None::<()>,
            Some(vec![message]),
        ));
    }
    if ["A ", "An ", "Only ", "The ", "This ", "Map "]
        .iter()
        .any(|prefix| message.starts_with(prefix))
    {
        return bad_request(&message);
    }
    log::error!("Gradebook mark import failed: {diagnostic}");
    internal_error("Gradebook could not complete the mark import.")
}

fn internal_error(message: &str) -> HttpResponse {
    HttpResponse::InternalServerError().json(ApiResponse::from_status(
        StatusCode::INTERNAL_SERVER_ERROR,
        None::<()>,
        Some(vec![message.to_string()]),
    ))
}

//
//  cp-hr-payroll
//  routes.rs
//
//  Created by OpenAI Codex on 2026/08/27.
//  Copyright (c) 2025 Codecraft Solutions. All rights reserved.
//

use actix_multipart::Multipart;
use actix_web::http::StatusCode;
use actix_web::{HttpMessage, HttpRequest, HttpResponse, delete, get, post, put, web};
use cp_audit::{AuditActor, RequestContext};
use cp_common::{
    ApiResponse, PaginationMeta, RequirePermission, TenantId, flatten_validation_errors,
};
use cp_imports::{MAX_SOURCE_BYTES, parse_source};
use futures_util::StreamExt;
use uuid::Uuid;
use validator::Validate;

use crate::{
    dtos::{
        CreateDepartmentRequest, CreateEmployeeAvailabilityRequest, CreateEmployeeRequest,
        CreateEmploymentEngagementRequest, CreatePositionRequest, DepartmentResponse,
        DirectoryListQuery, DirectoryStatus, EmployeeAvailabilityListQuery,
        EmployeeAvailabilityResponse, EmployeeListQuery, EmployeeResponse,
        EmploymentEngagementListQuery, EmploymentEngagementResponse, LinkEmployeeAccountRequest,
        PaginatedDepartmentsResponse, PaginatedEmployeeAvailabilityResponse,
        PaginatedEmployeesResponse, PaginatedEmploymentEngagementsResponse,
        PaginatedPositionsResponse, PositionResponse, UpdateDepartmentRequest,
        UpdateEmployeeAvailabilityRequest, UpdateEmployeeRequest,
        UpdateEmploymentEngagementRequest, UpdatePositionRequest,
    },
    imports::{
        CommitImportRequest, HrImportListResponse, HrImportMapping, HrImportOps, ImportListQuery,
        NewHrImport, PreviewRowsQuery,
    },
    ops::{
        DeleteOutcome, DepartmentOps, EmployeeAvailabilityOps, EmployeeOps,
        EmploymentEngagementOps, PositionOps,
    },
};

#[get("")]
async fn list_imports(
    pool: web::Data<sqlx::PgPool>,
    tenant: web::ReqData<TenantId>,
    query: web::Query<ImportListQuery>,
) -> HttpResponse {
    let (page, per_page) = bounded_page(query.page, query.per_page);
    match HrImportOps::list(
        pool.get_ref(),
        tenant.into_inner().into_inner(),
        page,
        per_page,
    )
    .await
    {
        Ok((imports, total)) => HttpResponse::Ok().json(ApiResponse::with_pagination(
            StatusCode::OK,
            Some(HrImportListResponse { imports }),
            PaginationMeta::new(page as u32, per_page as u32, total),
            None,
        )),
        Err(error) => internal_error("Employee imports could not be loaded", error),
    }
}

#[post("")]
async fn upload_import(
    pool: web::Data<sqlx::PgPool>,
    tenant: web::ReqData<TenantId>,
    actor: web::ReqData<AuditActor>,
    request_context: web::ReqData<RequestContext>,
    mut payload: Multipart,
) -> HttpResponse {
    let mut file_name = None;
    let mut content_type = None;
    let mut source_bytes = None;

    while let Some(item) = payload.next().await {
        let mut field = match item {
            Ok(field) => field,
            Err(_) => return bad_request("The import upload is malformed."),
        };
        let disposition = field.content_disposition();
        let field_name = disposition
            .and_then(|value| value.get_name())
            .unwrap_or_default();
        if field_name != "file" {
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

    let Some(file_name) = file_name else {
        return bad_request("Choose a CSV or XLSX file.");
    };
    let Some(source_bytes) = source_bytes else {
        return bad_request("Choose a CSV or XLSX file.");
    };
    let parse_name = file_name.clone();
    let parse_bytes = source_bytes.clone();
    let parsed = match web::block(move || parse_source(&parse_name, &parse_bytes)).await {
        Ok(Ok(parsed)) => parsed,
        Ok(Err(error)) => return bad_request(&error.to_string()),
        Err(_) => return internal_error_simple("The import file could not be read."),
    };
    import_created_or_error(
        HrImportOps::create(
            pool.get_ref(),
            tenant.into_inner().into_inner(),
            actor.into_inner(),
            request_context.into_inner(),
            NewHrImport {
                file_name,
                content_type: content_type
                    .unwrap_or_else(|| "application/octet-stream".to_string()),
                source_bytes,
                parsed,
            },
        )
        .await,
    )
}

#[get("/{id}")]
async fn read_import(
    pool: web::Data<sqlx::PgPool>,
    tenant: web::ReqData<TenantId>,
    id: web::Path<Uuid>,
) -> HttpResponse {
    match HrImportOps::get(
        pool.get_ref(),
        tenant.into_inner().into_inner(),
        id.into_inner(),
    )
    .await
    {
        Ok(Some(record)) => ok(record),
        Ok(None) => not_found("Employee import was not found"),
        Err(error) => internal_error("Employee import could not be loaded", error),
    }
}

#[put("/{id}/mapping")]
async fn preview_import_mapping(
    pool: web::Data<sqlx::PgPool>,
    tenant: web::ReqData<TenantId>,
    actor: web::ReqData<AuditActor>,
    request_context: web::ReqData<RequestContext>,
    id: web::Path<Uuid>,
    mapping: web::Json<HrImportMapping>,
) -> HttpResponse {
    let tenant_id = tenant.into_inner().into_inner();
    let import_id = id.into_inner();
    let source = match HrImportOps::retained_source(pool.get_ref(), tenant_id, import_id).await {
        Ok(Some(source)) => source,
        Ok(None) => return not_found("Employee import was not found"),
        Err(error) => {
            return internal_error("The retained import source could not be loaded", error);
        }
    };
    let parse_name = source.file_name;
    let parse_bytes = source.source_bytes;
    let table = match web::block(move || parse_source(&parse_name, &parse_bytes)).await {
        Ok(Ok(parsed)) => parsed.table,
        Ok(Err(error)) => return bad_request(&error.to_string()),
        Err(_) => return internal_error_simple("The retained import source could not be read."),
    };
    import_updated_or_error(
        HrImportOps::create_preview(
            pool.get_ref(),
            tenant_id,
            actor.into_inner(),
            request_context.into_inner(),
            import_id,
            mapping.into_inner(),
            &table,
        )
        .await,
    )
}

#[get("/{id}/preview")]
async fn read_import_preview(
    pool: web::Data<sqlx::PgPool>,
    tenant: web::ReqData<TenantId>,
    id: web::Path<Uuid>,
    query: web::Query<PreviewRowsQuery>,
) -> HttpResponse {
    let (page, per_page) = bounded_page(query.page, query.per_page);
    match HrImportOps::preview(
        pool.get_ref(),
        tenant.into_inner().into_inner(),
        id.into_inner(),
        page,
        per_page,
    )
    .await
    {
        Ok(Some(preview)) => ok(preview),
        Ok(None) => not_found("Employee import preview was not found"),
        Err(error) => internal_error("Employee import preview could not be loaded", error),
    }
}

#[post("/{id}/commit")]
async fn commit_import(
    pool: web::Data<sqlx::PgPool>,
    tenant: web::ReqData<TenantId>,
    actor: web::ReqData<AuditActor>,
    request_context: web::ReqData<RequestContext>,
    id: web::Path<Uuid>,
    body: web::Json<CommitImportRequest>,
) -> HttpResponse {
    import_updated_or_error(
        HrImportOps::commit(
            pool.get_ref(),
            tenant.into_inner().into_inner(),
            actor.into_inner(),
            request_context.into_inner(),
            id.into_inner(),
            body.preview_id,
        )
        .await,
    )
}

#[get("")]
async fn list_departments(
    pool: web::Data<sqlx::PgPool>,
    tenant: web::ReqData<TenantId>,
    query: web::Query<DirectoryListQuery<DirectoryStatus>>,
) -> HttpResponse {
    let (page, per_page) = bounded_page(query.page, query.per_page);
    match DepartmentOps::list(
        &pool,
        tenant.into_inner().into_inner(),
        page,
        per_page,
        trimmed(query.search.as_deref()),
        query.status.map(|value| value.as_str()),
    )
    .await
    {
        Ok((departments, total)) => HttpResponse::Ok().json(ApiResponse::with_pagination(
            StatusCode::OK,
            Some(PaginatedDepartmentsResponse {
                departments: departments
                    .into_iter()
                    .map(DepartmentResponse::from)
                    .collect(),
            }),
            PaginationMeta::new(page as u32, per_page as u32, total),
            None,
        )),
        Err(error) => internal_error("Departments could not be loaded", error),
    }
}

#[get("/{id}")]
async fn get_department(
    pool: web::Data<sqlx::PgPool>,
    tenant: web::ReqData<TenantId>,
    id: web::Path<Uuid>,
) -> HttpResponse {
    match DepartmentOps::get_by_id(&pool, tenant.into_inner().into_inner(), id.into_inner()).await {
        Ok(Some(value)) => ok(DepartmentResponse::from(value)),
        Ok(None) => not_found("Department was not found"),
        Err(error) => internal_error("Department could not be loaded", error),
    }
}

#[post("")]
async fn create_department(
    pool: web::Data<sqlx::PgPool>,
    tenant: web::ReqData<TenantId>,
    request: web::Json<CreateDepartmentRequest>,
) -> HttpResponse {
    if let Err(error) = request.validate() {
        return validation_error(error);
    }
    match DepartmentOps::create(&pool, tenant.into_inner().into_inner(), &request).await {
        Ok(value) => created(DepartmentResponse::from(value)),
        Err(error) => write_error("Department could not be created", error),
    }
}

#[put("/{id}")]
async fn update_department(
    pool: web::Data<sqlx::PgPool>,
    tenant: web::ReqData<TenantId>,
    id: web::Path<Uuid>,
    request: web::Json<UpdateDepartmentRequest>,
) -> HttpResponse {
    if let Err(error) = request.validate() {
        return validation_error(error);
    }
    match DepartmentOps::update(
        &pool,
        tenant.into_inner().into_inner(),
        id.into_inner(),
        &request,
    )
    .await
    {
        Ok(Some(value)) => ok(DepartmentResponse::from(value)),
        Ok(None) => not_found("Department was not found"),
        Err(error) => write_error("Department could not be updated", error),
    }
}

#[delete("/{id}")]
async fn delete_department(
    pool: web::Data<sqlx::PgPool>,
    tenant: web::ReqData<TenantId>,
    id: web::Path<Uuid>,
) -> HttpResponse {
    delete_response(
        DepartmentOps::delete(&pool, tenant.into_inner().into_inner(), id.into_inner()).await,
        "Department",
    )
}

#[get("")]
async fn list_positions(
    pool: web::Data<sqlx::PgPool>,
    tenant: web::ReqData<TenantId>,
    query: web::Query<DirectoryListQuery<DirectoryStatus>>,
) -> HttpResponse {
    let (page, per_page) = bounded_page(query.page, query.per_page);
    match PositionOps::list(
        &pool,
        tenant.into_inner().into_inner(),
        page,
        per_page,
        trimmed(query.search.as_deref()),
        query.status.map(|value| value.as_str()),
    )
    .await
    {
        Ok((positions, total)) => HttpResponse::Ok().json(ApiResponse::with_pagination(
            StatusCode::OK,
            Some(PaginatedPositionsResponse {
                positions: positions.into_iter().map(PositionResponse::from).collect(),
            }),
            PaginationMeta::new(page as u32, per_page as u32, total),
            None,
        )),
        Err(error) => internal_error("Positions could not be loaded", error),
    }
}

#[get("/{id}")]
async fn get_position(
    pool: web::Data<sqlx::PgPool>,
    tenant: web::ReqData<TenantId>,
    id: web::Path<Uuid>,
) -> HttpResponse {
    match PositionOps::get_by_id(&pool, tenant.into_inner().into_inner(), id.into_inner()).await {
        Ok(Some(value)) => ok(PositionResponse::from(value)),
        Ok(None) => not_found("Position was not found"),
        Err(error) => internal_error("Position could not be loaded", error),
    }
}

#[post("")]
async fn create_position(
    pool: web::Data<sqlx::PgPool>,
    tenant: web::ReqData<TenantId>,
    request: web::Json<CreatePositionRequest>,
) -> HttpResponse {
    if let Err(error) = request.validate() {
        return validation_error(error);
    }
    match PositionOps::create(&pool, tenant.into_inner().into_inner(), &request).await {
        Ok(value) => created(PositionResponse::from(value)),
        Err(error) => write_error("Position could not be created", error),
    }
}

#[put("/{id}")]
async fn update_position(
    pool: web::Data<sqlx::PgPool>,
    tenant: web::ReqData<TenantId>,
    id: web::Path<Uuid>,
    request: web::Json<UpdatePositionRequest>,
) -> HttpResponse {
    if let Err(error) = request.validate() {
        return validation_error(error);
    }
    match PositionOps::update(
        &pool,
        tenant.into_inner().into_inner(),
        id.into_inner(),
        &request,
    )
    .await
    {
        Ok(Some(value)) => ok(PositionResponse::from(value)),
        Ok(None) => not_found("Position was not found"),
        Err(error) => write_error("Position could not be updated", error),
    }
}

#[delete("/{id}")]
async fn delete_position(
    pool: web::Data<sqlx::PgPool>,
    tenant: web::ReqData<TenantId>,
    id: web::Path<Uuid>,
) -> HttpResponse {
    delete_response(
        PositionOps::delete(&pool, tenant.into_inner().into_inner(), id.into_inner()).await,
        "Position",
    )
}

#[get("")]
async fn list_employees(
    pool: web::Data<sqlx::PgPool>,
    tenant: web::ReqData<TenantId>,
    query: web::Query<EmployeeListQuery>,
) -> HttpResponse {
    let (page, per_page) = bounded_page(query.page, query.per_page);
    match EmployeeOps::list(
        &pool,
        tenant.into_inner().into_inner(),
        page,
        per_page,
        trimmed(query.search.as_deref()),
        query.status.map(|value| value.as_str()),
        query.department_id,
        query.position_id,
        query.account_linked,
    )
    .await
    {
        Ok((employees, total)) => HttpResponse::Ok().json(ApiResponse::with_pagination(
            StatusCode::OK,
            Some(PaginatedEmployeesResponse {
                employees: employees.into_iter().map(EmployeeResponse::from).collect(),
            }),
            PaginationMeta::new(page as u32, per_page as u32, total),
            None,
        )),
        Err(error) => internal_error("Employees could not be loaded", error),
    }
}

#[get("/{id}")]
async fn get_employee(
    pool: web::Data<sqlx::PgPool>,
    tenant: web::ReqData<TenantId>,
    id: web::Path<Uuid>,
) -> HttpResponse {
    match EmployeeOps::get_by_id(&pool, tenant.into_inner().into_inner(), id.into_inner()).await {
        Ok(Some(value)) => ok(EmployeeResponse::from(value)),
        Ok(None) => not_found("Employee was not found"),
        Err(error) => internal_error("Employee could not be loaded", error),
    }
}

#[post("")]
async fn create_employee(
    pool: web::Data<sqlx::PgPool>,
    tenant: web::ReqData<TenantId>,
    request: web::Json<CreateEmployeeRequest>,
) -> HttpResponse {
    if let Err(error) = request.validate() {
        return validation_error(error);
    }
    match EmployeeOps::create(&pool, tenant.into_inner().into_inner(), &request).await {
        Ok(value) => created(EmployeeResponse::from(value)),
        Err(error) => write_error("Employee could not be created", error),
    }
}

#[put("/{id}")]
async fn update_employee(
    pool: web::Data<sqlx::PgPool>,
    tenant: web::ReqData<TenantId>,
    id: web::Path<Uuid>,
    request: web::Json<UpdateEmployeeRequest>,
) -> HttpResponse {
    if let Err(error) = request.validate() {
        return validation_error(error);
    }
    match EmployeeOps::update(
        &pool,
        tenant.into_inner().into_inner(),
        id.into_inner(),
        &request,
    )
    .await
    {
        Ok(Some(value)) => ok(EmployeeResponse::from(value)),
        Ok(None) => not_found("Employee was not found"),
        Err(error) => write_error("Employee could not be updated", error),
    }
}

#[put("/{id}/account")]
async fn link_employee_account(
    pool: web::Data<sqlx::PgPool>,
    tenant: web::ReqData<TenantId>,
    id: web::Path<Uuid>,
    request: web::Json<LinkEmployeeAccountRequest>,
) -> HttpResponse {
    match EmployeeOps::link_account(
        &pool,
        tenant.into_inner().into_inner(),
        id.into_inner(),
        request.account_id,
    )
    .await
    {
        Ok(Some(value)) => ok(EmployeeResponse::from(value)),
        Ok(None) => not_found("Employee was not found"),
        Err(error) => write_error("Employee account link could not be updated", error),
    }
}

#[delete("/{id}")]
async fn delete_employee(
    pool: web::Data<sqlx::PgPool>,
    tenant: web::ReqData<TenantId>,
    id: web::Path<Uuid>,
) -> HttpResponse {
    delete_response(
        EmployeeOps::delete(&pool, tenant.into_inner().into_inner(), id.into_inner()).await,
        "Employee",
    )
}

#[get("")]
async fn list_employment_engagements(
    pool: web::Data<sqlx::PgPool>,
    tenant: web::ReqData<TenantId>,
    query: web::Query<EmploymentEngagementListQuery>,
) -> HttpResponse {
    let (page, per_page) = bounded_page(query.page, query.per_page);
    match EmploymentEngagementOps::list(
        &pool,
        tenant.into_inner().into_inner(),
        page,
        per_page,
        trimmed(query.search.as_deref()),
        query.employee_id,
        query.status.map(|value| value.as_str()),
        query.employment_type.map(|value| value.as_str()),
    )
    .await
    {
        Ok((engagements, total)) => HttpResponse::Ok().json(ApiResponse::with_pagination(
            StatusCode::OK,
            Some(PaginatedEmploymentEngagementsResponse {
                employment_engagements: engagements
                    .into_iter()
                    .map(EmploymentEngagementResponse::from)
                    .collect(),
            }),
            PaginationMeta::new(page as u32, per_page as u32, total),
            None,
        )),
        Err(error) => internal_error("Employment engagements could not be loaded", error),
    }
}

#[get("/{id}")]
async fn get_employment_engagement(
    pool: web::Data<sqlx::PgPool>,
    tenant: web::ReqData<TenantId>,
    id: web::Path<Uuid>,
) -> HttpResponse {
    match EmploymentEngagementOps::get_by_id(
        &pool,
        tenant.into_inner().into_inner(),
        id.into_inner(),
    )
    .await
    {
        Ok(Some(value)) => ok(EmploymentEngagementResponse::from(value)),
        Ok(None) => not_found("Employment engagement was not found"),
        Err(error) => internal_error("Employment engagement could not be loaded", error),
    }
}

#[post("")]
async fn create_employment_engagement(
    pool: web::Data<sqlx::PgPool>,
    tenant: web::ReqData<TenantId>,
    request: web::Json<CreateEmploymentEngagementRequest>,
) -> HttpResponse {
    if let Err(error) = request.validate() {
        return validation_error(error);
    }
    match EmploymentEngagementOps::create(&pool, tenant.into_inner().into_inner(), &request).await {
        Ok(value) => created(EmploymentEngagementResponse::from(value)),
        Err(error) => write_error("Employment engagement could not be created", error),
    }
}

#[put("/{id}")]
async fn update_employment_engagement(
    pool: web::Data<sqlx::PgPool>,
    tenant: web::ReqData<TenantId>,
    id: web::Path<Uuid>,
    request: web::Json<UpdateEmploymentEngagementRequest>,
) -> HttpResponse {
    if let Err(error) = request.validate() {
        return validation_error(error);
    }
    match EmploymentEngagementOps::update(
        &pool,
        tenant.into_inner().into_inner(),
        id.into_inner(),
        &request,
    )
    .await
    {
        Ok(Some(value)) => ok(EmploymentEngagementResponse::from(value)),
        Ok(None) => not_found("Employment engagement was not found"),
        Err(error) => write_error("Employment engagement could not be updated", error),
    }
}

#[delete("/{id}")]
async fn delete_employment_engagement(
    pool: web::Data<sqlx::PgPool>,
    tenant: web::ReqData<TenantId>,
    id: web::Path<Uuid>,
) -> HttpResponse {
    match EmploymentEngagementOps::delete(&pool, tenant.into_inner().into_inner(), id.into_inner())
        .await
    {
        Ok(DeleteOutcome::Deleted) => ok(serde_json::json!({ "success": true })),
        Ok(DeleteOutcome::NotFound) => not_found("Employment engagement was not found"),
        Ok(DeleteOutcome::InUse) => HttpResponse::Conflict().json(ApiResponse::from_status(
            StatusCode::CONFLICT,
            None::<()>,
            Some(vec![
                "Only draft employment engagements can be removed".to_string(),
            ]),
        )),
        Err(error) => internal_error("Employment engagement could not be removed", error),
    }
}

#[get("")]
async fn list_employee_availability(
    pool: web::Data<sqlx::PgPool>,
    tenant: web::ReqData<TenantId>,
    query: web::Query<EmployeeAvailabilityListQuery>,
) -> HttpResponse {
    let (page, per_page) = bounded_page(query.page, query.per_page);
    match EmployeeAvailabilityOps::list(
        &pool,
        tenant.into_inner().into_inner(),
        page,
        per_page,
        trimmed(query.search.as_deref()),
        query.employee_id,
        query.status.map(|value| value.as_str()),
        query.kind.map(|value| value.as_str()),
        query.from,
        query.to,
    )
    .await
    {
        Ok((periods, total)) => HttpResponse::Ok().json(ApiResponse::with_pagination(
            StatusCode::OK,
            Some(PaginatedEmployeeAvailabilityResponse {
                availability_periods: periods
                    .into_iter()
                    .map(EmployeeAvailabilityResponse::from)
                    .collect(),
            }),
            PaginationMeta::new(page as u32, per_page as u32, total),
            None,
        )),
        Err(error) => internal_error("Employee availability could not be loaded", error),
    }
}

#[get("/{id}")]
async fn get_employee_availability(
    pool: web::Data<sqlx::PgPool>,
    tenant: web::ReqData<TenantId>,
    id: web::Path<Uuid>,
) -> HttpResponse {
    match EmployeeAvailabilityOps::get_by_id(
        &pool,
        tenant.into_inner().into_inner(),
        id.into_inner(),
    )
    .await
    {
        Ok(Some(value)) => ok(EmployeeAvailabilityResponse::from(value)),
        Ok(None) => not_found("Employee availability period was not found"),
        Err(error) => internal_error("Employee availability could not be loaded", error),
    }
}

#[post("")]
async fn create_employee_availability(
    pool: web::Data<sqlx::PgPool>,
    tenant: web::ReqData<TenantId>,
    http_request: HttpRequest,
    request: web::Json<CreateEmployeeAvailabilityRequest>,
) -> HttpResponse {
    if let Err(error) = request.validate() {
        return validation_error(error);
    }
    let Some(actor_user_id) = request_actor_user_id(&http_request) else {
        return missing_actor();
    };
    match EmployeeAvailabilityOps::create(
        &pool,
        tenant.into_inner().into_inner(),
        actor_user_id,
        &request,
    )
    .await
    {
        Ok(value) => created(EmployeeAvailabilityResponse::from(value)),
        Err(error) => write_error("Employee availability could not be created", error),
    }
}

#[put("/{id}")]
async fn update_employee_availability(
    pool: web::Data<sqlx::PgPool>,
    tenant: web::ReqData<TenantId>,
    http_request: HttpRequest,
    id: web::Path<Uuid>,
    request: web::Json<UpdateEmployeeAvailabilityRequest>,
) -> HttpResponse {
    if let Err(error) = request.validate() {
        return validation_error(error);
    }
    let Some(actor_user_id) = request_actor_user_id(&http_request) else {
        return missing_actor();
    };
    match EmployeeAvailabilityOps::update(
        &pool,
        tenant.into_inner().into_inner(),
        actor_user_id,
        id.into_inner(),
        &request,
    )
    .await
    {
        Ok(Some(value)) => ok(EmployeeAvailabilityResponse::from(value)),
        Ok(None) => not_found("Employee availability period was not found"),
        Err(error) => write_error("Employee availability could not be updated", error),
    }
}

#[delete("/{id}")]
async fn delete_employee_availability(
    pool: web::Data<sqlx::PgPool>,
    tenant: web::ReqData<TenantId>,
    id: web::Path<Uuid>,
) -> HttpResponse {
    match EmployeeAvailabilityOps::delete(&pool, tenant.into_inner().into_inner(), id.into_inner())
        .await
    {
        Ok(DeleteOutcome::Deleted) => ok(serde_json::json!({ "success": true })),
        Ok(DeleteOutcome::NotFound) => not_found("Employee availability period was not found"),
        Ok(DeleteOutcome::InUse) => HttpResponse::Conflict().json(ApiResponse::from_status(
            StatusCode::CONFLICT,
            None::<()>,
            Some(vec![
                "Only draft availability periods can be removed".to_string(),
            ]),
        )),
        Err(error) => internal_error("Employee availability period could not be removed", error),
    }
}

pub fn routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/imports")
            .wrap(RequirePermission::new("hr_payroll"))
            .service(list_imports)
            .service(upload_import)
            .service(read_import)
            .service(preview_import_mapping)
            .service(read_import_preview)
            .service(commit_import),
    )
    .service(
        web::scope("/departments")
            .wrap(RequirePermission::new("hr_payroll"))
            .service(list_departments)
            .service(get_department)
            .service(create_department)
            .service(update_department)
            .service(delete_department),
    )
    .service(
        web::scope("/positions")
            .wrap(RequirePermission::new("hr_payroll"))
            .service(list_positions)
            .service(get_position)
            .service(create_position)
            .service(update_position)
            .service(delete_position),
    )
    .service(
        web::scope("/employees")
            .wrap(RequirePermission::new("hr_payroll"))
            .service(list_employees)
            .service(get_employee)
            .service(create_employee)
            .service(update_employee)
            .service(link_employee_account)
            .service(delete_employee),
    )
    .service(
        web::scope("/employment-engagements")
            .wrap(RequirePermission::new("hr_payroll"))
            .service(list_employment_engagements)
            .service(get_employment_engagement)
            .service(create_employment_engagement)
            .service(update_employment_engagement)
            .service(delete_employment_engagement),
    )
    .service(
        web::scope("/availability")
            .wrap(RequirePermission::new("hr_payroll"))
            .service(list_employee_availability)
            .service(get_employee_availability)
            .service(create_employee_availability)
            .service(update_employee_availability)
            .service(delete_employee_availability),
    );
}

fn request_actor_user_id(request: &HttpRequest) -> Option<Uuid> {
    request
        .extensions()
        .get::<AuditActor>()
        .copied()
        .and_then(AuditActor::user_id)
}

fn missing_actor() -> HttpResponse {
    HttpResponse::Unauthorized().json(ApiResponse::from_status(
        StatusCode::UNAUTHORIZED,
        None::<()>,
        Some(vec!["Authenticated actor is required".to_string()]),
    ))
}

fn bounded_page(page: Option<i64>, per_page: Option<i64>) -> (i64, i64) {
    (
        page.unwrap_or(1).max(1),
        per_page.unwrap_or(20).clamp(1, 100),
    )
}

fn trimmed(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

fn ok<T: serde::Serialize>(value: T) -> HttpResponse {
    HttpResponse::Ok().json(ApiResponse::from_status(StatusCode::OK, Some(value), None))
}

fn created<T: serde::Serialize>(value: T) -> HttpResponse {
    HttpResponse::Created().json(ApiResponse::from_status(
        StatusCode::CREATED,
        Some(value),
        None,
    ))
}

fn bad_request(message: &str) -> HttpResponse {
    HttpResponse::BadRequest().json(ApiResponse::from_status(
        StatusCode::BAD_REQUEST,
        None::<()>,
        Some(vec![message.to_string()]),
    ))
}

fn import_created_or_error<T: serde::Serialize>(result: anyhow::Result<T>) -> HttpResponse {
    match result {
        Ok(value) => created(value),
        Err(error) => import_operation_error(error),
    }
}

fn import_updated_or_error<T: serde::Serialize>(result: anyhow::Result<T>) -> HttpResponse {
    match result {
        Ok(value) => ok(value),
        Err(error) => import_operation_error(error),
    }
}

fn import_operation_error(error: anyhow::Error) -> HttpResponse {
    if let Some(database) = error.root_cause().downcast_ref::<sqlx::Error>() {
        if let sqlx::Error::Database(database) = database
            && database.code().as_deref() == Some("23505")
        {
            return HttpResponse::Conflict().json(ApiResponse::from_status(
                StatusCode::CONFLICT,
                None::<()>,
                Some(vec![
                    "That employee conflicts with an existing record.".to_string(),
                ]),
            ));
        }
        return internal_error_simple("The employee import could not be saved.");
    }
    bad_request(&error.to_string())
}

fn internal_error_simple(message: &str) -> HttpResponse {
    HttpResponse::InternalServerError().json(ApiResponse::from_status(
        StatusCode::INTERNAL_SERVER_ERROR,
        None::<()>,
        Some(vec![message.to_string()]),
    ))
}

async fn read_bounded_field(
    field: &mut actix_multipart::Field,
    maximum_bytes: usize,
) -> Result<Vec<u8>, &'static str> {
    let mut bytes = Vec::new();
    while let Some(chunk) = field.next().await {
        let chunk = chunk.map_err(|_| "The import upload could not be read.")?;
        if bytes.len().saturating_add(chunk.len()) > maximum_bytes {
            return Err("The import file exceeds the 5 MB limit.");
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

fn not_found(message: &str) -> HttpResponse {
    HttpResponse::NotFound().json(ApiResponse::from_status(
        StatusCode::NOT_FOUND,
        None::<()>,
        Some(vec![message.to_string()]),
    ))
}

fn validation_error(error: validator::ValidationErrors) -> HttpResponse {
    HttpResponse::BadRequest().json(ApiResponse::from_status(
        StatusCode::BAD_REQUEST,
        None::<()>,
        Some(flatten_validation_errors(&error)),
    ))
}

fn write_error(message: &str, error: anyhow::Error) -> HttpResponse {
    log::warn!("{message}: {error:#}");
    HttpResponse::BadRequest().json(ApiResponse::from_status(
        StatusCode::BAD_REQUEST,
        None::<()>,
        Some(vec![message.to_string()]),
    ))
}

fn internal_error(message: &str, error: anyhow::Error) -> HttpResponse {
    log::error!("{message}: {error:#}");
    HttpResponse::InternalServerError().json(ApiResponse::from_status(
        StatusCode::INTERNAL_SERVER_ERROR,
        None::<()>,
        Some(vec![message.to_string()]),
    ))
}

fn delete_response(result: anyhow::Result<DeleteOutcome>, subject: &str) -> HttpResponse {
    match result {
        Ok(DeleteOutcome::Deleted) => ok(serde_json::json!({ "success": true })),
        Ok(DeleteOutcome::NotFound) => not_found(&format!("{subject} was not found")),
        Ok(DeleteOutcome::InUse) => HttpResponse::Conflict().json(ApiResponse::from_status(
            StatusCode::CONFLICT,
            None::<()>,
            Some(vec![format!("{subject} is still in use")]),
        )),
        Err(error) => internal_error(&format!("{subject} could not be removed"), error),
    }
}

#[cfg(test)]
mod tests {
    use super::{bounded_page, trimmed};

    #[test]
    fn directory_filters_are_bounded() {
        assert_eq!(bounded_page(Some(0), Some(500)), (1, 100));
        assert_eq!(trimmed(Some("  staff  ")), Some("staff"));
        assert_eq!(trimmed(Some("  ")), None);
    }
}

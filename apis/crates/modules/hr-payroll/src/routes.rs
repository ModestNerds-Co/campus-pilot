//
//  cp-hr-payroll
//  routes.rs
//
//  Created by OpenAI Codex on 2026/08/27.
//  Copyright (c) 2025 Codecraft Solutions. All rights reserved.
//

use actix_web::http::StatusCode;
use actix_web::{HttpResponse, delete, get, post, put, web};
use cp_common::{
    ApiResponse, PaginationMeta, RequirePermission, TenantId, flatten_validation_errors,
};
use uuid::Uuid;
use validator::Validate;

use crate::{
    dtos::{
        CreateDepartmentRequest, CreateEmployeeRequest, CreatePositionRequest, DepartmentResponse,
        DirectoryListQuery, DirectoryStatus, EmployeeListQuery, EmployeeResponse,
        LinkEmployeeAccountRequest, PaginatedDepartmentsResponse, PaginatedEmployeesResponse,
        PaginatedPositionsResponse, PositionResponse, UpdateDepartmentRequest,
        UpdateEmployeeRequest, UpdatePositionRequest,
    },
    ops::{DeleteOutcome, DepartmentOps, EmployeeOps, PositionOps},
};

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

pub fn routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
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
    );
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

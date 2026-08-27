//
//  cp-hr-payroll
//  dtos.rs
//
//  Created by OpenAI Codex on 2026/08/27.
//  Copyright (c) 2025 Codecraft Solutions. All rights reserved.
//

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

use crate::models::{Department, EmployeeWithDetails, Position};

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DirectoryStatus {
    Active,
    Inactive,
}

impl DirectoryStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Inactive => "inactive",
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EmploymentStatus {
    Active,
    Inactive,
    Suspended,
    Terminated,
}

impl EmploymentStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Inactive => "inactive",
            Self::Suspended => "suspended",
            Self::Terminated => "terminated",
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct DirectoryListQuery<S> {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
    pub search: Option<String>,
    pub status: Option<S>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreateDepartmentRequest {
    #[validate(length(min = 1, max = 40))]
    pub code: String,
    #[validate(length(min = 1, max = 160))]
    pub name: String,
    pub status: Option<DirectoryStatus>,
    #[validate(length(max = 2_000))]
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateDepartmentRequest {
    #[validate(length(min = 1, max = 40))]
    pub code: Option<String>,
    #[validate(length(min = 1, max = 160))]
    pub name: Option<String>,
    pub status: Option<DirectoryStatus>,
    #[validate(length(max = 2_000))]
    pub notes: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct DepartmentResponse {
    pub id: Uuid,
    pub code: String,
    pub name: String,
    pub status: String,
    pub notes: Option<String>,
}

impl From<Department> for DepartmentResponse {
    fn from(value: Department) -> Self {
        Self {
            id: value.id,
            code: value.code,
            name: value.name,
            status: value.status,
            notes: value.notes,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct PaginatedDepartmentsResponse {
    pub departments: Vec<DepartmentResponse>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreatePositionRequest {
    pub department_id: Option<Uuid>,
    #[validate(length(min = 1, max = 40))]
    pub code: String,
    #[validate(length(min = 1, max = 160))]
    pub title: String,
    pub status: Option<DirectoryStatus>,
    #[validate(length(max = 2_000))]
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdatePositionRequest {
    pub department_id: Option<Uuid>,
    #[validate(length(min = 1, max = 40))]
    pub code: Option<String>,
    #[validate(length(min = 1, max = 160))]
    pub title: Option<String>,
    pub status: Option<DirectoryStatus>,
    #[validate(length(max = 2_000))]
    pub notes: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct PositionResponse {
    pub id: Uuid,
    pub department_id: Option<Uuid>,
    pub code: String,
    pub title: String,
    pub status: String,
    pub notes: Option<String>,
}

impl From<Position> for PositionResponse {
    fn from(value: Position) -> Self {
        Self {
            id: value.id,
            department_id: value.department_id,
            code: value.code,
            title: value.title,
            status: value.status,
            notes: value.notes,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct PaginatedPositionsResponse {
    pub positions: Vec<PositionResponse>,
}

#[derive(Debug, Deserialize)]
pub struct EmployeeListQuery {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
    pub search: Option<String>,
    pub status: Option<EmploymentStatus>,
    pub department_id: Option<Uuid>,
    pub position_id: Option<Uuid>,
    pub account_linked: Option<bool>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreateEmployeeRequest {
    #[validate(length(min = 1, max = 80))]
    pub employee_number: String,
    #[validate(length(min = 1, max = 200))]
    pub display_name: String,
    #[validate(length(max = 120))]
    pub first_names: Option<String>,
    #[validate(length(max = 120))]
    pub surname: Option<String>,
    #[validate(email)]
    pub work_email: Option<String>,
    #[validate(length(max = 50))]
    pub phone: Option<String>,
    pub department_id: Option<Uuid>,
    pub position_id: Option<Uuid>,
    pub account_id: Option<Uuid>,
    pub employment_status: Option<EmploymentStatus>,
    pub hire_date: Option<NaiveDate>,
    pub end_date: Option<NaiveDate>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateEmployeeRequest {
    #[validate(length(min = 1, max = 80))]
    pub employee_number: Option<String>,
    #[validate(length(min = 1, max = 200))]
    pub display_name: Option<String>,
    #[validate(length(max = 120))]
    pub first_names: Option<String>,
    #[validate(length(max = 120))]
    pub surname: Option<String>,
    #[validate(email)]
    pub work_email: Option<String>,
    #[validate(length(max = 50))]
    pub phone: Option<String>,
    pub department_id: Option<Uuid>,
    pub position_id: Option<Uuid>,
    pub employment_status: Option<EmploymentStatus>,
    pub hire_date: Option<NaiveDate>,
    pub end_date: Option<NaiveDate>,
}

#[derive(Debug, Deserialize)]
pub struct LinkEmployeeAccountRequest {
    pub account_id: Option<Uuid>,
}

#[derive(Debug, Serialize)]
pub struct EmployeeResponse {
    pub id: Uuid,
    pub account_id: Option<Uuid>,
    pub account_email: Option<String>,
    pub employee_number: String,
    pub display_name: String,
    pub first_names: Option<String>,
    pub surname: Option<String>,
    pub work_email: Option<String>,
    pub phone: Option<String>,
    pub department_id: Option<Uuid>,
    pub department_name: Option<String>,
    pub position_id: Option<Uuid>,
    pub position_title: Option<String>,
    pub employment_status: String,
    pub hire_date: Option<NaiveDate>,
    pub end_date: Option<NaiveDate>,
}

impl From<EmployeeWithDetails> for EmployeeResponse {
    fn from(value: EmployeeWithDetails) -> Self {
        Self {
            id: value.id,
            account_id: value.account_id,
            account_email: value.account_email,
            employee_number: value.employee_number,
            display_name: value.display_name,
            first_names: value.first_names,
            surname: value.surname,
            work_email: value.work_email,
            phone: value.phone,
            department_id: value.department_id,
            department_name: value.department_name,
            position_id: value.position_id,
            position_title: value.position_title,
            employment_status: value.employment_status,
            hire_date: value.hire_date,
            end_date: value.end_date,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct PaginatedEmployeesResponse {
    pub employees: Vec<EmployeeResponse>,
}

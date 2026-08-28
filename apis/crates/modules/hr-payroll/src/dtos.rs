//
//  cp-hr-payroll
//  dtos.rs
//
//  Created by OpenAI Codex on 2026/08/27.
//  Copyright (c) 2025 Codecraft Solutions. All rights reserved.
//

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

use crate::models::{
    Department, EmployeeAvailabilityWithDetails, EmployeeWithDetails,
    EmploymentEngagementWithDetails, Position,
};

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

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EmploymentType {
    Permanent,
    FixedTerm,
    Temporary,
    Casual,
    Contractor,
    Intern,
}

impl EmploymentType {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Permanent => "permanent",
            Self::FixedTerm => "fixed_term",
            Self::Temporary => "temporary",
            Self::Casual => "casual",
            Self::Contractor => "contractor",
            Self::Intern => "intern",
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EngagementStatus {
    Draft,
    Active,
    Ended,
    Cancelled,
}

impl EngagementStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Active => "active",
            Self::Ended => "ended",
            Self::Cancelled => "cancelled",
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct EmploymentEngagementListQuery {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
    pub search: Option<String>,
    pub employee_id: Option<Uuid>,
    pub status: Option<EngagementStatus>,
    pub employment_type: Option<EmploymentType>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreateEmploymentEngagementRequest {
    pub employee_id: Uuid,
    #[validate(length(max = 80))]
    pub reference: Option<String>,
    pub employment_type: EmploymentType,
    pub department_id: Option<Uuid>,
    pub position_id: Option<Uuid>,
    pub status: Option<EngagementStatus>,
    pub start_date: NaiveDate,
    pub end_date: Option<NaiveDate>,
    #[validate(range(min = 1, max = 10_000))]
    pub workload_basis_points: Option<i32>,
    #[validate(length(max = 4_000))]
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateEmploymentEngagementRequest {
    #[validate(length(max = 80))]
    pub reference: Option<String>,
    pub employment_type: EmploymentType,
    pub department_id: Option<Uuid>,
    pub position_id: Option<Uuid>,
    pub status: EngagementStatus,
    pub start_date: NaiveDate,
    pub end_date: Option<NaiveDate>,
    #[validate(range(min = 1, max = 10_000))]
    pub workload_basis_points: i32,
    #[validate(length(max = 4_000))]
    pub notes: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct EmploymentEngagementResponse {
    pub id: Uuid,
    pub employee_id: Uuid,
    pub employee_number: String,
    pub employee_name: String,
    pub reference: Option<String>,
    pub employment_type: String,
    pub department_id: Option<Uuid>,
    pub department_name: Option<String>,
    pub position_id: Option<Uuid>,
    pub position_title: Option<String>,
    pub status: String,
    pub start_date: Option<NaiveDate>,
    pub end_date: Option<NaiveDate>,
    pub workload_basis_points: i32,
    pub notes: Option<String>,
}

impl From<EmploymentEngagementWithDetails> for EmploymentEngagementResponse {
    fn from(value: EmploymentEngagementWithDetails) -> Self {
        Self {
            id: value.id,
            employee_id: value.employee_id,
            employee_number: value.employee_number,
            employee_name: value.employee_name,
            reference: value.reference,
            employment_type: value.employment_type,
            department_id: value.department_id,
            department_name: value.department_name,
            position_id: value.position_id,
            position_title: value.position_title,
            status: value.status,
            start_date: value.start_date,
            end_date: value.end_date,
            workload_basis_points: value.workload_basis_points,
            notes: value.notes,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct PaginatedEmploymentEngagementsResponse {
    pub employment_engagements: Vec<EmploymentEngagementResponse>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AvailabilityKind {
    Leave,
    Training,
    Medical,
    Personal,
    Other,
}

impl AvailabilityKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Leave => "leave",
            Self::Training => "training",
            Self::Medical => "medical",
            Self::Personal => "personal",
            Self::Other => "other",
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AvailabilityStatus {
    Draft,
    Submitted,
    Approved,
    Rejected,
    Cancelled,
}

impl AvailabilityStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Submitted => "submitted",
            Self::Approved => "approved",
            Self::Rejected => "rejected",
            Self::Cancelled => "cancelled",
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct EmployeeAvailabilityListQuery {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
    pub search: Option<String>,
    pub employee_id: Option<Uuid>,
    pub status: Option<AvailabilityStatus>,
    pub kind: Option<AvailabilityKind>,
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreateEmployeeAvailabilityRequest {
    pub employee_id: Uuid,
    pub kind: AvailabilityKind,
    pub starts_at: DateTime<Utc>,
    pub ends_at: DateTime<Utc>,
    pub status: Option<AvailabilityStatus>,
    #[validate(length(max = 4_000))]
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateEmployeeAvailabilityRequest {
    pub kind: AvailabilityKind,
    pub starts_at: DateTime<Utc>,
    pub ends_at: DateTime<Utc>,
    pub status: AvailabilityStatus,
    #[validate(length(max = 4_000))]
    pub notes: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct EmployeeAvailabilityResponse {
    pub id: Uuid,
    pub employee_id: Uuid,
    pub employee_number: String,
    pub employee_name: String,
    pub kind: String,
    pub starts_at: DateTime<Utc>,
    pub ends_at: DateTime<Utc>,
    pub status: String,
    pub notes: Option<String>,
    pub decided_by: Option<Uuid>,
    pub decided_by_name: Option<String>,
    pub decided_at: Option<DateTime<Utc>>,
}

impl From<EmployeeAvailabilityWithDetails> for EmployeeAvailabilityResponse {
    fn from(value: EmployeeAvailabilityWithDetails) -> Self {
        Self {
            id: value.id,
            employee_id: value.employee_id,
            employee_number: value.employee_number,
            employee_name: value.employee_name,
            kind: value.kind,
            starts_at: value.starts_at,
            ends_at: value.ends_at,
            status: value.status,
            notes: value.notes,
            decided_by: value.decided_by,
            decided_by_name: value.decided_by_name,
            decided_at: value.decided_at,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct PaginatedEmployeeAvailabilityResponse {
    pub availability_periods: Vec<EmployeeAvailabilityResponse>,
}

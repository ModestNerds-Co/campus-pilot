//! HTTP contracts for department stock requests and atomic fulfilment.
//!
//! Quantities are exact scaled integers. Candidate responses expose minimum HR
//! labels only, and lifecycle writes carry optimistic versions and idempotency.

use chrono::{DateTime, NaiveDate, Utc};
use cp_hr_payroll::models::{StockRequestDepartmentReference, StockRequestEmployeeReference};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

#[derive(Debug, Deserialize)]
pub struct StockRequestCandidateQuery {
    pub search: Option<String>,
    pub department_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize)]
pub struct StockRequesterCandidateResponse {
    pub id: Uuid,
    pub employee_number: String,
    pub display_name: String,
    pub department_id: Uuid,
    pub department_code: String,
    pub department_name: String,
}

impl From<StockRequestEmployeeReference> for StockRequesterCandidateResponse {
    fn from(value: StockRequestEmployeeReference) -> Self {
        Self {
            id: value.id,
            employee_number: value.employee_number,
            display_name: value.display_name,
            department_id: value.department_id,
            department_code: value.department_code,
            department_name: value.department_name,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct StockRequestDepartmentResponse {
    pub id: Uuid,
    pub code: String,
    pub name: String,
}

impl From<StockRequestDepartmentReference> for StockRequestDepartmentResponse {
    fn from(value: StockRequestDepartmentReference) -> Self {
        Self {
            id: value.id,
            code: value.code,
            name: value.name,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct StockRequesterCandidatesResponse {
    pub employees: Vec<StockRequesterCandidateResponse>,
}

#[derive(Debug, Serialize)]
pub struct StockRequestDepartmentsResponse {
    pub departments: Vec<StockRequestDepartmentResponse>,
}

#[derive(Debug, Deserialize)]
pub struct StockRequestListQuery {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
    pub search: Option<String>,
    pub status: Option<String>,
    pub requester_employee_id: Option<Uuid>,
    pub department_id: Option<Uuid>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Validate)]
pub struct StockRequestLineInput {
    pub item_id: Uuid,
    #[validate(range(min = 1i64, max = 9007199254740991i64))]
    pub requested_quantity_minor: i64,
}

#[derive(Debug, Deserialize, Serialize, Validate)]
pub struct CreateStockRequest {
    pub requester_employee_id: Uuid,
    pub department_id: Uuid,
    #[validate(length(min = 1, max = 2000))]
    pub purpose: String,
    pub needed_by: Option<NaiveDate>,
    #[validate(length(min = 1, max = 200))]
    pub idempotency_key: String,
    #[validate(length(min = 1, max = 200), nested)]
    pub lines: Vec<StockRequestLineInput>,
}

#[derive(Debug, Deserialize, Serialize, Validate)]
pub struct UpdateStockRequest {
    pub requester_employee_id: Uuid,
    pub department_id: Uuid,
    #[validate(length(min = 1, max = 2000))]
    pub purpose: String,
    pub needed_by: Option<NaiveDate>,
    #[validate(range(min = 1))]
    pub expected_version: i32,
    #[validate(length(min = 1, max = 200))]
    pub idempotency_key: String,
    #[validate(length(min = 1, max = 200), nested)]
    pub lines: Vec<StockRequestLineInput>,
}

#[derive(Debug, Deserialize, Serialize, Validate)]
pub struct StockRequestVersionCommand {
    #[validate(range(min = 1))]
    pub expected_version: i32,
    #[validate(length(min = 1, max = 200))]
    pub idempotency_key: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, Validate)]
pub struct StockRequestApprovalLineInput {
    pub request_line_id: Uuid,
    #[validate(range(min = 0i64, max = 9007199254740991i64))]
    pub approved_quantity_minor: i64,
}

#[derive(Debug, Deserialize, Serialize, Validate)]
pub struct ApproveStockRequest {
    #[validate(range(min = 1))]
    pub expected_version: i32,
    #[validate(length(max = 1000))]
    pub note: Option<String>,
    #[validate(length(min = 1, max = 200))]
    pub idempotency_key: String,
    #[validate(length(min = 1, max = 200), nested)]
    pub lines: Vec<StockRequestApprovalLineInput>,
}

#[derive(Debug, Deserialize, Serialize, Validate)]
pub struct StockRequestReasonCommand {
    #[validate(range(min = 1))]
    pub expected_version: i32,
    #[validate(length(min = 1, max = 1000))]
    pub reason: String,
    #[validate(length(min = 1, max = 200))]
    pub idempotency_key: String,
}

#[derive(Debug, Deserialize, Serialize, Validate)]
pub struct CloseStockRequest {
    #[validate(range(min = 1))]
    pub expected_version: i32,
    #[validate(length(max = 1000))]
    pub note: Option<String>,
    #[validate(length(min = 1, max = 200))]
    pub idempotency_key: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, Validate)]
pub struct FulfilStockRequestLineInput {
    pub request_line_id: Uuid,
    pub store_id: Uuid,
    #[validate(range(min = 1i64, max = 9007199254740991i64))]
    pub quantity_minor: i64,
    #[validate(range(min = 0))]
    pub expected_balance_version: i32,
}

#[derive(Debug, Deserialize, Serialize, Validate)]
pub struct FulfilStockRequest {
    #[validate(range(min = 1))]
    pub expected_request_version: i32,
    pub effective_on: NaiveDate,
    #[validate(length(max = 2000))]
    pub reason: Option<String>,
    #[validate(length(min = 1, max = 200))]
    pub idempotency_key: String,
    #[validate(length(min = 1, max = 200), nested)]
    pub lines: Vec<FulfilStockRequestLineInput>,
}

#[derive(Debug, Clone, Serialize)]
pub struct StockRequestSummaryResponse {
    pub id: Uuid,
    pub request_number: String,
    pub requester_employee_id: Uuid,
    pub requester_employee_number: Option<String>,
    pub requester_name: Option<String>,
    pub department_id: Uuid,
    pub department_code: Option<String>,
    pub department_name: Option<String>,
    pub needed_by: Option<NaiveDate>,
    pub status: String,
    pub version: i32,
    pub line_count: i64,
    pub requested_quantity_minor: i64,
    pub approved_quantity_minor: i64,
    pub issued_quantity_minor: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct PaginatedStockRequestsResponse {
    pub requests: Vec<StockRequestSummaryResponse>,
}

#[derive(Debug, Clone, Serialize)]
pub struct StockRequestLineResponse {
    pub id: Uuid,
    pub line_number: i32,
    pub item_id: Uuid,
    pub item_number: String,
    pub item_name: String,
    pub unit_label: String,
    pub quantity_scale: i16,
    pub requested_quantity_minor: i64,
    pub approved_quantity_minor: Option<i64>,
    pub issued_quantity_minor: i64,
    pub remaining_quantity_minor: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct StockRequestEventResponse {
    pub event_type: String,
    pub from_status: Option<String>,
    pub to_status: String,
    pub request_version: i32,
    pub note: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct StockRequestFulfilmentLineResponse {
    pub request_line_id: Uuid,
    pub item_id: Uuid,
    pub item_number: String,
    pub item_name: String,
    pub store_id: Uuid,
    pub store_number: String,
    pub store_name: String,
    pub quantity_minor: i64,
    pub quantity_scale: i16,
    pub unit_label: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct StockRequestFulfilmentResponse {
    pub id: Uuid,
    pub movement_id: Uuid,
    pub movement_number: String,
    pub effective_on: NaiveDate,
    pub quantity_minor: i64,
    pub created_at: DateTime<Utc>,
    pub lines: Vec<StockRequestFulfilmentLineResponse>,
}

#[derive(Debug, Clone, Serialize)]
pub struct StockRequestResponse {
    #[serde(flatten)]
    pub summary: StockRequestSummaryResponse,
    pub purpose: String,
    pub submitted_at: Option<DateTime<Utc>>,
    pub decided_at: Option<DateTime<Utc>>,
    pub decision_note: Option<String>,
    pub cancelled_at: Option<DateTime<Utc>>,
    pub cancellation_note: Option<String>,
    pub closed_at: Option<DateTime<Utc>>,
    pub closure_note: Option<String>,
    pub lines: Vec<StockRequestLineResponse>,
    pub events: Vec<StockRequestEventResponse>,
    pub fulfilments: Vec<StockRequestFulfilmentResponse>,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct StockRequestBalancePreview {
    pub item_id: Uuid,
    pub store_id: Uuid,
    pub store_number: String,
    pub store_name: String,
    pub on_hand_minor: i64,
    pub quantity_scale: i16,
    pub unit_label: String,
    pub version: i32,
}

#[derive(Debug, Serialize)]
pub struct StockRequestFulfilmentPreviewResponse {
    pub request: StockRequestResponse,
    pub balances: Vec<StockRequestBalancePreview>,
}

#[derive(Debug, Serialize)]
pub struct FulfilStockRequestResponse {
    pub request: StockRequestResponse,
    pub movement_id: Uuid,
    pub movement_number: String,
}

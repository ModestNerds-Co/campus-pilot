//! Typed HTTP and Agent-facing contracts for Facilities operations.

use chrono::{DateTime, NaiveDate, Utc};
use cp_hr_payroll::models::EmployeeReference;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FacilitiesRequestScope {
    Denied,
    SelfRecord(Uuid),
    Campus,
}

impl FacilitiesRequestScope {
    #[must_use]
    pub const fn reporter_user_id(self) -> Option<Uuid> {
        match self {
            Self::Denied | Self::Campus => None,
            Self::SelfRecord(user_id) => Some(user_id),
        }
    }

    #[must_use]
    pub const fn is_campus(self) -> bool {
        matches!(self, Self::Campus)
    }

    #[must_use]
    pub const fn is_denied(self) -> bool {
        matches!(self, Self::Denied)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FacilitiesWorkOrderScope {
    Denied,
    AssignedAccount(Uuid),
    Campus,
}

impl FacilitiesWorkOrderScope {
    #[must_use]
    pub const fn assigned_account_id(self) -> Option<Uuid> {
        match self {
            Self::Denied | Self::Campus => None,
            Self::AssignedAccount(user_id) => Some(user_id),
        }
    }

    #[must_use]
    pub const fn is_campus(self) -> bool {
        matches!(self, Self::Campus)
    }

    #[must_use]
    pub const fn is_denied(self) -> bool {
        matches!(self, Self::Denied)
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FacilityLocationKind {
    Site,
    Building,
    Floor,
    Room,
    ExternalArea,
}

impl FacilityLocationKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Site => "site",
            Self::Building => "building",
            Self::Floor => "floor",
            Self::Room => "room",
            Self::ExternalArea => "external_area",
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FacilityPriority {
    Low,
    Normal,
    High,
    Urgent,
}

impl FacilityPriority {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Normal => "normal",
            Self::High => "high",
            Self::Urgent => "urgent",
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FacilityRequestStatus {
    Open,
    Assigned,
    Resolved,
    Closed,
    Cancelled,
}

impl FacilityRequestStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Assigned => "assigned",
            Self::Resolved => "resolved",
            Self::Closed => "closed",
            Self::Cancelled => "cancelled",
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FacilityWorkOrderStatus {
    Assigned,
    InProgress,
    ReadyForInspection,
    Completed,
    Cancelled,
}

impl FacilityWorkOrderStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Assigned => "assigned",
            Self::InProgress => "in_progress",
            Self::ReadyForInspection => "ready_for_inspection",
            Self::Completed => "completed",
            Self::Cancelled => "cancelled",
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FacilityInspectionOutcome {
    Pass,
    Fail,
}

impl FacilityInspectionOutcome {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pass => "pass",
            Self::Fail => "fail",
        }
    }
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreateFacilityLocationRequest {
    pub parent_id: Option<Uuid>,
    pub kind: FacilityLocationKind,
    #[validate(length(min = 1, max = 40))]
    pub code: String,
    #[validate(length(min = 1, max = 160))]
    pub name: String,
    pub capacity: Option<i32>,
    #[validate(length(max = 4000))]
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateFacilityLocationRequest {
    pub parent_id: Option<Uuid>,
    pub kind: FacilityLocationKind,
    #[validate(length(min = 1, max = 40))]
    pub code: String,
    #[validate(length(min = 1, max = 160))]
    pub name: String,
    pub capacity: Option<i32>,
    #[validate(length(max = 4000))]
    pub notes: Option<String>,
    #[validate(range(min = 1))]
    pub expected_version: i32,
}

#[derive(Debug, Deserialize, Validate)]
pub struct ArchiveFacilityLocationRequest {
    #[validate(range(min = 1))]
    pub expected_version: i32,
    #[validate(length(min = 1, max = 2000))]
    pub reason: String,
}

#[derive(Debug, Deserialize, Default)]
pub struct FacilityLocationQuery {
    pub parent_id: Option<Uuid>,
    pub kind: Option<FacilityLocationKind>,
    pub status: Option<String>,
    pub search: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FacilityLocationResponse {
    pub id: Uuid,
    pub parent_id: Option<Uuid>,
    pub parent_name: Option<String>,
    pub kind: String,
    pub code: String,
    pub name: String,
    pub status: String,
    pub capacity: Option<i32>,
    pub notes: Option<String>,
    pub version: i32,
    pub child_count: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreateFacilityServiceRequest {
    pub location_id: Uuid,
    pub priority: FacilityPriority,
    #[validate(length(min = 1, max = 200))]
    pub summary: String,
    #[validate(length(min = 1, max = 6000))]
    pub description: String,
}

#[derive(Debug, Deserialize, Validate)]
pub struct FacilityTransitionRequest {
    #[validate(range(min = 1))]
    pub expected_version: i32,
    #[validate(length(min = 1, max = 3000))]
    pub reason: String,
}

#[derive(Debug, Deserialize, Default)]
pub struct FacilityRequestQuery {
    pub status: Option<FacilityRequestStatus>,
    pub priority: Option<FacilityPriority>,
    pub location_id: Option<Uuid>,
    pub search: Option<String>,
    pub page: Option<i64>,
    pub per_page: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FacilityServiceRequestSummary {
    pub id: Uuid,
    pub reference: String,
    pub location_id: Uuid,
    pub location_name: String,
    pub reporter_user_id: Uuid,
    pub reporter_name: String,
    pub priority: String,
    pub summary: String,
    pub status: String,
    pub version: i32,
    pub work_order_id: Option<Uuid>,
    pub work_order_reference: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FacilityServiceRequestRecord {
    pub request: FacilityServiceRequestSummary,
    pub description: String,
    pub resolution_summary: Option<String>,
    pub resolved_at: Option<DateTime<Utc>>,
    pub closure_reason: Option<String>,
    pub closed_at: Option<DateTime<Utc>>,
    pub cancellation_reason: Option<String>,
    pub cancelled_at: Option<DateTime<Utc>>,
    pub history: Vec<FacilityEventResponse>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreateFacilityWorkOrderRequest {
    pub service_request_id: Uuid,
    pub assigned_employee_id: Uuid,
    #[validate(length(min = 1, max = 200))]
    pub title: String,
    #[validate(length(max = 6000))]
    pub instructions: Option<String>,
    pub target_date: Option<NaiveDate>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct FacilityWorkOrderTransitionRequest {
    #[validate(range(min = 1))]
    pub expected_version: i32,
}

#[derive(Debug, Deserialize, Validate)]
pub struct SubmitFacilityCompletionRequest {
    #[validate(range(min = 1))]
    pub expected_version: i32,
    #[validate(length(min = 1, max = 6000))]
    pub summary: String,
}

#[derive(Debug, Deserialize, Validate)]
pub struct InspectFacilityWorkOrderRequest {
    #[validate(range(min = 1))]
    pub expected_version: i32,
    pub outcome: FacilityInspectionOutcome,
    #[validate(length(min = 1, max = 6000))]
    pub notes: String,
}

#[derive(Debug, Deserialize, Default)]
pub struct FacilityWorkOrderQuery {
    pub status: Option<FacilityWorkOrderStatus>,
    pub assigned_employee_id: Option<Uuid>,
    pub location_id: Option<Uuid>,
    pub search: Option<String>,
    pub page: Option<i64>,
    pub per_page: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FacilityWorkOrderSummary {
    pub id: Uuid,
    pub reference: String,
    pub service_request_id: Uuid,
    pub service_request_reference: String,
    pub location_id: Uuid,
    pub location_name: String,
    pub assigned_employee_id: Uuid,
    pub assigned_employee_number: String,
    pub assigned_employee_name: String,
    pub title: String,
    pub target_date: Option<NaiveDate>,
    pub status: String,
    pub version: i32,
    pub inspection_count: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FacilityWorkOrderRecord {
    pub work_order: FacilityWorkOrderSummary,
    pub instructions: Option<String>,
    pub started_at: Option<DateTime<Utc>>,
    pub completion_summary: Option<String>,
    pub completion_submitted_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub cancellation_reason: Option<String>,
    pub cancelled_at: Option<DateTime<Utc>>,
    pub inspections: Vec<FacilityInspectionResponse>,
    pub history: Vec<FacilityEventResponse>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FacilityInspectionResponse {
    pub id: Uuid,
    pub outcome: String,
    pub notes: String,
    pub inspected_by: Uuid,
    pub inspector_name: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FacilityEventResponse {
    pub id: Uuid,
    pub service_request_id: Option<Uuid>,
    pub work_order_id: Option<Uuid>,
    pub event_type: String,
    pub actor_id: Uuid,
    pub actor_name: String,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, Default)]
pub struct FacilityReferenceQuery {
    pub search: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct FacilityReferenceData {
    pub locations: Vec<FacilityLocationResponse>,
    pub employees: Vec<EmployeeReference>,
}

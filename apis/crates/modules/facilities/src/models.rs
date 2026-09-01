//! Private SQL projections for Facilities persistence.

use chrono::{DateTime, NaiveDate, Utc};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, FromRow)]
pub(crate) struct FacilityLocationRow {
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

#[derive(Debug, Clone, FromRow)]
pub(crate) struct FacilityRequestRow {
    pub id: Uuid,
    pub reference: String,
    pub location_id: Uuid,
    pub location_name: String,
    pub reporter_user_id: Uuid,
    pub reporter_name: String,
    pub priority: String,
    pub summary: String,
    pub description: String,
    pub status: String,
    pub version: i32,
    pub work_order_id: Option<Uuid>,
    pub work_order_reference: Option<String>,
    pub resolution_summary: Option<String>,
    pub resolved_at: Option<DateTime<Utc>>,
    pub closure_reason: Option<String>,
    pub closed_at: Option<DateTime<Utc>>,
    pub cancellation_reason: Option<String>,
    pub cancelled_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
pub(crate) struct FacilityWorkOrderRow {
    pub id: Uuid,
    pub reference: String,
    pub service_request_id: Uuid,
    pub service_request_reference: String,
    pub location_id: Uuid,
    pub location_name: String,
    pub assigned_employee_id: Uuid,
    pub title: String,
    pub instructions: Option<String>,
    pub target_date: Option<NaiveDate>,
    pub status: String,
    pub version: i32,
    pub inspection_count: i64,
    pub started_at: Option<DateTime<Utc>>,
    pub completion_summary: Option<String>,
    pub completion_submitted_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub cancellation_reason: Option<String>,
    pub cancelled_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
pub(crate) struct FacilityInspectionRow {
    pub id: Uuid,
    pub outcome: String,
    pub notes: String,
    pub inspected_by: Uuid,
    pub inspector_name: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
pub(crate) struct FacilityEventRow {
    pub id: Uuid,
    pub service_request_id: Option<Uuid>,
    pub work_order_id: Option<Uuid>,
    pub event_type: String,
    pub actor_id: Uuid,
    pub actor_name: String,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
pub(crate) struct LockedFacilityRequest {
    pub reference: String,
    pub location_id: Uuid,
    pub status: String,
    pub version: i32,
}

#[derive(Debug, Clone, FromRow)]
pub(crate) struct LockedFacilityWorkOrder {
    pub reference: String,
    pub service_request_id: Uuid,
    pub status: String,
    pub version: i32,
}

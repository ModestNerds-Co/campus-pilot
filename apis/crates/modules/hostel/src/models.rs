//! Private persistence rows for Hostel and boarding operations.

use chrono::{DateTime, NaiveDate, Utc};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, FromRow)]
pub(crate) struct ResidenceRow {
    pub id: Uuid,
    pub code: String,
    pub name: String,
    pub description: Option<String>,
    pub status: String,
    pub version: i32,
    pub room_count: i64,
    pub bed_capacity: i64,
    pub occupied_count: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
pub(crate) struct RoomRow {
    pub id: Uuid,
    pub residence_id: Uuid,
    pub residence_code: String,
    pub residence_name: String,
    pub code: String,
    pub floor_label: Option<String>,
    pub capacity: i16,
    pub occupied_count: i64,
    pub status: String,
    pub version: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
pub(crate) struct AllocationRow {
    pub id: Uuid,
    pub learner_id: Uuid,
    pub room_id: Uuid,
    pub residence_id: Uuid,
    pub residence_code: String,
    pub residence_name: String,
    pub room_code: String,
    pub starts_on: NaiveDate,
    pub expected_end_on: Option<NaiveDate>,
    pub ended_on: Option<NaiveDate>,
    pub status: String,
    pub version: i32,
    pub previous_allocation_id: Option<Uuid>,
    pub decision_reason: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
pub(crate) struct PastoralRecordRow {
    pub id: Uuid,
    pub learner_id: Uuid,
    pub allocation_id: Option<Uuid>,
    pub residence_name: Option<String>,
    pub room_code: Option<String>,
    pub category: String,
    pub severity: String,
    pub subject: String,
    pub details: String,
    pub occurred_at: DateTime<Utc>,
    pub status: String,
    pub resolution: Option<String>,
    pub version: i32,
    pub resolved_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
pub(crate) struct PreviewRoomRow {
    pub id: Uuid,
    pub residence_name: String,
    pub code: String,
    pub capacity: i16,
    pub status: String,
    pub version: i32,
    pub occupied_count: i64,
}

//! Private SQL projections for Activities persistence and completion snapshots.

use chrono::{DateTime, NaiveDate, Utc};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, FromRow)]
pub(crate) struct CatalogRow {
    pub id: Uuid,
    pub code: String,
    pub name: String,
    pub category: String,
    pub description: Option<String>,
    pub status: String,
    pub version: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
pub(crate) struct GroupRow {
    pub id: Uuid,
    pub activity_id: Uuid,
    pub activity_code: String,
    pub activity_name: String,
    pub code: String,
    pub name: String,
    pub starts_on: NaiveDate,
    pub ends_on: NaiveDate,
    pub capacity: Option<i32>,
    pub consent_required: bool,
    pub consent_instructions: Option<String>,
    pub status: String,
    pub leader_count: i64,
    pub member_count: i64,
    pub session_count: i64,
    pub version: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
pub(crate) struct LeaderRow {
    pub id: Uuid,
    pub employee_id: Uuid,
    pub leader_role: String,
    pub starts_on: NaiveDate,
    pub ends_on: Option<NaiveDate>,
    pub ended_at: Option<DateTime<Utc>>,
    pub end_reason: Option<String>,
    pub version: i32,
}

#[derive(Debug, Clone, FromRow)]
pub(crate) struct MembershipRow {
    pub id: Uuid,
    pub learner_id: Uuid,
    pub joined_on: NaiveDate,
    pub ended_on: Option<NaiveDate>,
    pub status: String,
    pub consent_status: String,
    pub consent_recorded_at: Option<DateTime<Utc>>,
    pub consent_notes: Option<String>,
    pub version: i32,
}

#[derive(Debug, Clone, FromRow)]
pub(crate) struct SessionRow {
    pub id: Uuid,
    pub reference: String,
    pub group_id: Uuid,
    pub group_code: String,
    pub group_name: String,
    pub title: String,
    pub starts_at: DateTime<Utc>,
    pub ends_at: DateTime<Utc>,
    pub location_note: Option<String>,
    pub notes: Option<String>,
    pub status: String,
    pub completion_summary: Option<String>,
    pub cancellation_reason: Option<String>,
    pub roster_count: i64,
    pub marked_count: i64,
    pub present_count: i64,
    pub absent_count: i64,
    pub version: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
pub(crate) struct ParticipationRow {
    pub membership_id: Uuid,
    pub learner_id: Uuid,
    pub learner_number: String,
    pub learner_name: String,
    pub mark: Option<String>,
    pub notes: Option<String>,
    pub version: Option<i32>,
    pub marked_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, FromRow)]
pub(crate) struct EventRow {
    pub id: Uuid,
    pub event_type: String,
    pub actor_name: String,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
pub(crate) struct LockedGroup {
    pub activity_id: Uuid,
    pub starts_on: NaiveDate,
    pub ends_on: NaiveDate,
    pub capacity: Option<i32>,
    pub consent_required: bool,
    pub status: String,
    pub version: i32,
}

#[derive(Debug, Clone, FromRow)]
pub(crate) struct LockedSession {
    pub group_id: Uuid,
    pub reference: String,
    pub starts_at: DateTime<Utc>,
    pub status: String,
    pub version: i32,
}

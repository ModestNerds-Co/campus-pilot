//! Private persistence projections for Communication workflows.

use chrono::{DateTime, Utc};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, FromRow)]
pub(crate) struct AnnouncementRow {
    pub id: Uuid,
    pub title: String,
    pub body: String,
    pub priority: String,
    pub status: String,
    pub version: i32,
    pub created_by: Uuid,
    pub creator_name: String,
    pub submitted_at: Option<DateTime<Utc>>,
    pub published_at: Option<DateTime<Utc>>,
    pub cancelled_at: Option<DateTime<Utc>>,
    pub cancellation_reason: Option<String>,
    pub reopened_at: Option<DateTime<Utc>>,
    pub reopen_reason: Option<String>,
    pub recipient_count: i64,
    pub read_count: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
pub(crate) struct LockedAnnouncement {
    pub status: String,
    pub version: i32,
    pub created_by: Uuid,
}

#[derive(Debug, Clone, FromRow)]
pub(crate) struct AudienceTargetRow {
    pub id: Uuid,
    pub target_kind: String,
    pub target_id: Option<Uuid>,
    pub target_key: Option<String>,
    pub label_snapshot: String,
}

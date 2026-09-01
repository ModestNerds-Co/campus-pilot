//! Private persistence rows for Learning-owned state.

use chrono::{DateTime, Utc};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, FromRow)]
pub(crate) struct LearningSettingsRow {
    pub document_series_id: Option<Uuid>,
    pub version: i32,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
pub(crate) struct LearningSpaceRow {
    pub id: Uuid,
    pub teaching_assignment_id: Uuid,
    pub academic_year_id: Uuid,
    pub academic_term_id: Uuid,
    pub class_group_id: Uuid,
    pub title: String,
    pub summary: Option<String>,
    pub status: String,
    pub version: i32,
    pub published_at: Option<DateTime<Utc>>,
    pub archived_at: Option<DateTime<Utc>>,
    pub archive_reason: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub unit_count: i64,
    pub published_unit_count: i64,
}

#[derive(Debug, Clone, FromRow)]
pub(crate) struct LearningUnitRow {
    pub id: Uuid,
    pub learning_space_id: Uuid,
    pub position: i32,
    pub title: String,
    pub summary: Option<String>,
    pub status: String,
    pub version: i32,
    pub published_at: Option<DateTime<Utc>>,
    pub withdrawn_at: Option<DateTime<Utc>>,
    pub withdrawal_reason: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
pub(crate) struct LearningResourceRow {
    pub id: Uuid,
    pub learning_unit_id: Uuid,
    pub document_file_id: Uuid,
    pub display_title: String,
    pub sensitivity_snapshot: String,
    pub position: i32,
    pub status: String,
    pub version: i32,
    pub published_at: Option<DateTime<Utc>>,
    pub withdrawn_at: Option<DateTime<Utc>>,
    pub withdrawal_reason: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

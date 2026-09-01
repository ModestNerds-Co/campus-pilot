//! Private persistence rows for Gradebook mark sheets and learner marks.

use chrono::{DateTime, NaiveDate, Utc};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, FromRow)]
pub(crate) struct MarkSheetRow {
    pub assessment_component_id: Uuid,
    pub status: String,
    pub version: i32,
    pub reopened_at: Option<DateTime<Utc>>,
    pub reopen_reason: Option<String>,
}

#[derive(Debug, Clone, FromRow)]
pub(crate) struct MarkSheetSummaryRow {
    pub id: Uuid,
    pub assessment_component_id: Uuid,
    pub roster_on: NaiveDate,
    pub status: String,
    pub version: i32,
    pub learner_count: i64,
    pub scored_count: i64,
    pub absent_count: i64,
    pub exempt_count: i64,
    pub unmarked_count: i64,
    pub average_percentage_basis_points: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub submitted_at: Option<DateTime<Utc>>,
    pub published_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, FromRow)]
pub(crate) struct MarkRow {
    pub id: Uuid,
    pub enrolment_id: Uuid,
    pub learner_id: Uuid,
    pub mark_status: String,
    pub marks_awarded_hundredths: Option<i64>,
    pub note: Option<String>,
    pub version: i32,
    pub marked_at: Option<DateTime<Utc>>,
}

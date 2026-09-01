//! Private persistence rows for Attendance registers and roster marks.

use chrono::{DateTime, NaiveDate, Utc};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, FromRow)]
pub(crate) struct AttendanceRegisterRow {
    pub class_group_id: Uuid,
    pub status: String,
    pub version: i32,
    pub reopened_at: Option<DateTime<Utc>>,
    pub reopen_reason: Option<String>,
}

#[derive(Debug, Clone, FromRow)]
pub(crate) struct LearnerAttendanceHistoryRow {
    pub register_id: Uuid,
    pub class_group_id: Uuid,
    pub attendance_date: NaiveDate,
    pub period: String,
    pub mark: String,
    pub minutes_late: Option<i32>,
    pub note: Option<String>,
    pub submitted_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
pub(crate) struct AttendanceRegisterSummaryRow {
    pub id: Uuid,
    pub academic_term_id: Uuid,
    pub class_group_id: Uuid,
    pub attendance_date: NaiveDate,
    pub period: String,
    pub status: String,
    pub version: i32,
    pub learner_count: i64,
    pub present_count: i64,
    pub absent_count: i64,
    pub late_count: i64,
    pub excused_count: i64,
    pub unmarked_count: i64,
    pub created_at: DateTime<Utc>,
    pub submitted_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, FromRow)]
pub(crate) struct AttendanceMarkRow {
    pub id: Uuid,
    pub enrolment_id: Uuid,
    pub learner_id: Uuid,
    pub mark: String,
    pub minutes_late: Option<i32>,
    pub note: Option<String>,
    pub version: i32,
    pub marked_at: Option<DateTime<Utc>>,
}

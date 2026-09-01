//! Attendance transport contracts and parsed workflow values.
//!
//! Wire enums are closed, roster updates are versioned, and the server remains
//! authoritative for class membership and register lifecycle transitions.

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::{Validate, ValidationError};

/// Submitted attendance totals consumed by academic reporting.
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct AttendanceLearnerSummary {
    pub enrolment_id: Uuid,
    pub learner_id: Uuid,
    pub present_count: i64,
    pub absent_count: i64,
    pub late_count: i64,
    pub excused_count: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttendancePeriod {
    FullDay,
    Morning,
    Afternoon,
}

impl AttendancePeriod {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::FullDay => "full_day",
            Self::Morning => "morning",
            Self::Afternoon => "afternoon",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttendanceMarkStatus {
    Unmarked,
    Present,
    Absent,
    Late,
    Excused,
}

impl AttendanceMarkStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unmarked => "unmarked",
            Self::Present => "present",
            Self::Absent => "absent",
            Self::Late => "late",
            Self::Excused => "excused",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttendanceRegisterStatus {
    Draft,
    Submitted,
}

impl AttendanceRegisterStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Submitted => "submitted",
        }
    }
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreateAttendanceRegisterRequest {
    pub academic_term_id: Uuid,
    pub class_group_id: Uuid,
    pub attendance_date: NaiveDate,
    pub period: AttendancePeriod,
    #[validate(length(min = 1, max = 200))]
    pub idempotency_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
#[validate(schema(function = "validate_mark_input"))]
pub struct AttendanceMarkInput {
    pub learner_id: Uuid,
    pub mark: AttendanceMarkStatus,
    pub minutes_late: Option<i32>,
    #[validate(length(min = 1, max = 1000))]
    pub note: Option<String>,
}

fn validate_mark_input(value: &AttendanceMarkInput) -> Result<(), ValidationError> {
    if value
        .minutes_late
        .is_some_and(|minutes| !(0..=1440).contains(&minutes))
    {
        return Err(ValidationError::new("minutes_late_range"));
    }
    if value.mark != AttendanceMarkStatus::Late && value.minutes_late.is_some() {
        return Err(ValidationError::new("minutes_late_requires_late"));
    }
    if value.mark == AttendanceMarkStatus::Unmarked
        && (value.minutes_late.is_some() || value.note.is_some())
    {
        return Err(ValidationError::new("unmarked_has_details"));
    }
    Ok(())
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateAttendanceMarksRequest {
    #[validate(range(min = 1))]
    pub expected_version: i32,
    #[validate(length(min = 1, max = 500), nested)]
    pub marks: Vec<AttendanceMarkInput>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct SubmitAttendanceRegisterRequest {
    #[validate(range(min = 1))]
    pub expected_version: i32,
}

#[derive(Debug, Deserialize, Validate)]
pub struct ReopenAttendanceRegisterRequest {
    #[validate(range(min = 1))]
    pub expected_version: i32,
    #[validate(length(min = 1, max = 1000))]
    pub reason: String,
}

#[derive(Debug, Deserialize, Validate)]
pub struct DeleteAttendanceRegisterQuery {
    #[validate(range(min = 1))]
    pub expected_version: i32,
}

#[derive(Debug, Deserialize)]
pub struct AttendanceRegisterListQuery {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
    pub date_from: Option<NaiveDate>,
    pub date_to: Option<NaiveDate>,
    pub class_group_id: Option<Uuid>,
    pub period: Option<AttendancePeriod>,
    pub status: Option<AttendanceRegisterStatus>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AttendanceClassReference {
    pub id: Uuid,
    pub code: String,
    pub name: String,
    pub grade_level: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AttendanceTermReference {
    pub id: Uuid,
    pub academic_year_id: Uuid,
    pub academic_year_name: String,
    pub code: String,
    pub name: String,
    pub starts_on: NaiveDate,
    pub ends_on: NaiveDate,
}

#[derive(Debug, Clone, Serialize)]
pub struct AttendanceReferenceData {
    pub term: AttendanceTermReference,
    pub classes: Vec<AttendanceClassReference>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AttendanceRegisterSummary {
    pub id: Uuid,
    pub academic_term_id: Uuid,
    pub academic_term_name: String,
    pub class_group_id: Uuid,
    pub class_group_name: String,
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

#[derive(Debug, Clone, Serialize)]
pub struct AttendanceMarkResponse {
    pub id: Uuid,
    pub enrolment_id: Uuid,
    pub learner_id: Uuid,
    pub learner_number: String,
    pub learner_name: String,
    pub mark: String,
    pub minutes_late: Option<i32>,
    pub note: Option<String>,
    pub version: i32,
    pub marked_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AttendanceRegisterResponse {
    #[serde(flatten)]
    pub summary: AttendanceRegisterSummary,
    pub marks: Vec<AttendanceMarkResponse>,
    pub reopened_at: Option<DateTime<Utc>>,
    pub reopen_reason: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct PaginatedAttendanceRegistersResponse {
    pub registers: Vec<AttendanceRegisterSummary>,
}

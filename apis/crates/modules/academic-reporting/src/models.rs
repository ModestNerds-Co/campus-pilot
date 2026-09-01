//! Private persistence rows for academic reporting snapshots and review state.

use chrono::{DateTime, Utc};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, FromRow)]
pub(crate) struct GradingSchemeRow {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub is_default: bool,
    pub status: String,
    pub version: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
pub(crate) struct GradingBandRow {
    pub id: Uuid,
    pub code: String,
    pub label: String,
    pub minimum_basis_points: i16,
    pub is_pass: bool,
}

#[derive(Debug, Clone, FromRow)]
pub(crate) struct ReportBatchSummaryRow {
    pub id: Uuid,
    pub assessment_cycle_id: Uuid,
    pub assessment_cycle_name: String,
    pub academic_term_id: Uuid,
    pub academic_term_name: String,
    pub academic_year_id: Uuid,
    pub academic_year_name: String,
    pub class_group_id: Uuid,
    pub class_group_name: String,
    pub grading_scheme_id: Uuid,
    pub grading_scheme_name: String,
    pub grading_scheme_version: i32,
    pub status: String,
    pub version: i32,
    pub learner_count: i64,
    pub graded_subject_count: i64,
    pub incomplete_subject_count: i64,
    pub created_at: DateTime<Utc>,
    pub reviewed_at: Option<DateTime<Utc>>,
    pub published_at: Option<DateTime<Utc>>,
    pub reopened_at: Option<DateTime<Utc>>,
    pub reopen_reason: Option<String>,
    pub source_fingerprint: String,
}

#[derive(Debug, Clone, FromRow)]
pub(crate) struct ReportCardRow {
    pub id: Uuid,
    pub enrolment_id: Uuid,
    pub learner_id: Uuid,
    pub learner_number_snapshot: String,
    pub learner_name_snapshot: String,
    pub overall_percentage_basis_points: Option<i16>,
    pub overall_grade_code: Option<String>,
    pub overall_grade_label: Option<String>,
    pub teacher_comment: Option<String>,
    pub reviewer_comment: Option<String>,
    pub progression_outcome: String,
    pub target_grade_level_id: Option<Uuid>,
    pub target_grade_level_name: Option<String>,
    pub version: i32,
}

#[derive(Debug, Clone, FromRow)]
pub(crate) struct SubjectResultRow {
    pub id: Uuid,
    pub teaching_assignment_id: Uuid,
    pub subject_id: Uuid,
    pub subject_name: String,
    pub result_status: String,
    pub percentage_basis_points: Option<i16>,
    pub grade_code: Option<String>,
    pub grade_label: Option<String>,
    pub is_pass: Option<bool>,
    pub scored_component_count: i32,
    pub absent_component_count: i32,
    pub exempt_component_count: i32,
}

#[derive(Debug, Clone, FromRow)]
pub(crate) struct AttendanceSnapshotRow {
    pub present_count: i32,
    pub absent_count: i32,
    pub late_count: i32,
    pub excused_count: i32,
    pub attendance_percentage_basis_points: Option<i16>,
}

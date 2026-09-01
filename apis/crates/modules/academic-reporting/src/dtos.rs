//! Academic reporting transport contracts and parsed workflow values.
//!
//! Percentages use integer basis points. Every mutable record carries an
//! optimistic version and report source selection uses stable identifiers.

use chrono::{DateTime, NaiveDate, Utc};
use cp_gradebook::GradebookReportingSource;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::{Validate, ValidationError};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AcademicReportBatchStatus {
    Draft,
    Reviewed,
    Published,
}

impl AcademicReportBatchStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Reviewed => "reviewed",
            Self::Published => "published",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProgressionOutcome {
    NotApplicable,
    Pending,
    Promoted,
    Retained,
    Completed,
}

impl ProgressionOutcome {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NotApplicable => "not_applicable",
            Self::Pending => "pending",
            Self::Promoted => "promoted",
            Self::Retained => "retained",
            Self::Completed => "completed",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct GradingBandInput {
    #[validate(length(min = 1, max = 30))]
    pub code: String,
    #[validate(length(min = 1, max = 100))]
    pub label: String,
    #[validate(range(min = 0, max = 10000))]
    pub minimum_basis_points: i16,
    pub is_pass: bool,
}

#[derive(Debug, Deserialize, Validate)]
#[validate(schema(function = "validate_scheme_bands"))]
pub struct CreateGradingSchemeRequest {
    #[validate(length(min = 1, max = 150))]
    pub name: String,
    #[validate(length(min = 1, max = 1000))]
    pub description: Option<String>,
    pub is_default: bool,
    #[validate(length(min = 2, max = 20), nested)]
    pub bands: Vec<GradingBandInput>,
}

#[derive(Debug, Deserialize, Validate)]
#[validate(schema(function = "validate_update_scheme_bands"))]
pub struct UpdateGradingSchemeRequest {
    #[validate(range(min = 1))]
    pub expected_version: i32,
    #[validate(length(min = 1, max = 150))]
    pub name: String,
    #[validate(length(min = 1, max = 1000))]
    pub description: Option<String>,
    pub is_default: bool,
    #[validate(length(min = 2, max = 20), nested)]
    pub bands: Vec<GradingBandInput>,
}

fn validate_scheme_bands(value: &CreateGradingSchemeRequest) -> Result<(), ValidationError> {
    validate_bands(&value.bands)
}

fn validate_update_scheme_bands(value: &UpdateGradingSchemeRequest) -> Result<(), ValidationError> {
    validate_bands(&value.bands)
}

fn validate_bands(bands: &[GradingBandInput]) -> Result<(), ValidationError> {
    let mut minimums = bands
        .iter()
        .map(|band| band.minimum_basis_points)
        .collect::<Vec<_>>();
    minimums.sort_unstable();
    minimums.dedup();
    if minimums.len() != bands.len() || minimums.first().copied() != Some(0) {
        return Err(ValidationError::new("grading_band_boundaries"));
    }
    let mut codes = bands
        .iter()
        .map(|band| band.code.trim().to_lowercase())
        .collect::<Vec<_>>();
    codes.sort_unstable();
    codes.dedup();
    if codes.len() != bands.len() {
        return Err(ValidationError::new("grading_band_codes"));
    }
    Ok(())
}

#[derive(Debug, Deserialize, Validate)]
pub struct GenerateAcademicReportRequest {
    pub assessment_cycle_id: Uuid,
    pub class_group_id: Uuid,
    pub grading_scheme_id: Uuid,
    #[validate(length(min = 1, max = 200))]
    pub idempotency_key: String,
}

#[derive(Debug, Deserialize, Validate)]
pub struct TransitionAcademicReportRequest {
    #[validate(range(min = 1))]
    pub expected_version: i32,
}

#[derive(Debug, Deserialize, Validate)]
pub struct ReopenAcademicReportRequest {
    #[validate(range(min = 1))]
    pub expected_version: i32,
    #[validate(length(min = 1, max = 1000))]
    pub reason: String,
}

#[derive(Debug, Deserialize, Validate)]
pub struct DeleteAcademicReportQuery {
    #[validate(range(min = 1))]
    pub expected_version: i32,
}

#[derive(Debug, Deserialize, Validate)]
pub struct DeleteGradingSchemeQuery {
    #[validate(range(min = 1))]
    pub expected_version: i32,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateReportCardTeacherCommentRequest {
    #[validate(range(min = 1))]
    pub expected_version: i32,
    #[validate(length(min = 1, max = 2000))]
    pub teacher_comment: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
#[validate(schema(function = "validate_progression_review"))]
pub struct UpdateReportCardReviewRequest {
    #[validate(range(min = 1))]
    pub expected_version: i32,
    #[validate(length(min = 1, max = 2000))]
    pub reviewer_comment: Option<String>,
    pub progression_outcome: ProgressionOutcome,
    pub target_grade_level_id: Option<Uuid>,
}

fn validate_progression_review(
    value: &UpdateReportCardReviewRequest,
) -> Result<(), ValidationError> {
    if (value.progression_outcome == ProgressionOutcome::Promoted)
        != value.target_grade_level_id.is_some()
    {
        return Err(ValidationError::new("progression_target"));
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
pub struct AcademicReportBatchListQuery {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
    pub status: Option<AcademicReportBatchStatus>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AcademicGradeLevelReference {
    pub id: Uuid,
    pub code: String,
    pub name: String,
    pub sort_order: i32,
}

#[derive(Debug, Clone, Serialize)]
pub struct GradingBandResponse {
    pub id: Uuid,
    pub code: String,
    pub label: String,
    pub minimum_basis_points: i16,
    pub is_pass: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct GradingSchemeResponse {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub is_default: bool,
    pub status: String,
    pub version: i32,
    pub bands: Vec<GradingBandResponse>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct AcademicReportReferenceData {
    pub sources: Vec<GradebookReportingSource>,
    pub grading_schemes: Vec<GradingSchemeResponse>,
    pub grade_levels: Vec<AcademicGradeLevelReference>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AcademicReportBatchSummary {
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
}

#[derive(Debug, Clone, Serialize)]
pub struct AcademicSubjectResultResponse {
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

#[derive(Debug, Clone, Serialize)]
pub struct AcademicAttendanceResponse {
    pub present_count: i32,
    pub absent_count: i32,
    pub late_count: i32,
    pub excused_count: i32,
    pub attendance_percentage_basis_points: Option<i16>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AcademicReportCardResponse {
    pub id: Uuid,
    pub enrolment_id: Uuid,
    pub learner_id: Uuid,
    pub learner_number: String,
    pub learner_name: String,
    pub overall_percentage_basis_points: Option<i16>,
    pub overall_grade_code: Option<String>,
    pub overall_grade_label: Option<String>,
    pub teacher_comment: Option<String>,
    pub reviewer_comment: Option<String>,
    pub progression_outcome: String,
    pub target_grade_level_id: Option<Uuid>,
    pub target_grade_level_name: Option<String>,
    pub version: i32,
    pub subjects: Vec<AcademicSubjectResultResponse>,
    pub attendance: AcademicAttendanceResponse,
}

#[derive(Debug, Clone, Serialize)]
pub struct AcademicReportBatchResponse {
    #[serde(flatten)]
    pub summary: AcademicReportBatchSummary,
    pub cards: Vec<AcademicReportCardResponse>,
    pub reopened_at: Option<DateTime<Utc>>,
    pub reopen_reason: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct PaginatedAcademicReportBatchesResponse {
    pub report_batches: Vec<AcademicReportBatchSummary>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AcademicTranscriptEntry {
    pub report_batch_id: Uuid,
    pub assessment_cycle_name: String,
    pub academic_term_name: String,
    pub academic_year_name: String,
    pub class_group_name: String,
    pub published_at: DateTime<Utc>,
    pub overall_percentage_basis_points: Option<i16>,
    pub overall_grade_code: Option<String>,
    pub overall_grade_label: Option<String>,
    pub progression_outcome: String,
    pub subjects: Vec<AcademicSubjectResultResponse>,
}

#[derive(Debug, Serialize)]
pub struct AcademicTranscriptResponse {
    pub learner_id: Uuid,
    pub learner_number: String,
    pub learner_name: String,
    pub entries: Vec<AcademicTranscriptEntry>,
}

/// Calculation inputs after all transport validation has succeeded.
#[derive(Debug, Clone)]
pub(crate) struct ReportingSourceBoundary {
    pub assessment_cycle_id: Uuid,
    pub class_group_id: Uuid,
    pub academic_year_id: Uuid,
    pub term_starts_on: NaiveDate,
    pub term_ends_on: NaiveDate,
}

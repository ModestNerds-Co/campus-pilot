//! Gradebook transport contracts and parsed workflow values.
//!
//! Mark values use hundredths of one mark so decimal capture remains exact and
//! every write carries the optimistic version of its parent mark sheet.

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use validator::{Validate, ValidationError};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GradebookMarkStatus {
    Unmarked,
    Scored,
    Absent,
    Exempt,
}

impl GradebookMarkStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unmarked => "unmarked",
            Self::Scored => "scored",
            Self::Absent => "absent",
            Self::Exempt => "exempt",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GradebookSheetStatus {
    Draft,
    Submitted,
    Published,
}

impl GradebookSheetStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Submitted => "submitted",
            Self::Published => "published",
        }
    }
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreateMarkSheetRequest {
    pub assessment_component_id: Uuid,
    pub roster_on: NaiveDate,
    #[validate(length(min = 1, max = 200))]
    pub idempotency_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
#[validate(schema(function = "validate_mark_input"))]
pub struct GradebookMarkInput {
    pub learner_id: Uuid,
    pub mark_status: GradebookMarkStatus,
    #[validate(range(min = 0))]
    pub marks_awarded_hundredths: Option<i64>,
    #[validate(length(min = 1, max = 1000))]
    pub note: Option<String>,
}

fn validate_mark_input(value: &GradebookMarkInput) -> Result<(), ValidationError> {
    if (value.mark_status == GradebookMarkStatus::Scored)
        != value.marks_awarded_hundredths.is_some()
    {
        return Err(ValidationError::new("scored_mark_shape"));
    }
    if value.mark_status == GradebookMarkStatus::Unmarked && value.note.is_some() {
        return Err(ValidationError::new("unmarked_has_note"));
    }
    Ok(())
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateGradebookMarksRequest {
    #[validate(range(min = 1))]
    pub expected_version: i32,
    #[validate(length(min = 1, max = 500), nested)]
    pub marks: Vec<GradebookMarkInput>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct TransitionMarkSheetRequest {
    #[validate(range(min = 1))]
    pub expected_version: i32,
}

#[derive(Debug, Deserialize, Validate)]
pub struct ReopenMarkSheetRequest {
    #[validate(range(min = 1))]
    pub expected_version: i32,
    #[validate(length(min = 1, max = 1000))]
    pub reason: String,
}

#[derive(Debug, Deserialize, Validate)]
pub struct DeleteMarkSheetQuery {
    #[validate(range(min = 1))]
    pub expected_version: i32,
}

#[derive(Debug, Deserialize)]
pub struct GradebookSheetListQuery {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
    pub status: Option<GradebookSheetStatus>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GradebookComponentReference {
    pub assessment_component_id: Uuid,
    pub assessment_component_code: String,
    pub assessment_component_name: String,
    pub assessment_kind: String,
    pub maximum_marks: i32,
    pub weight_basis_points: i16,
    pub occurs_on: Option<NaiveDate>,
    pub assessment_cycle_id: Uuid,
    pub assessment_cycle_name: String,
    pub assessment_cycle_status: String,
    pub academic_term_id: Uuid,
    pub academic_term_name: String,
    pub academic_term_starts_on: NaiveDate,
    pub academic_term_ends_on: NaiveDate,
    pub academic_year_id: Uuid,
    pub academic_year_name: String,
    pub teaching_assignment_id: Uuid,
    pub class_group_id: Uuid,
    pub class_group_name: String,
    pub subject_id: Uuid,
    pub subject_name: String,
    pub teacher_profile_id: Uuid,
    pub teacher_name: String,
    pub mark_sheet_id: Option<Uuid>,
    pub mark_sheet_status: Option<String>,
    pub mark_sheet_version: Option<i32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GradebookReferenceData {
    pub components: Vec<GradebookComponentReference>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GradebookSheetSummary {
    pub id: Uuid,
    pub assessment_component_id: Uuid,
    pub assessment_component_code: String,
    pub assessment_component_name: String,
    pub assessment_kind: String,
    pub maximum_marks: i32,
    pub weight_basis_points: i16,
    pub assessment_cycle_id: Uuid,
    pub assessment_cycle_name: String,
    pub academic_term_id: Uuid,
    pub academic_term_name: String,
    pub academic_year_id: Uuid,
    pub academic_year_name: String,
    pub teaching_assignment_id: Uuid,
    pub class_group_id: Uuid,
    pub class_group_name: String,
    pub subject_id: Uuid,
    pub subject_name: String,
    pub teacher_profile_id: Uuid,
    pub teacher_name: String,
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

#[derive(Debug, Clone, Serialize)]
pub struct GradebookMarkResponse {
    pub id: Uuid,
    pub enrolment_id: Uuid,
    pub learner_id: Uuid,
    pub learner_number: String,
    pub learner_name: String,
    pub mark_status: String,
    pub marks_awarded_hundredths: Option<i64>,
    pub percentage_basis_points: Option<i64>,
    pub weighted_score_basis_points: Option<i64>,
    pub note: Option<String>,
    pub version: i32,
    pub marked_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GradebookSheetResponse {
    #[serde(flatten)]
    pub summary: GradebookSheetSummary,
    pub marks: Vec<GradebookMarkResponse>,
    pub reopened_at: Option<DateTime<Utc>>,
    pub reopen_reason: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct PaginatedGradebookSheetsResponse {
    pub mark_sheets: Vec<GradebookSheetSummary>,
}

/// One closed-cycle class that can feed an academic report batch.
///
/// Teacher account identifiers are request-authority evidence for assigned
/// scope consumers and never form part of browser or Agent responses.
#[derive(Debug, Clone, Serialize, FromRow)]
pub struct GradebookReportingSource {
    pub assessment_cycle_id: Uuid,
    pub assessment_cycle_name: String,
    pub academic_term_id: Uuid,
    pub academic_term_name: String,
    pub academic_term_starts_on: NaiveDate,
    pub academic_term_ends_on: NaiveDate,
    pub academic_year_id: Uuid,
    pub academic_year_name: String,
    pub class_group_id: Uuid,
    pub class_group_name: String,
    pub component_count: i64,
    pub published_sheet_count: i64,
    #[serde(skip_serializing)]
    pub teacher_account_ids: Vec<Uuid>,
}

/// Exact published Gradebook evidence consumed by academic reporting.
///
/// Reporting calculates snapshots from these stable identifiers and integer
/// values; it does not issue private SQL against Gradebook tables.
#[derive(Debug, Clone, FromRow)]
pub struct PublishedAssessmentMark {
    pub mark_sheet_id: Uuid,
    pub mark_sheet_version: i32,
    pub assessment_component_id: Uuid,
    pub teaching_assignment_id: Uuid,
    pub subject_id: Uuid,
    pub subject_name: String,
    pub class_group_id: Uuid,
    pub enrolment_id: Uuid,
    pub learner_id: Uuid,
    pub mark_status: String,
    pub marks_awarded_hundredths: Option<i64>,
    pub maximum_marks: i32,
    pub weight_basis_points: i16,
}

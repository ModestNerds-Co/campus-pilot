//! Closed transport and record-scope contracts for E-learning.

use chrono::{DateTime, NaiveDate, Utc};
use cp_document_registry::EvidenceFileReference;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

/// Visibility proof selected from current role record-scope grants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LearningAccessScope {
    Campus,
    AssignedTo(Uuid),
    SelfFor(Uuid),
    SelfAndAssigned(Uuid),
}

/// Identifies the reviewed resource-creation path for audit evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LearningResourceCreation {
    Link,
    Upload,
}

impl LearningResourceCreation {
    #[must_use]
    pub const fn operation_key(self) -> &'static str {
        match self {
            Self::Link => "learning.resources.create",
            Self::Upload => "learning.resources.upload",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LearningSpaceStatus {
    Draft,
    Published,
    Archived,
}

impl LearningSpaceStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Published => "published",
            Self::Archived => "archived",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LearningUnitStatus {
    Draft,
    Published,
    Withdrawn,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LearningResourceStatus {
    Draft,
    Published,
    Withdrawn,
}

/// Published-assignment lifecycle exposed through Learning APIs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LearningAssignmentStatus {
    Draft,
    Published,
    Closed,
}

impl LearningAssignmentStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Published => "published",
            Self::Closed => "closed",
        }
    }
}

/// Current learner-work state; immutable attempts live separately.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LearningSubmissionStatus {
    Draft,
    Submitted,
    RevisionRequested,
    Graded,
}

impl LearningSubmissionStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Submitted => "submitted",
            Self::RevisionRequested => "revision_requested",
            Self::Graded => "graded",
        }
    }
}

/// Final teacher decision released for one immutable attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LearningReviewOutcome {
    Graded,
    RevisionRequested,
}

impl LearningReviewOutcome {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Graded => "graded",
            Self::RevisionRequested => "revision_requested",
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct LearningSpaceListQuery {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
    pub search: Option<String>,
    pub status: Option<LearningSpaceStatus>,
}

#[derive(Debug, Deserialize)]
pub struct LearningResourceFileQuery {
    pub search: Option<String>,
    pub limit: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct LearningAssignmentListQuery {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
    pub status: Option<LearningAssignmentStatus>,
}

#[derive(Debug, Deserialize)]
pub struct LearningSubmissionListQuery {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
    pub status: Option<LearningSubmissionStatus>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LearningSettingsResponse {
    pub document_series_id: Option<Uuid>,
    pub document_series_name: Option<String>,
    pub version: i32,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateLearningSettingsRequest {
    pub document_series_id: Option<Uuid>,
    #[validate(range(min = 1))]
    pub expected_version: i32,
}

#[derive(Debug, Clone, Serialize)]
pub struct LearningTermReference {
    pub id: Uuid,
    pub academic_year_id: Uuid,
    pub academic_year_name: String,
    pub code: String,
    pub name: String,
    pub starts_on: NaiveDate,
    pub ends_on: NaiveDate,
}

#[derive(Debug, Clone, Serialize)]
pub struct LearningAssignmentReference {
    pub id: Uuid,
    pub academic_year_id: Uuid,
    pub academic_year_name: String,
    pub class_group_id: Uuid,
    pub class_group_name: String,
    pub subject_id: Uuid,
    pub subject_name: String,
    pub teacher_name: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct LearningReferenceData {
    pub active_term: Option<LearningTermReference>,
    pub assignments: Vec<LearningAssignmentReference>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreateLearningSpaceRequest {
    pub teaching_assignment_id: Uuid,
    pub academic_term_id: Uuid,
    #[validate(length(min = 1, max = 200))]
    pub title: String,
    #[validate(length(max = 4000))]
    pub summary: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateLearningSpaceRequest {
    #[validate(length(min = 1, max = 200))]
    pub title: String,
    #[validate(length(max = 4000))]
    pub summary: Option<String>,
    #[validate(range(min = 1))]
    pub expected_version: i32,
}

#[derive(Debug, Deserialize, Validate)]
pub struct VersionedLearningRequest {
    #[validate(range(min = 1))]
    pub expected_version: i32,
}

#[derive(Debug, Deserialize, Validate)]
pub struct ReasonedLearningTransitionRequest {
    #[validate(range(min = 1))]
    pub expected_version: i32,
    #[validate(length(min = 1, max = 2000))]
    pub reason: String,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreateLearningUnitRequest {
    #[validate(range(min = 1))]
    pub position: i32,
    #[validate(length(min = 1, max = 200))]
    pub title: String,
    #[validate(length(max = 4000))]
    pub summary: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateLearningUnitRequest {
    #[validate(range(min = 1))]
    pub position: i32,
    #[validate(length(min = 1, max = 200))]
    pub title: String,
    #[validate(length(max = 4000))]
    pub summary: Option<String>,
    #[validate(range(min = 1))]
    pub expected_version: i32,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreateLearningResourceRequest {
    pub document_file_id: Uuid,
    #[validate(length(min = 1, max = 240))]
    pub display_title: String,
    #[validate(range(min = 1))]
    pub position: i32,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateLearningResourceRequest {
    #[validate(length(min = 1, max = 240))]
    pub display_title: String,
    #[validate(range(min = 1))]
    pub position: i32,
    #[validate(range(min = 1))]
    pub expected_version: i32,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreateLearningAssignmentRequest {
    #[validate(range(min = 1))]
    pub position: i32,
    #[validate(length(min = 1, max = 200))]
    pub title: String,
    #[validate(length(min = 1, max = 20000))]
    pub instructions: String,
    pub due_at: DateTime<Utc>,
    #[validate(range(min = 1))]
    pub max_score_hundredths: i32,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateLearningAssignmentRequest {
    #[validate(range(min = 1))]
    pub position: i32,
    #[validate(length(min = 1, max = 200))]
    pub title: String,
    #[validate(length(min = 1, max = 20000))]
    pub instructions: String,
    pub due_at: DateTime<Utc>,
    #[validate(range(min = 1))]
    pub max_score_hundredths: i32,
    #[validate(range(min = 1))]
    pub expected_version: i32,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreateLearningRubricCriterionRequest {
    #[validate(range(min = 1))]
    pub position: i32,
    #[validate(length(min = 1, max = 200))]
    pub title: String,
    #[validate(length(max = 4000))]
    pub description: Option<String>,
    #[validate(range(min = 1))]
    pub max_score_hundredths: i32,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateLearningRubricCriterionRequest {
    #[validate(range(min = 1))]
    pub position: i32,
    #[validate(length(min = 1, max = 200))]
    pub title: String,
    #[validate(length(max = 4000))]
    pub description: Option<String>,
    #[validate(range(min = 1))]
    pub max_score_hundredths: i32,
    #[validate(range(min = 1))]
    pub expected_version: i32,
}

#[derive(Debug, Deserialize, Validate)]
pub struct DeleteLearningRubricCriterionRequest {
    #[validate(range(min = 1))]
    pub expected_version: i32,
}

/// Saves the current learner draft. `None` proves first creation; subsequent
/// writes must carry the current optimistic version.
#[derive(Debug, Deserialize, Validate)]
pub struct SaveLearningSubmissionRequest {
    #[validate(length(max = 20000))]
    pub body: String,
    #[validate(range(min = 1))]
    pub expected_version: Option<i32>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct SubmitLearningSubmissionRequest {
    #[validate(range(min = 1))]
    pub expected_version: i32,
    pub idempotency_key: Uuid,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LearningRubricScoreInput {
    pub rubric_criterion_id: Uuid,
    pub earned_score_hundredths: i32,
    pub feedback: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateLearningFeedbackRequest {
    pub submission_version_id: Uuid,
    #[validate(length(max = 10000))]
    pub overall_feedback: Option<String>,
    pub scores: Vec<LearningRubricScoreInput>,
    #[validate(range(min = 1))]
    pub expected_review_version: Option<i32>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct ReleaseLearningFeedbackRequest {
    pub outcome: LearningReviewOutcome,
    #[validate(range(min = 1))]
    pub expected_review_version: i32,
    pub idempotency_key: Uuid,
}

#[derive(Debug, Clone, Serialize)]
pub struct LearningRubricCriterionResponse {
    pub id: Uuid,
    pub learning_assignment_id: Uuid,
    pub position: i32,
    pub title: String,
    pub description: Option<String>,
    pub max_score_hundredths: i32,
    pub version: i32,
}

#[derive(Debug, Clone, Serialize)]
pub struct LearningAssignmentResponse {
    pub id: Uuid,
    pub learning_unit_id: Uuid,
    pub learning_space_id: Uuid,
    pub position: i32,
    pub title: String,
    pub instructions: String,
    pub due_at: DateTime<Utc>,
    pub max_score_hundredths: i32,
    pub status: LearningAssignmentStatus,
    pub version: i32,
    pub recipient_count: i64,
    pub submission_count: i64,
    pub published_at: Option<DateTime<Utc>>,
    pub closed_at: Option<DateTime<Utc>>,
    pub close_reason: Option<String>,
    pub rubric: Vec<LearningRubricCriterionResponse>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct LearningAssignmentsPage {
    pub assignments: Vec<LearningAssignmentResponse>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LearningSubmissionVersionResponse {
    pub id: Uuid,
    pub revision_number: i32,
    pub body: String,
    pub late: bool,
    pub submitted_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LearningReviewScoreResponse {
    pub rubric_criterion_id: Uuid,
    pub earned_score_hundredths: i32,
    pub feedback: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LearningFeedbackResponse {
    pub id: Uuid,
    pub submission_version_id: Uuid,
    pub status: String,
    pub outcome: Option<LearningReviewOutcome>,
    pub overall_feedback: Option<String>,
    pub total_score_hundredths: Option<i32>,
    pub version: i32,
    pub scores: Vec<LearningReviewScoreResponse>,
    pub released_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LearningSubmissionResponse {
    pub id: Uuid,
    pub learning_assignment_id: Uuid,
    pub assignment_recipient_id: Uuid,
    pub learner_id: Uuid,
    pub enrolment_id: Uuid,
    pub learner_name: String,
    pub learner_number: String,
    pub draft_body: Option<String>,
    pub status: LearningSubmissionStatus,
    pub version: i32,
    pub current_submission_version_id: Option<Uuid>,
    pub versions: Vec<LearningSubmissionVersionResponse>,
    pub feedback: Option<LearningFeedbackResponse>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct LearningSubmissionsPage {
    pub submissions: Vec<LearningSubmissionResponse>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LearningProgressEntry {
    pub learner_id: Uuid,
    pub enrolment_id: Uuid,
    pub learner_name: String,
    pub learner_number: String,
    pub total_assignments: i64,
    pub not_started: i64,
    pub drafts: i64,
    pub awaiting_feedback: i64,
    pub revision_requested: i64,
    pub graded: i64,
    pub overdue: i64,
    pub completion_percent: i32,
    pub earned_score_hundredths: i64,
    pub possible_score_hundredths: i64,
}

#[derive(Debug, Serialize)]
pub struct LearningProgressPage {
    pub progress: Vec<LearningProgressEntry>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LearningSpaceSummary {
    pub id: Uuid,
    pub teaching_assignment_id: Uuid,
    pub academic_year_id: Uuid,
    pub academic_year_name: String,
    pub academic_term_id: Uuid,
    pub academic_term_name: String,
    pub class_group_id: Uuid,
    pub class_group_name: String,
    pub subject_name: String,
    pub teacher_name: String,
    pub title: String,
    pub summary: Option<String>,
    pub status: LearningSpaceStatus,
    pub version: i32,
    pub unit_count: i64,
    pub published_unit_count: i64,
    pub published_at: Option<DateTime<Utc>>,
    pub archived_at: Option<DateTime<Utc>>,
    pub archive_reason: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LearningResourceResponse {
    pub id: Uuid,
    pub learning_unit_id: Uuid,
    pub document_file_id: Uuid,
    pub document: Option<EvidenceFileReference>,
    pub display_title: String,
    pub sensitivity_snapshot: String,
    pub position: i32,
    pub status: LearningResourceStatus,
    pub version: i32,
    pub published_at: Option<DateTime<Utc>>,
    pub withdrawn_at: Option<DateTime<Utc>>,
    pub withdrawal_reason: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LearningUnitResponse {
    pub id: Uuid,
    pub learning_space_id: Uuid,
    pub position: i32,
    pub title: String,
    pub summary: Option<String>,
    pub status: LearningUnitStatus,
    pub version: i32,
    pub published_at: Option<DateTime<Utc>>,
    pub withdrawn_at: Option<DateTime<Utc>>,
    pub withdrawal_reason: Option<String>,
    pub resources: Vec<LearningResourceResponse>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LearningSpaceResponse {
    #[serde(flatten)]
    pub summary: LearningSpaceSummary,
    pub units: Vec<LearningUnitResponse>,
}

#[derive(Debug, Serialize)]
pub struct LearningSpacesPage {
    pub spaces: Vec<LearningSpaceSummary>,
}

#[derive(Debug, Serialize)]
pub struct LearningResourceFilesResponse {
    pub files: Vec<EvidenceFileReference>,
}

#[derive(Debug, Serialize)]
pub struct LearningDownloadResponse {
    pub url: String,
    pub expires_in_seconds: u64,
}

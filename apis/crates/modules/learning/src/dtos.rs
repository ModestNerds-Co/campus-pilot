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

/// Response material accepted by one assignment. The method is fixed when the
/// assignment is published so learners cannot lose a required input channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LearningSubmissionMethod {
    Text,
    File,
    TextOrFile,
}

impl LearningSubmissionMethod {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::File => "file",
            Self::TextOrFile => "text_or_file",
        }
    }

    #[must_use]
    pub const fn accepts_files(self) -> bool {
        matches!(self, Self::File | Self::TextOrFile)
    }
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

/// Authoring and participation lifecycle for one class-linked quiz.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LearningQuizStatus {
    Draft,
    Published,
    Closed,
}

impl LearningQuizStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Published => "published",
            Self::Closed => "closed",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LearningQuizAttemptStatus {
    InProgress,
    Submitted,
}

impl LearningQuizAttemptStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InProgress => "in_progress",
            Self::Submitted => "submitted",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LearningCompletionPolicyStatus {
    Draft,
    Published,
    Superseded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LearningCompletionRequirementType {
    Assignment,
    Quiz,
}

impl LearningCompletionRequirementType {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Assignment => "assignment",
            Self::Quiz => "quiz",
        }
    }
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

#[derive(Debug, Deserialize)]
pub struct LearningQuizListQuery {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
    pub status: Option<LearningQuizStatus>,
}

#[derive(Debug, Deserialize)]
pub struct LearningQuizAttemptListQuery {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
    pub status: Option<LearningQuizAttemptStatus>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LearningSettingsResponse {
    pub document_series_id: Option<Uuid>,
    pub document_series_name: Option<String>,
    pub learner_submission_series_id: Option<Uuid>,
    pub learner_submission_series_name: Option<String>,
    pub version: i32,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LearningUploadClassificationOption {
    pub id: Uuid,
    pub code: String,
    pub name: String,
    pub default_sensitivity: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct LearningUploadClassificationOptionsResponse {
    pub resource_series: Vec<LearningUploadClassificationOption>,
    pub learner_submission_series: Vec<LearningUploadClassificationOption>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateLearningSettingsRequest {
    pub document_series_id: Option<Uuid>,
    pub learner_submission_series_id: Option<Uuid>,
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
    pub submission_method: LearningSubmissionMethod,
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
    pub submission_method: LearningSubmissionMethod,
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

#[derive(Debug, Deserialize, Validate)]
pub struct RemoveLearningSubmissionFileRequest {
    #[validate(range(min = 1))]
    pub expected_submission_version: i32,
    #[validate(range(min = 1))]
    pub expected_attachment_version: i32,
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

#[derive(Debug, Deserialize, Validate)]
pub struct CreateLearningQuizRequest {
    #[validate(range(min = 1))]
    pub position: i32,
    #[validate(length(min = 1, max = 200))]
    pub title: String,
    #[validate(length(max = 10000))]
    pub instructions: Option<String>,
    pub opens_at: Option<DateTime<Utc>>,
    pub closes_at: Option<DateTime<Utc>>,
    #[validate(range(min = 1, max = 10))]
    pub attempt_limit: i32,
    #[validate(range(min = 0, max = 10000))]
    pub pass_score_basis_points: i32,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateLearningQuizRequest {
    #[validate(range(min = 1))]
    pub position: i32,
    #[validate(length(min = 1, max = 200))]
    pub title: String,
    #[validate(length(max = 10000))]
    pub instructions: Option<String>,
    pub opens_at: Option<DateTime<Utc>>,
    pub closes_at: Option<DateTime<Utc>>,
    #[validate(range(min = 1, max = 10))]
    pub attempt_limit: i32,
    #[validate(range(min = 0, max = 10000))]
    pub pass_score_basis_points: i32,
    #[validate(range(min = 1))]
    pub expected_version: i32,
}

#[derive(Debug, Clone, Deserialize, Serialize, Validate)]
pub struct LearningQuizChoiceInput {
    #[validate(length(min = 1, max = 1000))]
    pub label: String,
    pub is_correct: bool,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreateLearningQuizQuestionRequest {
    #[validate(range(min = 1))]
    pub position: i32,
    #[validate(length(min = 1, max = 4000))]
    pub prompt: String,
    #[validate(range(min = 1, max = 1000))]
    pub points: i32,
    #[validate(length(min = 2, max = 8))]
    pub choices: Vec<LearningQuizChoiceInput>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateLearningQuizQuestionRequest {
    #[validate(range(min = 1))]
    pub position: i32,
    #[validate(length(min = 1, max = 4000))]
    pub prompt: String,
    #[validate(range(min = 1, max = 1000))]
    pub points: i32,
    #[validate(length(min = 2, max = 8))]
    pub choices: Vec<LearningQuizChoiceInput>,
    #[validate(range(min = 1))]
    pub expected_version: i32,
}

#[derive(Debug, Deserialize, Validate)]
pub struct DeleteLearningQuizQuestionRequest {
    #[validate(range(min = 1))]
    pub expected_version: i32,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LearningQuizAnswerInput {
    pub question_id: Uuid,
    pub selected_choice_id: Uuid,
}

#[derive(Debug, Deserialize, Validate)]
pub struct SaveLearningQuizAttemptRequest {
    pub answers: Vec<LearningQuizAnswerInput>,
    #[validate(range(min = 1))]
    pub expected_version: i32,
}

#[derive(Debug, Deserialize, Validate)]
pub struct SubmitLearningQuizAttemptRequest {
    #[validate(range(min = 1))]
    pub expected_version: i32,
    pub idempotency_key: Uuid,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LearningCompletionRequirementInput {
    pub requirement_type: LearningCompletionRequirementType,
    pub source_id: Uuid,
    pub minimum_score_basis_points: i32,
}

#[derive(Debug, Deserialize, Validate)]
pub struct SaveLearningCompletionPolicyRequest {
    #[validate(length(min = 1, max = 100))]
    pub requirements: Vec<LearningCompletionRequirementInput>,
    #[validate(range(min = 1))]
    pub expected_version: Option<i32>,
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
    pub submission_method: LearningSubmissionMethod,
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
    pub body: Option<String>,
    pub files: Vec<LearningSubmissionFileResponse>,
    pub late: bool,
    pub submitted_at: DateTime<Utc>,
}

/// Safe attachment metadata. Bytes, object keys, hashes, and signed URLs are
/// deliberately absent from every HTTP and Agent projection.
#[derive(Debug, Clone, Serialize)]
pub struct LearningSubmissionFileResponse {
    pub id: Uuid,
    pub document_file_id: Uuid,
    pub document_reference: String,
    pub original_file_name: String,
    pub media_type: String,
    pub byte_size: i64,
    pub position: i32,
    pub version: Option<i32>,
    pub attached_at: DateTime<Utc>,
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
    pub draft_files: Vec<LearningSubmissionFileResponse>,
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
pub struct LearningQuizChoiceResponse {
    pub id: Uuid,
    pub position: i32,
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_correct: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LearningQuizQuestionResponse {
    pub id: Uuid,
    pub position: i32,
    pub prompt: String,
    pub points: i32,
    pub version: i32,
    pub choices: Vec<LearningQuizChoiceResponse>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LearningQuizResponse {
    pub id: Uuid,
    pub learning_unit_id: Uuid,
    pub learning_space_id: Uuid,
    pub position: i32,
    pub title: String,
    pub instructions: Option<String>,
    pub opens_at: Option<DateTime<Utc>>,
    pub closes_at: Option<DateTime<Utc>>,
    pub attempt_limit: i32,
    pub pass_score_basis_points: i32,
    pub status: LearningQuizStatus,
    pub version: i32,
    pub recipient_count: i64,
    pub submitted_attempt_count: i64,
    pub my_attempt_count: i64,
    pub questions: Vec<LearningQuizQuestionResponse>,
    pub published_at: Option<DateTime<Utc>>,
    pub closed_at: Option<DateTime<Utc>>,
    pub close_reason: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct LearningQuizzesPage {
    pub quizzes: Vec<LearningQuizResponse>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LearningQuizAttemptAnswerResponse {
    pub question_id: Uuid,
    pub selected_choice_id: Uuid,
}

#[derive(Debug, Clone, Serialize)]
pub struct LearningQuizAttemptResponse {
    pub id: Uuid,
    pub learning_quiz_id: Uuid,
    pub learner_id: Uuid,
    pub enrolment_id: Uuid,
    pub learner_name: String,
    pub learner_number: String,
    pub attempt_number: i32,
    pub status: LearningQuizAttemptStatus,
    pub version: i32,
    pub answers: Vec<LearningQuizAttemptAnswerResponse>,
    pub started_at: DateTime<Utc>,
    pub submitted_at: Option<DateTime<Utc>>,
    pub total_points: Option<i32>,
    pub earned_points: Option<i32>,
    pub score_basis_points: Option<i32>,
    pub passed: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct LearningQuizAttemptsPage {
    pub attempts: Vec<LearningQuizAttemptResponse>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LearningCompletionRequirementResponse {
    pub id: Uuid,
    pub position: i32,
    pub requirement_type: LearningCompletionRequirementType,
    pub source_id: Uuid,
    pub source_title: String,
    pub minimum_score_basis_points: i32,
}

#[derive(Debug, Clone, Serialize)]
pub struct LearningCompletionPolicyResponse {
    pub id: Uuid,
    pub learning_space_id: Uuid,
    pub status: LearningCompletionPolicyStatus,
    pub version: i32,
    pub requirements: Vec<LearningCompletionRequirementResponse>,
    pub recipient_count: i64,
    pub published_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LearningCompletionEntry {
    pub learner_id: Uuid,
    pub enrolment_id: Uuid,
    pub learner_name: String,
    pub learner_number: String,
    pub required_count: i64,
    pub completed_count: i64,
    pub completion_percent: i32,
    pub complete: bool,
}

#[derive(Debug, Serialize)]
pub struct LearningCompletionPage {
    pub policy: Option<LearningCompletionPolicyResponse>,
    pub progress: Vec<LearningCompletionEntry>,
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

#[cfg(test)]
mod quiz_projection_tests {
    use serde_json::to_value;
    use uuid::Uuid;

    use super::LearningQuizChoiceResponse;

    #[test]
    fn learner_choice_projection_omits_the_answer_key_field() {
        let learner = LearningQuizChoiceResponse {
            id: Uuid::nil(),
            position: 1,
            label: "A learner-visible choice".to_string(),
            is_correct: None,
        };
        let teacher = LearningQuizChoiceResponse {
            is_correct: Some(true),
            ..learner.clone()
        };

        let learner_json = to_value(learner).expect("learner choice should serialize");
        let teacher_json = to_value(teacher).expect("teacher choice should serialize");
        assert!(learner_json.get("is_correct").is_none());
        assert_eq!(
            teacher_json.get("is_correct"),
            Some(&serde_json::json!(true))
        );
    }
}

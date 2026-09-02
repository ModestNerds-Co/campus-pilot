//! Private persistence rows for Learning-owned state.

use chrono::{DateTime, Utc};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, FromRow)]
pub(crate) struct LearningSettingsRow {
    pub document_series_id: Option<Uuid>,
    pub learner_submission_series_id: Option<Uuid>,
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

#[derive(Debug, Clone, FromRow)]
pub(crate) struct LearningAssignmentRow {
    pub id: Uuid,
    pub learning_unit_id: Uuid,
    pub learning_space_id: Uuid,
    pub position: i32,
    pub title: String,
    pub instructions: String,
    pub due_at: DateTime<Utc>,
    pub max_score_hundredths: i32,
    pub submission_method: String,
    pub status: String,
    pub version: i32,
    pub published_at: Option<DateTime<Utc>>,
    pub closed_at: Option<DateTime<Utc>>,
    pub close_reason: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub recipient_count: i64,
    pub submission_count: i64,
}

#[derive(Debug, Clone, FromRow)]
pub(crate) struct LearningRubricCriterionRow {
    pub id: Uuid,
    pub learning_assignment_id: Uuid,
    pub position: i32,
    pub title: String,
    pub description: Option<String>,
    pub max_score_hundredths: i32,
    pub version: i32,
}

#[derive(Debug, Clone, FromRow)]
pub(crate) struct LearningSubmissionRow {
    pub id: Uuid,
    pub learning_assignment_id: Uuid,
    pub assignment_recipient_id: Uuid,
    pub learner_id: Uuid,
    pub enrolment_id: Uuid,
    pub draft_body: Option<String>,
    pub status: String,
    pub version: i32,
    pub current_submission_version_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
pub(crate) struct LearningSubmissionVersionRow {
    pub id: Uuid,
    pub revision_number: i32,
    pub body_snapshot: Option<String>,
    pub late_snapshot: bool,
    pub submitted_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
pub(crate) struct LearningSubmissionFileRow {
    pub id: Uuid,
    pub document_file_id: Uuid,
    pub document_reference_snapshot: String,
    pub original_file_name_snapshot: String,
    pub media_type_snapshot: String,
    pub byte_size_snapshot: i64,
    pub position: i32,
    pub version: Option<i32>,
    pub attached_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
pub(crate) struct LearningFeedbackRow {
    pub id: Uuid,
    pub submission_version_id: Uuid,
    pub status: String,
    pub outcome: Option<String>,
    pub overall_feedback: Option<String>,
    pub total_score_hundredths: Option<i32>,
    pub version: i32,
    pub released_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, FromRow)]
pub(crate) struct LearningReviewScoreRow {
    pub rubric_criterion_id: Uuid,
    pub earned_score_hundredths: i32,
    pub feedback: Option<String>,
}

#[derive(Debug, Clone, FromRow)]
pub(crate) struct LearningProgressRow {
    pub learner_id: Uuid,
    pub enrolment_id: Uuid,
    pub total_assignments: i64,
    pub not_started: i64,
    pub drafts: i64,
    pub awaiting_feedback: i64,
    pub revision_requested: i64,
    pub graded: i64,
    pub overdue: i64,
    pub earned_score_hundredths: i64,
    pub possible_score_hundredths: i64,
}

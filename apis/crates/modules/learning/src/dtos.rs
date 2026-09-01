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

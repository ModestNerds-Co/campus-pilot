//! Closed transport contracts for Document Registry.

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

#[derive(Debug, Deserialize)]
pub struct RegistryListQuery {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
    pub search: Option<String>,
    pub status: Option<String>,
    pub series_id: Option<Uuid>,
    pub sensitivity: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct NumberingPolicyResponse {
    pub prefix: String,
    pub padding: i16,
    pub next_sequence: i64,
    pub next_reference: String,
    pub version: i32,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateNumberingPolicyRequest {
    #[validate(length(min = 1, max = 20))]
    pub prefix: String,
    #[validate(range(min = 3, max = 12))]
    pub padding: i16,
    #[validate(range(min = 1))]
    pub next_sequence: i64,
    #[validate(range(min = 1))]
    pub version: i32,
}

#[derive(Debug, Clone, Serialize)]
pub struct SeriesResponse {
    pub id: Uuid,
    pub code: String,
    pub name: String,
    pub description: Option<String>,
    pub retention_trigger: String,
    pub retention_period_months: Option<i16>,
    pub final_disposition: String,
    pub default_sensitivity: String,
    pub status: String,
    pub version: i32,
    pub file_count: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreateSeriesRequest {
    #[validate(length(min = 1, max = 30))]
    pub code: String,
    #[validate(length(min = 1, max = 160))]
    pub name: String,
    #[validate(length(max = 2000))]
    pub description: Option<String>,
    pub retention_trigger: String,
    pub retention_period_months: Option<i16>,
    pub final_disposition: String,
    pub default_sensitivity: String,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateSeriesRequest {
    #[validate(length(min = 1, max = 30))]
    pub code: String,
    #[validate(length(min = 1, max = 160))]
    pub name: String,
    #[validate(length(max = 2000))]
    pub description: Option<String>,
    pub retention_trigger: String,
    pub retention_period_months: Option<i16>,
    pub final_disposition: String,
    pub default_sensitivity: String,
    pub status: String,
    #[validate(range(min = 1))]
    pub version: i32,
}

#[derive(Debug, Clone)]
pub struct NewRegistryFile {
    pub series_id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub document_date: Option<NaiveDate>,
    pub sensitivity: Option<String>,
    pub original_file_name: String,
    pub media_type: String,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FileResponse {
    pub id: Uuid,
    pub reference: String,
    pub series_id: Uuid,
    pub series_code: String,
    pub series_name: String,
    pub retention_trigger: String,
    pub retention_period_months: Option<i16>,
    pub final_disposition: String,
    pub sensitivity: String,
    pub title: String,
    pub description: Option<String>,
    pub document_date: Option<NaiveDate>,
    pub filed_on: NaiveDate,
    pub retain_until: Option<NaiveDate>,
    pub status: String,
    pub original_file_name: String,
    pub media_type: String,
    pub byte_size: i64,
    pub sha256_hex: String,
    pub scanned_at: DateTime<Utc>,
    pub version: i32,
    pub closed_at: Option<DateTime<Utc>>,
    pub close_reason: Option<String>,
    pub destroyed_at: Option<DateTime<Utc>>,
    pub destruction_reason: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateFileRequest {
    #[validate(length(min = 1, max = 240))]
    pub title: String,
    #[validate(length(max = 4000))]
    pub description: Option<String>,
    pub document_date: Option<NaiveDate>,
    pub sensitivity: String,
    #[validate(range(min = 1))]
    pub version: i32,
}

#[derive(Debug, Deserialize, Validate)]
pub struct ReclassifyFileRequest {
    pub series_id: Uuid,
    pub sensitivity: Option<String>,
    #[validate(length(min = 1, max = 2000))]
    pub reason: String,
    #[validate(range(min = 1))]
    pub version: i32,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CloseFileRequest {
    #[validate(length(min = 1, max = 2000))]
    pub reason: String,
    #[validate(range(min = 1))]
    pub version: i32,
}

#[derive(Debug, Clone, Serialize)]
pub struct ActivityResponse {
    pub id: Uuid,
    pub aggregate_type: String,
    pub aggregate_id: Uuid,
    pub file_id: Option<Uuid>,
    pub event_type: String,
    pub actor_id: Uuid,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReviewResponse {
    pub id: Uuid,
    pub file_id: Uuid,
    pub file_reference: String,
    pub file_title: String,
    pub recommendation: String,
    pub proposed_retain_until: Option<NaiveDate>,
    pub request_reason: String,
    pub status: String,
    pub version: i32,
    pub requested_by: Uuid,
    pub reviewed_by: Option<Uuid>,
    pub reviewed_at: Option<DateTime<Utc>>,
    pub review_reason: Option<String>,
    pub executed_by: Option<Uuid>,
    pub executed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreateReviewRequest {
    pub recommendation: String,
    pub proposed_retain_until: Option<NaiveDate>,
    #[validate(length(min = 1, max = 2000))]
    pub reason: String,
    #[validate(range(min = 1))]
    pub file_version: i32,
}

#[derive(Debug, Deserialize, Validate)]
pub struct ReviewDecisionRequest {
    #[validate(length(min = 1, max = 2000))]
    pub reason: String,
    #[validate(range(min = 1))]
    pub version: i32,
}

#[derive(Debug, Deserialize, Validate)]
pub struct ExecuteDestructionRequest {
    #[validate(length(min = 1, max = 2000))]
    pub reason: String,
    #[validate(range(min = 1))]
    pub version: i32,
}

#[derive(Debug, Serialize)]
pub struct SeriesPage {
    pub series: Vec<SeriesResponse>,
}
#[derive(Debug, Serialize)]
pub struct FilesPage {
    pub files: Vec<FileResponse>,
}
#[derive(Debug, Serialize)]
pub struct ReviewsPage {
    pub reviews: Vec<ReviewResponse>,
}
#[derive(Debug, Serialize)]
pub struct ActivityPage {
    pub activity: Vec<ActivityResponse>,
}
#[derive(Debug, Serialize)]
pub struct DownloadResponse {
    pub url: String,
    pub expires_in_seconds: u64,
}

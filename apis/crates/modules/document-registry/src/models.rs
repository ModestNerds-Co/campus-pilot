//! Private persistence rows for Document Registry.

use chrono::{DateTime, NaiveDate, Utc};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, FromRow)]
pub(crate) struct NumberingPolicyRow {
    pub prefix: String,
    pub padding: i16,
    pub next_sequence: i64,
    pub version: i32,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
pub(crate) struct SeriesRow {
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

#[derive(Debug, Clone, FromRow)]
pub(crate) struct FileRow {
    pub id: Uuid,
    pub reference: String,
    pub series_id: Uuid,
    pub series_code_snapshot: String,
    pub series_name_snapshot: String,
    pub retention_trigger_snapshot: String,
    pub retention_period_months_snapshot: Option<i16>,
    pub final_disposition_snapshot: String,
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

#[derive(Debug, Clone, FromRow)]
pub(crate) struct ReviewRow {
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

#[derive(Debug, Clone, FromRow)]
pub(crate) struct ActivityRow {
    pub id: Uuid,
    pub aggregate_type: String,
    pub aggregate_id: Uuid,
    pub file_id: Option<Uuid>,
    pub event_type: String,
    pub actor_id: Uuid,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
pub(crate) struct LegalHoldRow {
    pub id: Uuid,
    pub file_id: Uuid,
    pub file_reference: String,
    pub file_title: String,
    pub reference: Option<String>,
    pub reason: String,
    pub status: String,
    pub version: i32,
    pub applied_by: Uuid,
    pub applied_at: DateTime<Utc>,
    pub released_by: Option<Uuid>,
    pub released_at: Option<DateTime<Utc>>,
    pub release_reason: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
pub(crate) struct DeletionJobClaim {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub review_id: Uuid,
    pub file_id: Uuid,
    pub object_key: String,
    pub destruction_reason: String,
    pub requested_by: Uuid,
    pub lease_token: Uuid,
}

//! Private SQL rows for Internal Audit persistence projections.

use chrono::{DateTime, NaiveDate, Utc};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, FromRow)]
pub(crate) struct NumberingPolicyRow {
    pub plan_prefix: String,
    pub engagement_prefix: String,
    pub finding_prefix: String,
    pub padding: i16,
    pub next_plan_sequence: i64,
    pub next_engagement_sequence: i64,
    pub next_finding_sequence: i64,
    pub version: i32,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
pub(crate) struct PlanRow {
    pub id: Uuid,
    pub reference: String,
    pub title: String,
    pub objective: String,
    pub risk_summary: Option<String>,
    pub period_start: NaiveDate,
    pub period_end: NaiveDate,
    pub status: String,
    pub version: i32,
    pub engagement_count: i64,
    pub approved_at: Option<DateTime<Utc>>,
    pub closed_at: Option<DateTime<Utc>>,
    pub close_summary: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
pub(crate) struct EngagementRow {
    pub id: Uuid,
    pub plan_id: Uuid,
    pub plan_reference: String,
    pub plan_title: String,
    pub reference: String,
    pub title: String,
    pub objective: String,
    pub scope_text: String,
    pub lead_auditor_user_id: Uuid,
    pub lead_auditor_name: String,
    pub lead_auditor_email: String,
    pub starts_on: NaiveDate,
    pub due_on: NaiveDate,
    pub status: String,
    pub version: i32,
    pub finding_count: i64,
    pub evidence_count: i64,
    pub started_at: Option<DateTime<Utc>>,
    pub reporting_at: Option<DateTime<Utc>>,
    pub closed_at: Option<DateTime<Utc>>,
    pub close_summary: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
pub(crate) struct EvidenceRow {
    pub id: Uuid,
    pub engagement_id: Uuid,
    pub document_file_id: Uuid,
    pub document_reference: String,
    pub document_title: String,
    pub document_sensitivity: String,
    pub purpose: String,
    pub linked_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
pub(crate) struct FindingRow {
    pub id: Uuid,
    pub engagement_id: Uuid,
    pub engagement_reference: String,
    pub engagement_title: String,
    pub reference: String,
    pub title: String,
    pub rating: String,
    pub criteria: String,
    pub condition: String,
    pub risk_effect: String,
    pub recommendation: String,
    pub status: String,
    pub version: i32,
    pub issued_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

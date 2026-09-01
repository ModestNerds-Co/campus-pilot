//! Closed HTTP and Agent-facing contracts for Internal Audit.

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InternalAuditAccessScope {
    Campus,
    AssignedTo(Uuid),
}

impl InternalAuditAccessScope {
    #[must_use]
    pub const fn assigned_user_id(self) -> Option<Uuid> {
        match self {
            Self::Campus => None,
            Self::AssignedTo(user_id) => Some(user_id),
        }
    }

    #[must_use]
    pub const fn is_campus(self) -> bool {
        matches!(self, Self::Campus)
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FindingRating {
    Low,
    Moderate,
    High,
    Critical,
}

impl FindingRating {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Moderate => "moderate",
            Self::High => "high",
            Self::Critical => "critical",
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct InternalAuditListQuery {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
    pub search: Option<String>,
    pub status: Option<String>,
    pub plan_id: Option<Uuid>,
    pub engagement_id: Option<Uuid>,
    pub rating: Option<FindingRating>,
}

#[derive(Debug, Clone, Serialize)]
pub struct NumberingPolicyResponse {
    pub plan_prefix: String,
    pub engagement_prefix: String,
    pub finding_prefix: String,
    pub padding: i16,
    pub next_plan_sequence: i64,
    pub next_engagement_sequence: i64,
    pub next_finding_sequence: i64,
    pub next_plan_reference: String,
    pub next_engagement_reference: String,
    pub next_finding_reference: String,
    pub version: i32,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateNumberingPolicyRequest {
    #[validate(length(min = 1, max = 16))]
    pub plan_prefix: String,
    #[validate(length(min = 1, max = 16))]
    pub engagement_prefix: String,
    #[validate(length(min = 1, max = 16))]
    pub finding_prefix: String,
    #[validate(range(min = 3, max = 12))]
    pub padding: i16,
    #[validate(range(min = 1))]
    pub next_plan_sequence: i64,
    #[validate(range(min = 1))]
    pub next_engagement_sequence: i64,
    #[validate(range(min = 1))]
    pub next_finding_sequence: i64,
    #[validate(range(min = 1))]
    pub version: i32,
}

#[derive(Debug, Clone, Serialize)]
pub struct PlanResponse {
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

#[derive(Debug, Serialize)]
pub struct PlansPage {
    pub plans: Vec<PlanResponse>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreatePlanRequest {
    #[validate(length(min = 1, max = 200))]
    pub title: String,
    #[validate(length(min = 1, max = 4000))]
    pub objective: String,
    #[validate(length(max = 4000))]
    pub risk_summary: Option<String>,
    pub period_start: NaiveDate,
    pub period_end: NaiveDate,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdatePlanRequest {
    #[validate(length(min = 1, max = 200))]
    pub title: String,
    #[validate(length(min = 1, max = 4000))]
    pub objective: String,
    #[validate(length(max = 4000))]
    pub risk_summary: Option<String>,
    pub period_start: NaiveDate,
    pub period_end: NaiveDate,
    #[validate(range(min = 1))]
    pub expected_version: i32,
}

#[derive(Debug, Deserialize, Validate)]
pub struct VersionRequest {
    #[validate(range(min = 1))]
    pub expected_version: i32,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CloseRequest {
    #[validate(length(min = 1, max = 4000))]
    pub summary: String,
    #[validate(range(min = 1))]
    pub expected_version: i32,
}

#[derive(Debug, Clone, Serialize)]
pub struct AuditorCandidateResponse {
    pub user_id: Uuid,
    pub full_name: String,
    pub email: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct EngagementResponse {
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

#[derive(Debug, Serialize)]
pub struct EngagementsPage {
    pub engagements: Vec<EngagementResponse>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreateEngagementRequest {
    pub plan_id: Uuid,
    #[validate(length(min = 1, max = 200))]
    pub title: String,
    #[validate(length(min = 1, max = 4000))]
    pub objective: String,
    #[validate(length(min = 1, max = 6000))]
    pub scope_text: String,
    pub lead_auditor_user_id: Uuid,
    pub starts_on: NaiveDate,
    pub due_on: NaiveDate,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateEngagementRequest {
    #[validate(length(min = 1, max = 200))]
    pub title: String,
    #[validate(length(min = 1, max = 4000))]
    pub objective: String,
    #[validate(length(min = 1, max = 6000))]
    pub scope_text: String,
    pub lead_auditor_user_id: Uuid,
    pub starts_on: NaiveDate,
    pub due_on: NaiveDate,
    #[validate(range(min = 1))]
    pub expected_version: i32,
}

#[derive(Debug, Clone, Serialize)]
pub struct EvidenceResponse {
    pub id: Uuid,
    pub engagement_id: Uuid,
    pub document_file_id: Uuid,
    pub document_reference: String,
    pub document_title: String,
    pub document_sensitivity: String,
    pub purpose: String,
    pub linked_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct EvidencePage {
    pub evidence: Vec<EvidenceResponse>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct LinkEvidenceRequest {
    pub document_file_id: Uuid,
    #[validate(length(min = 1, max = 2000))]
    pub purpose: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct FindingResponse {
    pub id: Uuid,
    pub engagement_id: Uuid,
    pub engagement_reference: String,
    pub engagement_title: String,
    pub reference: String,
    pub title: String,
    pub rating: FindingRating,
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

#[derive(Debug, Serialize)]
pub struct FindingsPage {
    pub findings: Vec<FindingResponse>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreateFindingRequest {
    #[validate(length(min = 1, max = 240))]
    pub title: String,
    pub rating: FindingRating,
    #[validate(length(min = 1, max = 6000))]
    pub criteria: String,
    #[validate(length(min = 1, max = 6000))]
    pub condition: String,
    #[validate(length(min = 1, max = 6000))]
    pub risk_effect: String,
    #[validate(length(min = 1, max = 6000))]
    pub recommendation: String,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateFindingRequest {
    #[validate(length(min = 1, max = 240))]
    pub title: String,
    pub rating: FindingRating,
    #[validate(length(min = 1, max = 6000))]
    pub criteria: String,
    #[validate(length(min = 1, max = 6000))]
    pub condition: String,
    #[validate(length(min = 1, max = 6000))]
    pub risk_effect: String,
    #[validate(length(min = 1, max = 6000))]
    pub recommendation: String,
    #[validate(range(min = 1))]
    pub expected_version: i32,
}

//! Closed HTTP and Agent-facing contracts for restricted Student Support work.

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

/// Current visibility proven at the request boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StudentSupportAccessScope {
    Campus,
    CaseTeam(Uuid),
}

impl StudentSupportAccessScope {
    #[must_use]
    pub const fn assigned_user_id(self) -> Option<Uuid> {
        match self {
            Self::Campus => None,
            Self::CaseTeam(user_id) => Some(user_id),
        }
    }

    #[must_use]
    pub const fn is_campus(self) -> bool {
        matches!(self, Self::Campus)
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ConcernCategory {
    Wellbeing,
    Behaviour,
    Conduct,
    Safeguarding,
    Family,
    LearningSupport,
    Other,
}

impl ConcernCategory {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Wellbeing => "wellbeing",
            Self::Behaviour => "behaviour",
            Self::Conduct => "conduct",
            Self::Safeguarding => "safeguarding",
            Self::Family => "family",
            Self::LearningSupport => "learning_support",
            Self::Other => "other",
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CaseSeverity {
    Low,
    Moderate,
    High,
    Critical,
}

impl CaseSeverity {
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

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CaseStatus {
    Open,
    Active,
    Escalated,
    Resolved,
    Closed,
}

impl CaseStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Active => "active",
            Self::Escalated => "escalated",
            Self::Resolved => "resolved",
            Self::Closed => "closed",
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CaseActionKind {
    Note,
    Contact,
    Meeting,
    Referral,
    SupportPlan,
    Review,
}

impl CaseActionKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Note => "note",
            Self::Contact => "contact",
            Self::Meeting => "meeting",
            Self::Referral => "referral",
            Self::SupportPlan => "support_plan",
            Self::Review => "review",
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CaseTeamRole {
    Member,
    Reviewer,
}

impl CaseTeamRole {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Member => "member",
            Self::Reviewer => "reviewer",
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct StudentSupportListQuery {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
    pub search: Option<String>,
    pub status: Option<CaseStatus>,
    pub category: Option<ConcernCategory>,
    pub severity: Option<CaseSeverity>,
    pub learner_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct ReferenceQuery {
    pub search: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LearnerCandidateResponse {
    pub learner_id: Uuid,
    pub learner_number: String,
    pub display_name: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CaseWorkerCandidateResponse {
    pub user_id: Uuid,
    pub full_name: String,
    pub email: String,
}

#[derive(Debug, Serialize)]
pub struct StudentSupportReferenceData {
    pub learners: Vec<LearnerCandidateResponse>,
    pub case_workers: Vec<CaseWorkerCandidateResponse>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreateCaseRequest {
    pub learner_id: Uuid,
    pub lead_case_worker_user_id: Option<Uuid>,
    pub category: ConcernCategory,
    pub severity: CaseSeverity,
    #[validate(length(min = 1, max = 200))]
    pub title: String,
    #[validate(length(min = 1, max = 6000))]
    pub summary: String,
    pub occurred_on: Option<NaiveDate>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateCaseRequest {
    pub category: ConcernCategory,
    pub severity: CaseSeverity,
    #[validate(length(min = 1, max = 200))]
    pub title: String,
    #[validate(length(min = 1, max = 6000))]
    pub summary: String,
    pub occurred_on: Option<NaiveDate>,
    #[validate(range(min = 1))]
    pub expected_version: i32,
}

#[derive(Debug, Deserialize, Validate)]
pub struct AssignTeamMemberRequest {
    pub user_id: Uuid,
    pub member_role: CaseTeamRole,
    #[validate(range(min = 1))]
    pub expected_version: i32,
}

#[derive(Debug, Deserialize, Validate)]
pub struct RemoveTeamMemberRequest {
    #[validate(range(min = 1))]
    pub expected_version: i32,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreateCaseActionRequest {
    pub action_kind: CaseActionKind,
    #[validate(length(min = 1, max = 300))]
    pub summary: String,
    #[validate(length(max = 6000))]
    pub details: Option<String>,
    pub occurred_at: DateTime<Utc>,
    #[validate(range(min = 1))]
    pub expected_version: i32,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CaseTransitionRequest {
    #[validate(length(min = 1, max = 6000))]
    pub reason: String,
    #[validate(range(min = 1))]
    pub expected_version: i32,
}

#[derive(Debug, Clone, Serialize)]
pub struct CaseSummaryResponse {
    pub id: Uuid,
    pub reference: String,
    pub learner_id: Uuid,
    pub learner_number: String,
    pub learner_name: String,
    pub lead_case_worker_user_id: Uuid,
    pub lead_case_worker_name: String,
    pub category: String,
    pub severity: String,
    pub title: String,
    pub occurred_on: Option<NaiveDate>,
    pub status: String,
    pub version: i32,
    pub action_count: i64,
    pub team_member_count: i64,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct CasesPage {
    pub cases: Vec<CaseSummaryResponse>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CaseTeamMemberResponse {
    pub user_id: Uuid,
    pub full_name: String,
    pub email: String,
    pub member_role: String,
    pub assigned_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CaseEventResponse {
    pub id: Uuid,
    pub case_id: Uuid,
    pub event_type: String,
    pub actor_id: Uuid,
    pub actor_name: String,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct CaseRecordResponse {
    #[serde(flatten)]
    pub case: CaseSummaryResponse,
    pub summary: String,
    pub escalation_reason: Option<String>,
    pub escalated_at: Option<DateTime<Utc>>,
    pub resolution_summary: Option<String>,
    pub resolved_at: Option<DateTime<Utc>>,
    pub closure_reason: Option<String>,
    pub closed_at: Option<DateTime<Utc>>,
    pub team: Vec<CaseTeamMemberResponse>,
    pub history: Vec<CaseEventResponse>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CaseActionResponse {
    pub id: Uuid,
    pub case_id: Uuid,
    pub action_kind: String,
    pub summary: String,
    pub details: Option<String>,
    pub occurred_at: DateTime<Utc>,
    pub created_by: Uuid,
    pub created_by_name: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct CaseActionsPage {
    pub actions: Vec<CaseActionResponse>,
}

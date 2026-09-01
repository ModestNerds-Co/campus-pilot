//! Private SQL projections for restricted Student Support persistence.

use chrono::{DateTime, NaiveDate, Utc};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, FromRow)]
pub(crate) struct CaseRow {
    pub id: Uuid,
    pub reference: String,
    pub learner_id: Uuid,
    pub lead_case_worker_user_id: Uuid,
    pub lead_case_worker_name: String,
    pub lead_case_worker_email: String,
    pub category: String,
    pub severity: String,
    pub title: String,
    pub summary: String,
    pub occurred_on: Option<NaiveDate>,
    pub status: String,
    pub version: i32,
    pub action_count: i64,
    pub team_member_count: i64,
    pub escalated_at: Option<DateTime<Utc>>,
    pub escalation_reason: Option<String>,
    pub resolved_at: Option<DateTime<Utc>>,
    pub resolution_summary: Option<String>,
    pub closed_at: Option<DateTime<Utc>>,
    pub closure_reason: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
pub(crate) struct TeamMemberRow {
    pub user_id: Uuid,
    pub full_name: String,
    pub email: String,
    pub member_role: String,
    pub assigned_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
pub(crate) struct ActionRow {
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

#[derive(Debug, FromRow)]
pub(crate) struct EventRow {
    pub id: Uuid,
    pub case_id: Uuid,
    pub event_type: String,
    pub actor_id: Uuid,
    pub actor_name: String,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

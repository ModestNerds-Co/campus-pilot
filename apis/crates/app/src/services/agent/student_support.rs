//! Exposes restricted Student Support reads through the Agent broker.
//!
//! Every call reloads current role scopes before the domain query. Case-team
//! removal therefore revokes the next capability call without cached access.

use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, Utc};
use cp_agent::{
    AuthorizedCapabilityContext, Capability, CapabilityDescriptor, CapabilityExecutionError,
    CapabilityExecutionErrorCode, CapabilityResource, CapabilityScope, DataSensitivity,
};
use cp_common::{EffectiveRecordScope, PaginationMeta, RecordScopeFamilyKey};
use cp_student_support::{
    CaseActionResponse, CaseRecordResponse, CaseSeverity, CaseStatus, CaseSummaryResponse,
    CaseTeamMemberResponse, ConcernCategory, StudentSupportAccessScope, StudentSupportListQuery,
    StudentSupportOps,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use crate::services::{access::ops::AccessOps, users::ops::UserOps};

use super::administration::read_descriptor;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ListStudentSupportCasesInput {
    page: Option<i64>,
    per_page: Option<i64>,
    search: Option<String>,
    status: Option<CaseStatus>,
    category: Option<ConcernCategory>,
    severity: Option<CaseSeverity>,
    learner_id: Option<Uuid>,
}

#[derive(Serialize)]
pub(super) struct ListStudentSupportCasesOutput {
    cases: Vec<AgentCaseSummary>,
    pagination: PaginationMeta,
}

#[derive(Debug, Serialize)]
struct AgentCaseSummary {
    id: Uuid,
    reference: String,
    learner_number: String,
    learner_name: String,
    lead_case_worker_name: String,
    category: String,
    severity: String,
    title: String,
    occurred_on: Option<NaiveDate>,
    status: String,
    version: i32,
    action_count: i64,
    team_member_count: i64,
    updated_at: DateTime<Utc>,
}

impl From<CaseSummaryResponse> for AgentCaseSummary {
    fn from(value: CaseSummaryResponse) -> Self {
        Self {
            id: value.id,
            reference: value.reference,
            learner_number: value.learner_number,
            learner_name: value.learner_name,
            lead_case_worker_name: value.lead_case_worker_name,
            category: value.category,
            severity: value.severity,
            title: value.title,
            occurred_on: value.occurred_on,
            status: value.status,
            version: value.version,
            action_count: value.action_count,
            team_member_count: value.team_member_count,
            updated_at: value.updated_at,
        }
    }
}

pub(super) struct StudentSupportCasesListCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}

impl StudentSupportCasesListCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "student_support.cases.list",
                "List assigned Student Support cases",
                "Returns the current account's restricted Student Support worklist with minimum case fields.",
                json!({
                    "page": { "type": ["integer", "null"], "minimum": 1 },
                    "per_page": { "type": ["integer", "null"], "minimum": 1, "maximum": 100 },
                    "search": { "type": ["string", "null"], "maxLength": 200 },
                    "status": { "type": ["string", "null"], "enum": ["open", "active", "escalated", "resolved", "closed", null] },
                    "category": { "type": ["string", "null"], "enum": ["wellbeing", "behaviour", "conduct", "safeguarding", "family", "learning_support", "other", null] },
                    "severity": { "type": ["string", "null"], "enum": ["low", "moderate", "high", "critical", null] },
                    "learner_id": { "type": ["string", "null"], "format": "uuid" }
                }),
                json!({ "cases": { "type": "array" }, "pagination": { "type": "object" } }),
                DataSensitivity::HighlySensitive,
                "student_support.cases",
            ),
        }
    }
}

#[async_trait]
impl Capability for StudentSupportCasesListCapability {
    type Input = ListStudentSupportCasesInput;
    type Output = ListStudentSupportCasesOutput;

    fn descriptor(&self) -> &CapabilityDescriptor {
        &self.descriptor
    }

    fn scope(&self, _input: &Self::Input) -> CapabilityScope {
        CapabilityScope::TenantWide
    }

    async fn execute(
        &self,
        context: AuthorizedCapabilityContext,
        input: Self::Input,
    ) -> Result<Self::Output, CapabilityExecutionError> {
        let principal = context.principal();
        let scope = current_scope(&self.pool, principal.tenant_id(), principal.user_id()).await?;
        let page = input.page.unwrap_or(1).max(1);
        let per_page = input.per_page.unwrap_or(20).clamp(1, 100);
        let query = StudentSupportListQuery {
            page: Some(page),
            per_page: Some(per_page),
            search: input.search,
            status: input.status,
            category: input.category,
            severity: input.severity,
            learner_id: input.learner_id,
        };
        let (cases, total) =
            StudentSupportOps::list_cases(&self.pool, principal.tenant_id(), scope, &query)
                .await
                .map_err(|_| dependency_failure("Student Support cases could not be loaded."))?;
        Ok(ListStudentSupportCasesOutput {
            cases: cases.into_iter().map(AgentCaseSummary::from).collect(),
            pagination: PaginationMeta::new(page as u32, per_page as u32, total),
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ReadStudentSupportCaseInput {
    case_id: Uuid,
}

#[derive(Serialize)]
pub(super) struct ReadStudentSupportCaseOutput {
    case: AgentCaseRecord,
}

#[derive(Debug, Serialize)]
struct AgentCaseRecord {
    #[serde(flatten)]
    case: AgentCaseSummary,
    summary: String,
    escalation_reason: Option<String>,
    escalated_at: Option<DateTime<Utc>>,
    resolution_summary: Option<String>,
    resolved_at: Option<DateTime<Utc>>,
    closure_reason: Option<String>,
    closed_at: Option<DateTime<Utc>>,
    team: Vec<AgentCaseTeamMember>,
    history: Vec<AgentCaseEvent>,
    created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
struct AgentCaseTeamMember {
    full_name: String,
    member_role: String,
    assigned_at: DateTime<Utc>,
}

impl From<CaseTeamMemberResponse> for AgentCaseTeamMember {
    fn from(value: CaseTeamMemberResponse) -> Self {
        Self {
            full_name: value.full_name,
            member_role: value.member_role,
            assigned_at: value.assigned_at,
        }
    }
}

#[derive(Debug, Serialize)]
struct AgentCaseEvent {
    event_type: String,
    actor_name: String,
    created_at: DateTime<Utc>,
}

impl From<CaseRecordResponse> for AgentCaseRecord {
    fn from(value: CaseRecordResponse) -> Self {
        Self {
            case: AgentCaseSummary::from(value.case),
            summary: value.summary,
            escalation_reason: value.escalation_reason,
            escalated_at: value.escalated_at,
            resolution_summary: value.resolution_summary,
            resolved_at: value.resolved_at,
            closure_reason: value.closure_reason,
            closed_at: value.closed_at,
            team: value
                .team
                .into_iter()
                .map(AgentCaseTeamMember::from)
                .collect(),
            history: value
                .history
                .into_iter()
                .map(|event| AgentCaseEvent {
                    event_type: event.event_type,
                    actor_name: event.actor_name,
                    created_at: event.created_at,
                })
                .collect(),
            created_at: value.created_at,
        }
    }
}

pub(super) struct StudentSupportCaseReadCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}

impl StudentSupportCaseReadCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "student_support.cases.read",
                "Read Student Support case",
                "Returns one restricted case, its active team, and redacted lifecycle evidence when currently authorized.",
                json!({ "case_id": { "type": "string", "format": "uuid" } }),
                json!({ "case": { "type": "object" } }),
                DataSensitivity::HighlySensitive,
                "student_support.cases",
            ),
        }
    }
}

#[async_trait]
impl Capability for StudentSupportCaseReadCapability {
    type Input = ReadStudentSupportCaseInput;
    type Output = ReadStudentSupportCaseOutput;

    fn descriptor(&self) -> &CapabilityDescriptor {
        &self.descriptor
    }

    fn scope(&self, input: &Self::Input) -> CapabilityScope {
        case_scope(input.case_id)
    }

    async fn execute(
        &self,
        context: AuthorizedCapabilityContext,
        input: Self::Input,
    ) -> Result<Self::Output, CapabilityExecutionError> {
        let principal = context.principal();
        let scope = current_scope(&self.pool, principal.tenant_id(), principal.user_id()).await?;
        let case =
            StudentSupportOps::get_case(&self.pool, principal.tenant_id(), input.case_id, scope)
                .await
                .map_err(|_| dependency_failure("The Student Support case could not be loaded."))?
                .ok_or_else(|| invalid_state("The Student Support case was not found."))?;
        Ok(ReadStudentSupportCaseOutput {
            case: AgentCaseRecord::from(case),
        })
    }
}

#[derive(Serialize)]
pub(super) struct ListStudentSupportActionsOutput {
    actions: Vec<AgentCaseAction>,
}

#[derive(Debug, Serialize)]
struct AgentCaseAction {
    id: Uuid,
    action_kind: String,
    summary: String,
    details: Option<String>,
    occurred_at: DateTime<Utc>,
    created_by_name: String,
    created_at: DateTime<Utc>,
}

impl From<CaseActionResponse> for AgentCaseAction {
    fn from(value: CaseActionResponse) -> Self {
        Self {
            id: value.id,
            action_kind: value.action_kind,
            summary: value.summary,
            details: value.details,
            occurred_at: value.occurred_at,
            created_by_name: value.created_by_name,
            created_at: value.created_at,
        }
    }
}

pub(super) struct StudentSupportActionsListCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}

impl StudentSupportActionsListCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "student_support.actions.list",
                "List Student Support case actions",
                "Returns the append-only action history for one currently authorized Student Support case.",
                json!({ "case_id": { "type": "string", "format": "uuid" } }),
                json!({ "actions": { "type": "array" } }),
                DataSensitivity::HighlySensitive,
                "student_support.actions",
            ),
        }
    }
}

#[async_trait]
impl Capability for StudentSupportActionsListCapability {
    type Input = ReadStudentSupportCaseInput;
    type Output = ListStudentSupportActionsOutput;

    fn descriptor(&self) -> &CapabilityDescriptor {
        &self.descriptor
    }

    fn scope(&self, input: &Self::Input) -> CapabilityScope {
        case_scope(input.case_id)
    }

    async fn execute(
        &self,
        context: AuthorizedCapabilityContext,
        input: Self::Input,
    ) -> Result<Self::Output, CapabilityExecutionError> {
        let principal = context.principal();
        let scope = current_scope(&self.pool, principal.tenant_id(), principal.user_id()).await?;
        let actions = StudentSupportOps::list_actions(
            &self.pool,
            principal.tenant_id(),
            input.case_id,
            scope,
        )
        .await
        .map_err(|_| dependency_failure("Student Support actions could not be loaded."))?
        .ok_or_else(|| invalid_state("The Student Support case was not found."))?;
        Ok(ListStudentSupportActionsOutput {
            actions: actions.into_iter().map(AgentCaseAction::from).collect(),
        })
    }
}

async fn current_scope(
    pool: &PgPool,
    tenant_id: Uuid,
    user_id: Uuid,
) -> Result<StudentSupportAccessScope, CapabilityExecutionError> {
    let user = UserOps::get_user_by_id(pool, tenant_id, user_id)
        .await
        .map_err(|_| dependency_failure("Current Student Support authority could not be loaded."))?
        .filter(|user| user.is_active)
        .ok_or_else(|| invalid_state("The current Student Support account is unavailable."))?;
    let access = AccessOps::effective_access(pool, tenant_id, &user.roles)
        .await
        .map_err(|_| dependency_failure("Current Student Support access could not be loaded."))?;
    if access
        .permissions
        .iter()
        .any(|permission| permission == "*")
    {
        return Ok(StudentSupportAccessScope::Campus);
    }
    if !access
        .permissions
        .iter()
        .any(|permission| permission == "student_support:view")
    {
        return Err(invalid_state(
            "Student Support permission is no longer available.",
        ));
    }
    let family = RecordScopeFamilyKey::parse("student_support.cases")
        .map_err(|_| invalid_state("The Student Support record scope is invalid."))?;
    match access.record_scopes.effective_scope(&family) {
        Some(EffectiveRecordScope::Campus) => Ok(StudentSupportAccessScope::Campus),
        Some(EffectiveRecordScope::Assigned | EffectiveRecordScope::SelfAndAssigned) => {
            Ok(StudentSupportAccessScope::CaseTeam(user_id))
        }
        Some(EffectiveRecordScope::SelfRecord) | None => Err(invalid_state(
            "Student Support record scope is unavailable.",
        )),
    }
}

fn case_scope(case_id: Uuid) -> CapabilityScope {
    CapabilityScope::resources([CapabilityResource::parse(
        "student_support_case",
        case_id.to_string(),
    )
    .unwrap_or_else(|error| panic!("invalid built-in capability resource: {error}"))])
    .unwrap_or_else(|error| panic!("invalid built-in capability scope: {error}"))
}

fn dependency_failure(message: &'static str) -> CapabilityExecutionError {
    CapabilityExecutionError::new(CapabilityExecutionErrorCode::DependencyUnavailable, message)
}

fn invalid_state(message: &'static str) -> CapabilityExecutionError {
    CapabilityExecutionError::new(CapabilityExecutionErrorCode::InvalidState, message)
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};
    use cp_student_support::{
        CaseEventResponse, CaseRecordResponse, CaseSummaryResponse, CaseTeamMemberResponse,
    };
    use serde_json::json;
    use uuid::Uuid;

    use super::{AgentCaseAction, AgentCaseRecord};

    #[test]
    fn agent_case_projections_omit_internal_people_and_evidence_fields() {
        let now = Utc
            .timestamp_opt(1_788_206_400, 0)
            .single()
            .unwrap_or_else(|| unreachable!());
        let case_id = Uuid::new_v4();
        let learner_id = Uuid::new_v4();
        let worker_id = Uuid::new_v4();
        let record = CaseRecordResponse {
            case: CaseSummaryResponse {
                id: case_id,
                reference: "SSC-000001".to_string(),
                learner_id,
                learner_number: "CP-000001".to_string(),
                learner_name: "Learner One".to_string(),
                lead_case_worker_user_id: worker_id,
                lead_case_worker_name: "Case Worker".to_string(),
                category: "wellbeing".to_string(),
                severity: "moderate".to_string(),
                title: "Support review".to_string(),
                occurred_on: None,
                status: "active".to_string(),
                version: 2,
                action_count: 1,
                team_member_count: 1,
                updated_at: now,
            },
            summary: "Restricted case summary".to_string(),
            escalation_reason: None,
            escalated_at: None,
            resolution_summary: None,
            resolved_at: None,
            closure_reason: None,
            closed_at: None,
            team: vec![CaseTeamMemberResponse {
                user_id: worker_id,
                full_name: "Case Worker".to_string(),
                email: "private@example.test".to_string(),
                member_role: "lead".to_string(),
                assigned_at: now,
            }],
            history: vec![CaseEventResponse {
                id: Uuid::new_v4(),
                case_id,
                event_type: "student_support.case.updated".to_string(),
                actor_id: worker_id,
                actor_name: "Case Worker".to_string(),
                metadata: json!({"user_id": worker_id, "private": "raw evidence"}),
                created_at: now,
            }],
            created_at: now,
        };

        let serialized = serde_json::to_string(&AgentCaseRecord::from(record))
            .unwrap_or_else(|_| unreachable!());
        for forbidden in [
            "learner_id",
            "lead_case_worker_user_id",
            "user_id",
            "actor_id",
            "metadata",
            "private@example.test",
            "raw evidence",
        ] {
            assert!(!serialized.contains(forbidden), "leaked {forbidden}");
        }
        assert!(serialized.contains("Learner One"));
        assert!(serialized.contains("Case Worker"));
    }

    #[test]
    fn agent_action_projection_omits_internal_actor_identifier() {
        let now = Utc
            .timestamp_opt(1_788_206_400, 0)
            .single()
            .unwrap_or_else(|| unreachable!());
        let action = cp_student_support::CaseActionResponse {
            id: Uuid::new_v4(),
            case_id: Uuid::new_v4(),
            action_kind: "meeting".to_string(),
            summary: "Reviewed support plan".to_string(),
            details: None,
            occurred_at: now,
            created_by: Uuid::new_v4(),
            created_by_name: "Case Worker".to_string(),
            created_at: now,
        };

        let serialized = serde_json::to_string(&AgentCaseAction::from(action))
            .unwrap_or_else(|_| unreachable!());
        assert!(!serialized.contains("created_by\""));
        assert!(!serialized.contains("case_id"));
        assert!(serialized.contains("created_by_name"));
    }
}

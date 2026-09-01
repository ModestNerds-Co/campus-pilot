//! Exposes record-scoped Internal Audit reads to the Agent broker.
//!
//! Capabilities reuse the domain's assigned-auditor queries. Evidence results
//! contain governed Document Registry metadata only; no private bytes, object
//! keys, download URLs, or cryptographic storage details cross this boundary.

use async_trait::async_trait;
use cp_agent::{
    AuthorizedCapabilityContext, Capability, CapabilityDescriptor, CapabilityExecutionError,
    CapabilityExecutionErrorCode, CapabilityResource, CapabilityScope, DataSensitivity,
};
use cp_common::{EffectiveRecordScope, RecordScopeFamilyKey};
use cp_internal_audit::{
    FindingRating, InternalAuditAccessScope, InternalAuditListQuery, InternalAuditOps,
};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

use super::administration::read_descriptor;
use crate::services::access::record_scopes::RoleRecordScopeOps;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct InternalAuditReadInput {
    record_id: Option<Uuid>,
    page: Option<i64>,
    per_page: Option<i64>,
    search: Option<String>,
    status: Option<String>,
    plan_id: Option<Uuid>,
    engagement_id: Option<Uuid>,
    rating: Option<FindingRating>,
}

#[derive(Debug, Clone, Copy)]
pub(super) enum InternalAuditReadKind {
    NumberingPolicy,
    PlansList,
    PlanRead,
    AuditorCandidates,
    EngagementsList,
    EngagementRead,
    EvidenceList,
    FindingsList,
    FindingRead,
}

impl InternalAuditReadKind {
    pub(super) const fn operation_key(self) -> &'static str {
        match self {
            Self::NumberingPolicy => "internal_audit.numbering_policy.read",
            Self::PlansList => "internal_audit.plans.list",
            Self::PlanRead => "internal_audit.plans.read",
            Self::AuditorCandidates => "internal_audit.auditor_candidates.list",
            Self::EngagementsList => "internal_audit.engagements.list",
            Self::EngagementRead => "internal_audit.engagements.read",
            Self::EvidenceList => "internal_audit.evidence.list",
            Self::FindingsList => "internal_audit.findings.list",
            Self::FindingRead => "internal_audit.findings.read",
        }
    }

    const fn title(self) -> &'static str {
        match self {
            Self::NumberingPolicy => "Read Internal Audit numbering",
            Self::PlansList => "List audit plans",
            Self::PlanRead => "Read audit plan",
            Self::AuditorCandidates => "List eligible auditors",
            Self::EngagementsList => "List audit engagements",
            Self::EngagementRead => "Read audit engagement",
            Self::EvidenceList => "List audit evidence",
            Self::FindingsList => "List audit findings",
            Self::FindingRead => "Read audit finding",
        }
    }

    const fn dataset(self) -> &'static str {
        match self {
            Self::NumberingPolicy | Self::PlansList | Self::PlanRead => "internal_audit.plans",
            _ => "internal_audit.records",
        }
    }

    const fn resource_kind(self) -> Option<&'static str> {
        match self {
            Self::PlanRead => Some("internal_audit_plan"),
            Self::EngagementRead | Self::EvidenceList => Some("internal_audit_engagement"),
            Self::FindingRead => Some("internal_audit_finding"),
            _ => None,
        }
    }

    const fn needs_plan_scope(self) -> bool {
        matches!(
            self,
            Self::NumberingPolicy | Self::PlansList | Self::PlanRead
        )
    }
}

pub(super) struct InternalAuditReadCapability {
    pool: PgPool,
    kind: InternalAuditReadKind,
    descriptor: CapabilityDescriptor,
}

impl InternalAuditReadCapability {
    pub(super) fn new(pool: PgPool, kind: InternalAuditReadKind) -> Self {
        let sensitivity = if matches!(kind, InternalAuditReadKind::NumberingPolicy) {
            DataSensitivity::General
        } else {
            DataSensitivity::Sensitive
        };
        Self {
            pool,
            kind,
            descriptor: read_descriptor(
                kind.operation_key(),
                kind.title(),
                "Returns only the audit plans or assigned engagement records authorized for the current account.",
                json!({
                    "record_id": {"type":["string","null"],"format":"uuid"},
                    "page": {"type":["integer","null"],"minimum":1},
                    "per_page": {"type":["integer","null"],"minimum":1,"maximum":100},
                    "search": {"type":["string","null"],"maxLength":240},
                    "status": {"type":["string","null"],"maxLength":40},
                    "plan_id": {"type":["string","null"],"format":"uuid"},
                    "engagement_id": {"type":["string","null"],"format":"uuid"},
                    "rating": {"type":["string","null"],"enum":["low","moderate","high","critical",null]}
                }),
                json!({"result":{"type":"object"}}),
                sensitivity,
                kind.dataset(),
            ),
        }
    }
}

#[async_trait]
impl Capability for InternalAuditReadCapability {
    type Input = InternalAuditReadInput;
    type Output = Value;

    fn descriptor(&self) -> &CapabilityDescriptor {
        &self.descriptor
    }

    fn scope(&self, input: &Self::Input) -> CapabilityScope {
        match (self.kind.resource_kind(), input.record_id) {
            (Some(kind), Some(id)) => resource_scope(kind, id),
            _ => CapabilityScope::TenantWide,
        }
    }

    async fn execute(
        &self,
        context: AuthorizedCapabilityContext,
        input: Self::Input,
    ) -> Result<Value, CapabilityExecutionError> {
        let principal = context.principal();
        let authority = load_scope(
            &self.pool,
            principal.tenant_id(),
            principal.user_id(),
            self.kind.needs_plan_scope(),
        )
        .await?;
        let query = InternalAuditListQuery {
            page: input.page,
            per_page: input.per_page,
            search: input.search,
            status: input.status,
            plan_id: input.plan_id,
            engagement_id: input.engagement_id,
            rating: input.rating,
        };
        let result = match self.kind {
            InternalAuditReadKind::NumberingPolicy => json!(
                InternalAuditOps::numbering_policy(&self.pool, principal.tenant_id())
                    .await
                    .map_err(|_| dependency_failure())?
            ),
            InternalAuditReadKind::PlansList => {
                let (plans, total) =
                    InternalAuditOps::list_plans(&self.pool, principal.tenant_id(), &query)
                        .await
                        .map_err(|_| dependency_failure())?;
                json!({"plans":plans,"total":total})
            }
            InternalAuditReadKind::PlanRead => json!(
                InternalAuditOps::get_plan(
                    &self.pool,
                    principal.tenant_id(),
                    required_id(input.record_id)?,
                )
                .await
                .map_err(|_| dependency_failure())?
                .ok_or_else(not_found)?
            ),
            InternalAuditReadKind::AuditorCandidates => json!({
                "auditors": InternalAuditOps::auditor_candidates(
                    &self.pool,
                    principal.tenant_id(),
                    query.search.as_deref(),
                )
                .await
                .map_err(|_| dependency_failure())?
            }),
            InternalAuditReadKind::EngagementsList => {
                let (engagements, total) = InternalAuditOps::list_engagements(
                    &self.pool,
                    principal.tenant_id(),
                    authority,
                    &query,
                )
                .await
                .map_err(|_| dependency_failure())?;
                json!({"engagements":engagements,"total":total})
            }
            InternalAuditReadKind::EngagementRead => json!(
                InternalAuditOps::get_engagement(
                    &self.pool,
                    principal.tenant_id(),
                    authority,
                    required_id(input.record_id)?,
                )
                .await
                .map_err(|_| dependency_failure())?
                .ok_or_else(not_found)?
            ),
            InternalAuditReadKind::EvidenceList => json!({
                "evidence": InternalAuditOps::list_evidence(
                    &self.pool,
                    principal.tenant_id(),
                    authority,
                    required_id(input.record_id)?,
                )
                .await
                .map_err(|_| dependency_failure())?
            }),
            InternalAuditReadKind::FindingsList => {
                let (findings, total) = InternalAuditOps::list_findings(
                    &self.pool,
                    principal.tenant_id(),
                    authority,
                    &query,
                )
                .await
                .map_err(|_| dependency_failure())?;
                json!({"findings":findings,"total":total})
            }
            InternalAuditReadKind::FindingRead => json!(
                InternalAuditOps::get_finding(
                    &self.pool,
                    principal.tenant_id(),
                    authority,
                    required_id(input.record_id)?,
                )
                .await
                .map_err(|_| dependency_failure())?
                .ok_or_else(not_found)?
            ),
        };
        Ok(json!({"result":result}))
    }
}

async fn load_scope(
    pool: &PgPool,
    tenant_id: Uuid,
    user_id: Uuid,
    plan_scope: bool,
) -> Result<InternalAuditAccessScope, CapabilityExecutionError> {
    let role_keys = sqlx::query_scalar::<_, Vec<String>>(
        "SELECT roles FROM users WHERE tenant_id=$1 AND id=$2 AND is_active AND deleted_at IS NULL",
    )
    .bind(tenant_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .map_err(|_| dependency_failure())?
    .ok_or_else(invalid_state)?;
    let wildcard = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM roles WHERE tenant_id=$1 AND key=ANY($2) AND deleted_at IS NULL AND '*'=ANY(permissions))",
    )
    .bind(tenant_id)
    .bind(&role_keys)
    .fetch_one(pool)
    .await
    .map_err(|_| dependency_failure())?;
    if wildcard {
        return Ok(InternalAuditAccessScope::Campus);
    }
    let grants = RoleRecordScopeOps::effective_for_roles(pool, tenant_id, &role_keys)
        .await
        .map_err(|_| dependency_failure())?;
    let family = RecordScopeFamilyKey::parse(if plan_scope {
        "internal_audit.plans"
    } else {
        "internal_audit.records"
    })
    .map_err(|_| invalid_state())?;
    match grants.effective_scope(&family) {
        Some(EffectiveRecordScope::Campus) => Ok(InternalAuditAccessScope::Campus),
        Some(EffectiveRecordScope::Assigned | EffectiveRecordScope::SelfAndAssigned)
            if !plan_scope =>
        {
            Ok(InternalAuditAccessScope::AssignedTo(user_id))
        }
        Some(EffectiveRecordScope::SelfRecord) | None | Some(_) => Err(invalid_state()),
    }
}

fn required_id(value: Option<Uuid>) -> Result<Uuid, CapabilityExecutionError> {
    value.ok_or_else(invalid_state)
}

fn resource_scope(kind: &str, id: Uuid) -> CapabilityScope {
    CapabilityScope::resources([
        CapabilityResource::parse(kind, id.to_string()).unwrap_or_else(|_| unreachable!())
    ])
    .unwrap_or_else(|_| unreachable!())
}

fn dependency_failure() -> CapabilityExecutionError {
    CapabilityExecutionError::new(
        CapabilityExecutionErrorCode::DependencyUnavailable,
        "Internal Audit records could not be loaded.",
    )
}

fn invalid_state() -> CapabilityExecutionError {
    CapabilityExecutionError::new(
        CapabilityExecutionErrorCode::InvalidState,
        "Internal Audit access or input is invalid.",
    )
}

fn not_found() -> CapabilityExecutionError {
    CapabilityExecutionError::new(
        CapabilityExecutionErrorCode::InvalidState,
        "The Internal Audit record was not found.",
    )
}

#[cfg(test)]
mod tests {
    use super::InternalAuditReadKind;

    #[test]
    fn every_internal_audit_read_has_a_stable_operation_key() {
        assert_eq!(
            InternalAuditReadKind::EvidenceList.operation_key(),
            "internal_audit.evidence.list"
        );
        assert_eq!(
            InternalAuditReadKind::FindingRead.operation_key(),
            "internal_audit.findings.read"
        );
    }
}

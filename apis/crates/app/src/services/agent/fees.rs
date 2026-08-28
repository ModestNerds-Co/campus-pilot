//! Agent read adapters for Fees and Billing.

use async_trait::async_trait;
use cp_agent::{
    AuthorizedCapabilityContext, Capability, CapabilityDescriptor, CapabilityExecutionError,
    CapabilityExecutionErrorCode, CapabilityResource, CapabilityScope, DataSensitivity,
};
use cp_common::PaginationMeta;
use cp_fees::foundation::{BillingAccountOps, FeeStructureOps, FeesReferenceOps};
use cp_sis::ops::LearnerOps;
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

use crate::services::{access::ops::AccessOps, users::ops::UserOps};

use super::administration::read_descriptor;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct FeesListInput {
    page: Option<i64>,
    per_page: Option<i64>,
    search: Option<String>,
    status: Option<String>,
}

#[derive(Debug, Clone, Copy)]
pub(super) enum FeesListKind {
    BillingAccounts,
    FeeStructures,
}

impl FeesListKind {
    pub(super) const fn operation_key(self) -> &'static str {
        match self {
            Self::BillingAccounts => "fees.billing_accounts.list",
            Self::FeeStructures => "fees.fee_structures.list",
        }
    }
}

pub(super) struct FeesListCapability {
    pool: PgPool,
    kind: FeesListKind,
    descriptor: CapabilityDescriptor,
}

impl FeesListCapability {
    pub(super) fn new(pool: PgPool, kind: FeesListKind) -> Self {
        let (title, description, output_key, sensitivity, resource) = match kind {
            FeesListKind::BillingAccounts => (
                "List learner billing accounts",
                "Returns billing-account identity and lifecycle data within the caller's current learner scope.",
                "billing_accounts",
                DataSensitivity::Sensitive,
                "fees.billing_accounts",
            ),
            FeesListKind::FeeStructures => (
                "List fee structures",
                "Returns versioned fee definitions with currency and Finance posting-account references.",
                "fee_structures",
                DataSensitivity::General,
                "fees.fee_structures",
            ),
        };
        Self {
            pool,
            kind,
            descriptor: read_descriptor(
                kind.operation_key(),
                title,
                description,
                json!({
                    "page": { "type": ["integer", "null"], "minimum": 1 },
                    "per_page": { "type": ["integer", "null"], "minimum": 1, "maximum": 100 },
                    "search": { "type": ["string", "null"], "maxLength": 200 },
                    "status": { "type": ["string", "null"], "maxLength": 40 }
                }),
                json!({ (output_key): { "type": "array" }, "pagination": { "type": "object" } }),
                sensitivity,
                resource,
            ),
        }
    }
}

#[async_trait]
impl Capability for FeesListCapability {
    type Input = FeesListInput;
    type Output = Value;

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
        let (page, per_page) = bounded_page(input.page, input.per_page);
        match self.kind {
            FeesListKind::BillingAccounts => {
                let visible_learner_ids =
                    billing_visibility(&self.pool, principal.tenant_id(), principal.user_id())
                        .await?;
                let (billing_accounts, total) = BillingAccountOps::list(
                    &self.pool,
                    principal.tenant_id(),
                    page,
                    per_page,
                    trimmed(input.search.as_deref()),
                    trimmed(input.status.as_deref()),
                    visible_learner_ids.as_deref(),
                )
                .await
                .map_err(|_| dependency_failure("Billing accounts could not be loaded."))?;
                Ok(json!({
                    "billing_accounts": billing_accounts,
                    "pagination": PaginationMeta::new(page as u32, per_page as u32, total)
                }))
            }
            FeesListKind::FeeStructures => {
                let (fee_structures, total) = FeeStructureOps::list(
                    &self.pool,
                    principal.tenant_id(),
                    page,
                    per_page,
                    trimmed(input.search.as_deref()),
                    trimmed(input.status.as_deref()),
                )
                .await
                .map_err(|_| dependency_failure("Fee structures could not be loaded."))?;
                Ok(json!({
                    "fee_structures": fee_structures,
                    "pagination": PaginationMeta::new(page as u32, per_page as u32, total)
                }))
            }
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct FeesRecordInput {
    record_id: Uuid,
}

#[derive(Debug, Clone, Copy)]
pub(super) enum FeesReadKind {
    BillingAccount,
    FeeStructure,
}

impl FeesReadKind {
    pub(super) const fn operation_key(self) -> &'static str {
        match self {
            Self::BillingAccount => "fees.billing_accounts.read",
            Self::FeeStructure => "fees.fee_structures.read",
        }
    }
}

pub(super) struct FeesReadCapability {
    pool: PgPool,
    kind: FeesReadKind,
    descriptor: CapabilityDescriptor,
}

impl FeesReadCapability {
    pub(super) fn new(pool: PgPool, kind: FeesReadKind) -> Self {
        let (title, description, resource, sensitivity) = match kind {
            FeesReadKind::BillingAccount => (
                "Read learner billing account",
                "Returns one billing account within the caller's current learner scope.",
                "fees.billing_accounts",
                DataSensitivity::Sensitive,
            ),
            FeesReadKind::FeeStructure => (
                "Read fee structure",
                "Returns one versioned fee definition and its controlled lifecycle.",
                "fees.fee_structures",
                DataSensitivity::General,
            ),
        };
        Self {
            pool,
            kind,
            descriptor: read_descriptor(
                kind.operation_key(),
                title,
                description,
                json!({ "record_id": { "type": "string", "format": "uuid" } }),
                json!({ "record": { "type": "object" } }),
                sensitivity,
                resource,
            ),
        }
    }
}

#[async_trait]
impl Capability for FeesReadCapability {
    type Input = FeesRecordInput;
    type Output = Value;

    fn descriptor(&self) -> &CapabilityDescriptor {
        &self.descriptor
    }

    fn scope(&self, input: &Self::Input) -> CapabilityScope {
        let kind = match self.kind {
            FeesReadKind::BillingAccount => "fees_billing_account",
            FeesReadKind::FeeStructure => "fees_fee_structure",
        };
        CapabilityScope::resources([resource(kind, input.record_id)])
            .unwrap_or_else(|error| panic!("invalid built-in capability scope: {error}"))
    }

    async fn execute(
        &self,
        context: AuthorizedCapabilityContext,
        input: Self::Input,
    ) -> Result<Self::Output, CapabilityExecutionError> {
        let principal = context.principal();
        let record = match self.kind {
            FeesReadKind::BillingAccount => {
                let visible_learner_ids =
                    billing_visibility(&self.pool, principal.tenant_id(), principal.user_id())
                        .await?;
                BillingAccountOps::get_by_id(
                    &self.pool,
                    principal.tenant_id(),
                    input.record_id,
                    visible_learner_ids.as_deref(),
                )
                .await
                .map(|value| value.map(|record| json!(record)))
            }
            FeesReadKind::FeeStructure => {
                FeeStructureOps::get_by_id(&self.pool, principal.tenant_id(), input.record_id)
                    .await
                    .map(|value| value.map(|record| json!(record)))
            }
        }
        .map_err(|_| dependency_failure("The fees record could not be loaded."))?
        .ok_or_else(|| {
            CapabilityExecutionError::new(
                CapabilityExecutionErrorCode::InvalidState,
                "The fees record was not found or is outside the current learner scope.",
            )
        })?;
        Ok(json!({ "record": record }))
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LearnerCandidatesInput {
    search: Option<String>,
}

pub(super) struct FeesLearnerCandidatesCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}

impl FeesLearnerCandidatesCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "fees.learner_candidates.list",
                "List learner billing candidates",
                "Returns the minimum SIS learner projection needed to open a billing account.",
                json!({ "search": { "type": ["string", "null"], "maxLength": 200 } }),
                json!({ "learners": { "type": "array" } }),
                DataSensitivity::Personal,
                "fees.learner_candidates",
            ),
        }
    }
}

#[async_trait]
impl Capability for FeesLearnerCandidatesCapability {
    type Input = LearnerCandidatesInput;
    type Output = Value;

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
        let learners = LearnerOps::billing_references(
            &self.pool,
            context.principal().tenant_id(),
            trimmed(input.search.as_deref()),
            100,
        )
        .await
        .map_err(|_| dependency_failure("Learner billing candidates could not be loaded."))?;
        Ok(json!({ "learners": learners }))
    }
}

pub(super) struct FeesReferenceDataCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}

impl FeesReferenceDataCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "fees.reference_data.read",
                "Read Fees reference data",
                "Returns current Finance posting references and Academics structure available to Fees.",
                json!({}),
                json!({ "reference_data": { "type": "object" } }),
                DataSensitivity::General,
                "fees.reference_data",
            ),
        }
    }
}

#[async_trait]
impl Capability for FeesReferenceDataCapability {
    type Input = Value;
    type Output = Value;

    fn descriptor(&self) -> &CapabilityDescriptor {
        &self.descriptor
    }

    fn scope(&self, _input: &Self::Input) -> CapabilityScope {
        CapabilityScope::TenantWide
    }

    async fn execute(
        &self,
        context: AuthorizedCapabilityContext,
        _input: Self::Input,
    ) -> Result<Self::Output, CapabilityExecutionError> {
        let reference_data = FeesReferenceOps::load(&self.pool, context.principal().tenant_id())
            .await
            .map_err(|_| dependency_failure("Fees reference data could not be loaded."))?;
        Ok(json!({ "reference_data": reference_data }))
    }
}

async fn billing_visibility(
    pool: &PgPool,
    tenant_id: Uuid,
    user_id: Uuid,
) -> Result<Option<Vec<Uuid>>, CapabilityExecutionError> {
    let user = UserOps::get_user_by_id(pool, tenant_id, user_id)
        .await
        .map_err(|_| dependency_failure("Current account access could not be loaded."))?
        .filter(|user| user.is_active)
        .ok_or_else(|| {
            CapabilityExecutionError::new(
                CapabilityExecutionErrorCode::InvalidState,
                "The current account is not available.",
            )
        })?;
    let access = AccessOps::effective_access(pool, tenant_id, &user.roles)
        .await
        .map_err(|_| dependency_failure("Current Fees access could not be loaded."))?;
    if access
        .permissions
        .iter()
        .any(|permission| matches!(permission.as_str(), "*" | "fees:create" | "fees:edit"))
    {
        return Ok(None);
    }
    LearnerOps::ids_for_linked_account(pool, tenant_id, user_id)
        .await
        .map(Some)
        .map_err(|_| dependency_failure("Current learner scope could not be loaded."))
}

fn bounded_page(page: Option<i64>, per_page: Option<i64>) -> (i64, i64) {
    (
        page.unwrap_or(1).max(1),
        per_page.unwrap_or(25).clamp(1, 100),
    )
}

fn trimmed(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

fn resource(kind: &str, id: Uuid) -> CapabilityResource {
    CapabilityResource::parse(kind, id.to_string())
        .unwrap_or_else(|error| panic!("invalid built-in capability resource: {error}"))
}

fn dependency_failure(message: &'static str) -> CapabilityExecutionError {
    CapabilityExecutionError::new(CapabilityExecutionErrorCode::DependencyUnavailable, message)
}

//! Exposes highly sensitive Health reads to installation-local Agent providers.

use async_trait::async_trait;
use cp_agent::{
    AuthorizedCapabilityContext, Capability, CapabilityDescriptor, CapabilityExecutionError,
    CapabilityExecutionErrorCode, CapabilityResource, CapabilityScope, DataSensitivity,
};
use cp_common::{EffectiveRecordScope, PaginationMeta, RecordScopeFamilyKey};
use cp_health::{HealthAccessScope, HealthListQuery, HealthOps};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

use super::administration::read_descriptor;
use crate::services::access::record_scopes::RoleRecordScopeOps;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct EmptyInput {}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct HealthListInput {
    page: Option<i64>,
    per_page: Option<i64>,
    search: Option<String>,
    status: Option<String>,
    patient_id: Option<Uuid>,
}

#[derive(Debug, Clone, Copy)]
pub(super) enum HealthListKind {
    Patients,
    Visits,
    MedicationPlans,
    MedicationAdministrations,
    FollowUps,
}

impl HealthListKind {
    pub(super) const fn operation_key(self) -> &'static str {
        match self {
            Self::Patients => "health.patients.list",
            Self::Visits => "health.visits.list",
            Self::MedicationPlans => "health.medication_plans.list",
            Self::MedicationAdministrations => "health.medication_administrations.list",
            Self::FollowUps => "health.follow_ups.list",
        }
    }

    const fn title(self) -> &'static str {
        match self {
            Self::Patients => "List Health patients",
            Self::Visits => "List clinic visits",
            Self::MedicationPlans => "List medication plans",
            Self::MedicationAdministrations => "List medication administrations",
            Self::FollowUps => "List health follow-ups",
        }
    }

    const fn collection(self) -> &'static str {
        match self {
            Self::Patients => "patients",
            Self::Visits => "visits",
            Self::MedicationPlans => "medication_plans",
            Self::MedicationAdministrations => "administrations",
            Self::FollowUps => "follow_ups",
        }
    }

    const fn scope_family(self) -> &'static str {
        match self {
            Self::Patients => "health.patients",
            Self::Visits
            | Self::MedicationPlans
            | Self::MedicationAdministrations
            | Self::FollowUps => "health.care",
        }
    }
}

pub(super) struct HealthListCapability {
    pool: PgPool,
    kind: HealthListKind,
    descriptor: CapabilityDescriptor,
}

impl HealthListCapability {
    pub(super) fn new(pool: PgPool, kind: HealthListKind) -> Self {
        Self {
            pool,
            kind,
            descriptor: read_descriptor(
                kind.operation_key(),
                kind.title(),
                "Returns current health records within the authenticated person's record scope.",
                json!({
                    "page": { "type": ["integer", "null"], "minimum": 1 },
                    "per_page": { "type": ["integer", "null"], "minimum": 1, "maximum": 100 },
                    "search": { "type": ["string", "null"], "maxLength": 180 },
                    "status": { "type": ["string", "null"], "maxLength": 40 },
                    "patient_id": { "type": ["string", "null"], "format": "uuid" }
                }),
                json!({ (kind.collection()): { "type": "array" }, "pagination": { "type": "object" } }),
                DataSensitivity::HighlySensitive,
                kind.scope_family(),
            ),
        }
    }
}

#[async_trait]
impl Capability for HealthListCapability {
    type Input = HealthListInput;
    type Output = Value;

    fn descriptor(&self) -> &CapabilityDescriptor {
        &self.descriptor
    }

    fn scope(&self, input: &Self::Input) -> CapabilityScope {
        input.patient_id.map_or(CapabilityScope::TenantWide, |id| {
            resource_scope("health_patient", id)
        })
    }

    async fn execute(
        &self,
        context: AuthorizedCapabilityContext,
        input: Self::Input,
    ) -> Result<Self::Output, CapabilityExecutionError> {
        let principal = context.principal();
        let scope = current_scope(
            &self.pool,
            principal.tenant_id(),
            principal.user_id(),
            self.kind.scope_family(),
        )
        .await?;
        let page = input.page.unwrap_or(1).max(1);
        let per_page = input.per_page.unwrap_or(25).clamp(1, 100);
        let query = HealthListQuery {
            page: Some(page),
            per_page: Some(per_page),
            search: input.search,
            status: input.status,
            patient_id: input.patient_id,
        };
        let (records, total) = match self.kind {
            HealthListKind::Patients => {
                let (values, total) =
                    HealthOps::list_patients(&self.pool, principal.tenant_id(), scope, &query)
                        .await
                        .map_err(|_| dependency_failure("Health patients could not be loaded."))?;
                (json!(values), total)
            }
            HealthListKind::Visits => {
                let (values, total) =
                    HealthOps::list_visits(&self.pool, principal.tenant_id(), scope, &query)
                        .await
                        .map_err(|_| dependency_failure("Clinic visits could not be loaded."))?;
                (json!(values), total)
            }
            HealthListKind::MedicationPlans => {
                let (values, total) = HealthOps::list_medication_plans(
                    &self.pool,
                    principal.tenant_id(),
                    scope,
                    &query,
                )
                .await
                .map_err(|_| dependency_failure("Medication plans could not be loaded."))?;
                (json!(values), total)
            }
            HealthListKind::MedicationAdministrations => {
                let (values, total) = HealthOps::list_medication_administrations(
                    &self.pool,
                    principal.tenant_id(),
                    scope,
                    &query,
                )
                .await
                .map_err(|_| {
                    dependency_failure("Medication administrations could not be loaded.")
                })?;
                (json!(values), total)
            }
            HealthListKind::FollowUps => {
                let (values, total) =
                    HealthOps::list_follow_ups(&self.pool, principal.tenant_id(), scope, &query)
                        .await
                        .map_err(|_| {
                            dependency_failure("Health follow-ups could not be loaded.")
                        })?;
                (json!(values), total)
            }
        };
        Ok(json!({
            (self.kind.collection()): records,
            "pagination": PaginationMeta::new(page as u32, per_page as u32, total)
        }))
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct HealthReadInput {
    record_id: Uuid,
}

#[derive(Debug, Clone, Copy)]
pub(super) enum HealthReadKind {
    Patient,
    Visit,
}

impl HealthReadKind {
    pub(super) const fn operation_key(self) -> &'static str {
        match self {
            Self::Patient => "health.patients.read",
            Self::Visit => "health.visits.read",
        }
    }

    const fn resource_kind(self) -> &'static str {
        match self {
            Self::Patient => "health_patient",
            Self::Visit => "health_visit",
        }
    }

    const fn scope_family(self) -> &'static str {
        match self {
            Self::Patient => "health.patients",
            Self::Visit => "health.care",
        }
    }
}

pub(super) struct HealthReadCapability {
    pool: PgPool,
    kind: HealthReadKind,
    descriptor: CapabilityDescriptor,
}

impl HealthReadCapability {
    pub(super) fn new(pool: PgPool, kind: HealthReadKind) -> Self {
        let title = match kind {
            HealthReadKind::Patient => "Read Health patient",
            HealthReadKind::Visit => "Read clinic visit",
        };
        Self {
            pool,
            kind,
            descriptor: read_descriptor(
                kind.operation_key(),
                title,
                "Returns one current health record within the authenticated person's record scope.",
                json!({ "record_id": { "type": "string", "format": "uuid" } }),
                json!({ "record": { "type": "object" } }),
                DataSensitivity::HighlySensitive,
                kind.scope_family(),
            ),
        }
    }
}

#[async_trait]
impl Capability for HealthReadCapability {
    type Input = HealthReadInput;
    type Output = Value;

    fn descriptor(&self) -> &CapabilityDescriptor {
        &self.descriptor
    }

    fn scope(&self, input: &Self::Input) -> CapabilityScope {
        resource_scope(self.kind.resource_kind(), input.record_id)
    }

    async fn execute(
        &self,
        context: AuthorizedCapabilityContext,
        input: Self::Input,
    ) -> Result<Self::Output, CapabilityExecutionError> {
        let principal = context.principal();
        let scope = current_scope(
            &self.pool,
            principal.tenant_id(),
            principal.user_id(),
            self.kind.scope_family(),
        )
        .await?;
        let record = match self.kind {
            HealthReadKind::Patient => {
                HealthOps::get_patient(&self.pool, principal.tenant_id(), input.record_id, scope)
                    .await
                    .map_err(|_| dependency_failure("The Health patient could not be loaded."))?
                    .map(|value| json!(value))
            }
            HealthReadKind::Visit => {
                HealthOps::get_visit(&self.pool, principal.tenant_id(), input.record_id, scope)
                    .await
                    .map_err(|_| dependency_failure("The clinic visit could not be loaded."))?
                    .map(|value| json!(value))
            }
        }
        .ok_or_else(|| invalid_state("The Health record was not found."))?;
        Ok(json!({ "record": record }))
    }
}

pub(super) struct HealthReferencesCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}

impl HealthReferencesCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "health.references.read",
                "Read Health references",
                "Returns current SIS and HR patient candidates and active employees for Health administration.",
                json!({}),
                json!({ "references": { "type": "object" } }),
                DataSensitivity::HighlySensitive,
                "health.patients",
            ),
        }
    }
}

#[async_trait]
impl Capability for HealthReferencesCapability {
    type Input = EmptyInput;
    type Output = Value;

    fn descriptor(&self) -> &CapabilityDescriptor {
        &self.descriptor
    }

    fn scope(&self, _: &Self::Input) -> CapabilityScope {
        CapabilityScope::TenantWide
    }

    async fn execute(
        &self,
        context: AuthorizedCapabilityContext,
        _: Self::Input,
    ) -> Result<Self::Output, CapabilityExecutionError> {
        let principal = context.principal();
        require_campus_scope(
            &self.pool,
            principal.tenant_id(),
            principal.user_id(),
            "health.patients",
        )
        .await?;
        let references = HealthOps::reference_data(&self.pool, principal.tenant_id(), None)
            .await
            .map_err(|_| dependency_failure("Health references could not be loaded."))?;
        Ok(json!({ "references": references }))
    }
}

async fn current_scope(
    pool: &PgPool,
    tenant_id: Uuid,
    user_id: Uuid,
    family: &str,
) -> Result<HealthAccessScope, CapabilityExecutionError> {
    let roles = sqlx::query_scalar::<_, Vec<String>>(
        "SELECT roles FROM users WHERE tenant_id=$1 AND id=$2 AND is_active AND deleted_at IS NULL",
    )
    .bind(tenant_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .map_err(|_| dependency_failure("Health authority could not be loaded."))?
    .ok_or_else(|| invalid_state("The Health account is unavailable."))?;
    let wildcard = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM roles WHERE tenant_id=$1 AND key=ANY($2) AND deleted_at IS NULL AND '*'=ANY(permissions))",
    )
    .bind(tenant_id)
    .bind(&roles)
    .fetch_one(pool)
    .await
    .map_err(|_| dependency_failure("Health authority could not be loaded."))?;
    if wildcard {
        return Ok(HealthAccessScope::Campus);
    }
    let grants = RoleRecordScopeOps::effective_for_roles(pool, tenant_id, &roles)
        .await
        .map_err(|_| dependency_failure("Health record scope could not be loaded."))?;
    let family = RecordScopeFamilyKey::parse(family)
        .map_err(|_| invalid_state("Health record scope is invalid."))?;
    match grants.effective_scope(&family) {
        Some(EffectiveRecordScope::Campus) => Ok(HealthAccessScope::Campus),
        Some(
            EffectiveRecordScope::SelfRecord
            | EffectiveRecordScope::Assigned
            | EffectiveRecordScope::SelfAndAssigned,
        ) => Ok(HealthAccessScope::SelfFor(user_id)),
        None => Err(invalid_state("Health record scope is unavailable.")),
    }
}

async fn require_campus_scope(
    pool: &PgPool,
    tenant_id: Uuid,
    user_id: Uuid,
    family: &str,
) -> Result<(), CapabilityExecutionError> {
    if matches!(
        current_scope(pool, tenant_id, user_id, family).await?,
        HealthAccessScope::Campus
    ) {
        Ok(())
    } else {
        Err(invalid_state("Campus-wide Health scope is required."))
    }
}

fn resource_scope(kind: &str, id: Uuid) -> CapabilityScope {
    CapabilityScope::resources([
        CapabilityResource::parse(kind, id.to_string()).unwrap_or_else(|_| unreachable!())
    ])
    .unwrap_or_else(|_| unreachable!())
}

fn dependency_failure(message: &'static str) -> CapabilityExecutionError {
    CapabilityExecutionError::new(CapabilityExecutionErrorCode::DependencyUnavailable, message)
}

fn invalid_state(message: &'static str) -> CapabilityExecutionError {
    CapabilityExecutionError::new(CapabilityExecutionErrorCode::InvalidState, message)
}

#[cfg(test)]
mod tests {
    use cp_agent::ProviderDataClass;

    use super::{HealthListCapability, HealthListKind};

    #[tokio::test]
    async fn health_reads_are_local_only() {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .connect_lazy("postgresql://unused:unused@127.0.0.1:1/unused")
            .unwrap_or_else(|_| unreachable!());
        let capability = HealthListCapability::new(pool, HealthListKind::Patients);
        assert_eq!(
            capability.descriptor.policy().provider_data_class(),
            ProviderDataClass::LocalOnly
        );
    }
}

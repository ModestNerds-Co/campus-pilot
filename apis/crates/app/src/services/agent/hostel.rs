//! Exposes scoped Hostel reads and allocation previews to the Agent broker.

use async_trait::async_trait;
use chrono::NaiveDate;
use cp_agent::{
    AuthorizedCapabilityContext, Capability, CapabilityDescriptor, CapabilityExecutionError,
    CapabilityExecutionErrorCode, CapabilityResource, CapabilityScope, DataSensitivity,
};
use cp_common::{EffectiveRecordScope, PaginationMeta, RecordScopeFamilyKey};
use cp_hostel::{
    AllocationPreviewRequest, HostelAccessScope, HostelListQuery, HostelOps,
    TransferAllocationPreviewRequest,
};
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
pub(super) struct HostelListInput {
    page: Option<i64>,
    per_page: Option<i64>,
    search: Option<String>,
    status: Option<String>,
    residence_id: Option<Uuid>,
    room_id: Option<Uuid>,
    learner_id: Option<Uuid>,
    category: Option<String>,
}

#[derive(Debug, Clone, Copy)]
pub(super) enum HostelListKind {
    Residences,
    Rooms,
    Allocations,
    PastoralRecords,
}

impl HostelListKind {
    pub(super) const fn operation_key(self) -> &'static str {
        match self {
            Self::Residences => "hostel.residences.list",
            Self::Rooms => "hostel.rooms.list",
            Self::Allocations => "hostel.allocations.list",
            Self::PastoralRecords => "hostel.pastoral_records.list",
        }
    }
    const fn title(self) -> &'static str {
        match self {
            Self::Residences => "List Hostel residences",
            Self::Rooms => "List Hostel rooms and occupancy",
            Self::Allocations => "List learner room allocations",
            Self::PastoralRecords => "List Hostel pastoral records",
        }
    }
    const fn collection(self) -> &'static str {
        match self {
            Self::Residences => "residences",
            Self::Rooms => "rooms",
            Self::Allocations => "allocations",
            Self::PastoralRecords => "pastoral_records",
        }
    }
    const fn sensitivity(self) -> DataSensitivity {
        match self {
            Self::Residences | Self::Rooms => DataSensitivity::General,
            Self::Allocations => DataSensitivity::Personal,
            Self::PastoralRecords => DataSensitivity::Sensitive,
        }
    }
    const fn scope_family(self) -> &'static str {
        match self {
            Self::PastoralRecords => "hostel.pastoral",
            Self::Residences | Self::Rooms | Self::Allocations => "hostel.occupancy",
        }
    }
    const fn campus_only(self) -> bool {
        !matches!(self, Self::Allocations)
    }
}

pub(super) struct HostelListCapability {
    pool: PgPool,
    kind: HostelListKind,
    descriptor: CapabilityDescriptor,
}

impl HostelListCapability {
    pub(super) fn new(pool: PgPool, kind: HostelListKind) -> Self {
        Self {
            pool,
            kind,
            descriptor: read_descriptor(
                kind.operation_key(),
                kind.title(),
                "Returns current Hostel records within the authenticated person's record scope.",
                json!({
                    "page": { "type": ["integer", "null"], "minimum": 1 },
                    "per_page": { "type": ["integer", "null"], "minimum": 1, "maximum": 100 },
                    "search": { "type": ["string", "null"], "maxLength": 180 },
                    "status": { "type": ["string", "null"], "maxLength": 40 },
                    "residence_id": { "type": ["string", "null"], "format": "uuid" },
                    "room_id": { "type": ["string", "null"], "format": "uuid" },
                    "learner_id": { "type": ["string", "null"], "format": "uuid" },
                    "category": { "type": ["string", "null"], "maxLength": 40 }
                }),
                json!({ (kind.collection()): { "type": "array" }, "pagination": { "type": "object" } }),
                kind.sensitivity(),
                kind.scope_family(),
            ),
        }
    }
}

#[async_trait]
impl Capability for HostelListCapability {
    type Input = HostelListInput;
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
        if self.kind.campus_only() && !matches!(scope, HostelAccessScope::Campus) {
            return Err(invalid_state("Campus-wide Hostel scope is required."));
        }
        let page = input.page.unwrap_or(1).max(1);
        let per_page = input.per_page.unwrap_or(25).clamp(1, 100);
        let query = HostelListQuery {
            page: Some(page),
            per_page: Some(per_page),
            search: input.search,
            status: input.status,
            residence_id: input.residence_id,
            room_id: input.room_id,
            learner_id: input.learner_id,
            category: input.category,
        };
        let (records, total) = match self.kind {
            HostelListKind::Residences => {
                let (values, total) =
                    HostelOps::list_residences(&self.pool, principal.tenant_id(), &query)
                        .await
                        .map_err(|_| {
                            dependency_failure("Hostel residences could not be loaded.")
                        })?;
                (json!(values), total)
            }
            HostelListKind::Rooms => {
                let (values, total) =
                    HostelOps::list_rooms(&self.pool, principal.tenant_id(), &query)
                        .await
                        .map_err(|_| dependency_failure("Hostel rooms could not be loaded."))?;
                (json!(values), total)
            }
            HostelListKind::Allocations => {
                let (values, total) =
                    HostelOps::list_allocations(&self.pool, principal.tenant_id(), scope, &query)
                        .await
                        .map_err(|_| {
                            dependency_failure("Hostel allocations could not be loaded.")
                        })?;
                (json!(values), total)
            }
            HostelListKind::PastoralRecords => {
                let (values, total) =
                    HostelOps::list_pastoral_records(&self.pool, principal.tenant_id(), &query)
                        .await
                        .map_err(|_| dependency_failure("Pastoral records could not be loaded."))?;
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
pub(super) struct HostelReadInput {
    record_id: Uuid,
}

#[derive(Debug, Clone, Copy)]
pub(super) enum HostelReadKind {
    Residence,
    Room,
    Allocation,
    PastoralRecord,
}

impl HostelReadKind {
    pub(super) const fn operation_key(self) -> &'static str {
        match self {
            Self::Residence => "hostel.residences.read",
            Self::Room => "hostel.rooms.read",
            Self::Allocation => "hostel.allocations.read",
            Self::PastoralRecord => "hostel.pastoral_records.read",
        }
    }
    const fn title(self) -> &'static str {
        match self {
            Self::Residence => "Read Hostel residence",
            Self::Room => "Read Hostel room",
            Self::Allocation => "Read learner room allocation",
            Self::PastoralRecord => "Read Hostel pastoral record",
        }
    }
    const fn resource_kind(self) -> &'static str {
        match self {
            Self::Residence => "hostel_residence",
            Self::Room => "hostel_room",
            Self::Allocation => "hostel_allocation",
            Self::PastoralRecord => "hostel_pastoral_record",
        }
    }
    const fn sensitivity(self) -> DataSensitivity {
        match self {
            Self::Residence | Self::Room => DataSensitivity::General,
            Self::Allocation => DataSensitivity::Personal,
            Self::PastoralRecord => DataSensitivity::Sensitive,
        }
    }
    const fn scope_family(self) -> &'static str {
        match self {
            Self::PastoralRecord => "hostel.pastoral",
            Self::Residence | Self::Room | Self::Allocation => "hostel.occupancy",
        }
    }
    const fn campus_only(self) -> bool {
        !matches!(self, Self::Allocation)
    }
}

pub(super) struct HostelReadCapability {
    pool: PgPool,
    kind: HostelReadKind,
    descriptor: CapabilityDescriptor,
}

impl HostelReadCapability {
    pub(super) fn new(pool: PgPool, kind: HostelReadKind) -> Self {
        Self {
            pool,
            kind,
            descriptor: read_descriptor(
                kind.operation_key(),
                kind.title(),
                "Returns one current Hostel record within the authenticated person's record scope.",
                json!({ "record_id": { "type": "string", "format": "uuid" } }),
                json!({ "record": { "type": "object" } }),
                kind.sensitivity(),
                kind.scope_family(),
            ),
        }
    }
}

#[async_trait]
impl Capability for HostelReadCapability {
    type Input = HostelReadInput;
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
        if self.kind.campus_only() && !matches!(scope, HostelAccessScope::Campus) {
            return Err(invalid_state("Campus-wide Hostel scope is required."));
        }
        let record = match self.kind {
            HostelReadKind::Residence => {
                HostelOps::get_residence(&self.pool, principal.tenant_id(), input.record_id)
                    .await
                    .map_err(|_| dependency_failure("The residence could not be loaded."))?
                    .map(|value| json!(value))
            }
            HostelReadKind::Room => {
                HostelOps::get_room(&self.pool, principal.tenant_id(), input.record_id)
                    .await
                    .map_err(|_| dependency_failure("The room could not be loaded."))?
                    .map(|value| json!(value))
            }
            HostelReadKind::Allocation => {
                HostelOps::get_allocation(&self.pool, principal.tenant_id(), input.record_id, scope)
                    .await
                    .map_err(|_| dependency_failure("The allocation could not be loaded."))?
                    .map(|value| json!(value))
            }
            HostelReadKind::PastoralRecord => {
                HostelOps::get_pastoral_record(&self.pool, principal.tenant_id(), input.record_id)
                    .await
                    .map_err(|_| dependency_failure("The pastoral record could not be loaded."))?
                    .map(|value| json!(value))
            }
        }
        .ok_or_else(|| invalid_state("The Hostel record was not found."))?;
        Ok(json!({ "record": record }))
    }
}

pub(super) struct HostelReferencesCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}
impl HostelReferencesCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "hostel.references.read",
                "Read Hostel references",
                "Returns active SIS learners and available Hostel rooms for allocation work.",
                json!({}),
                json!({ "references": { "type": "object" } }),
                DataSensitivity::Personal,
                "hostel.occupancy",
            ),
        }
    }
}

#[async_trait]
impl Capability for HostelReferencesCapability {
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
            "hostel.occupancy",
        )
        .await?;
        let references = HostelOps::reference_data(&self.pool, principal.tenant_id(), None)
            .await
            .map_err(|_| dependency_failure("Hostel references could not be loaded."))?;
        Ok(json!({ "references": references }))
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct AllocationPreviewInput {
    learner_id: Uuid,
    room_id: Uuid,
    starts_on: NaiveDate,
    expected_end_on: Option<NaiveDate>,
}

pub(super) struct AllocationPreviewCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}
impl AllocationPreviewCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "hostel.allocations.preview",
                "Preview learner room allocation",
                "Checks current learner and room availability without changing an allocation.",
                json!({
                    "learner_id": { "type": "string", "format": "uuid" },
                    "room_id": { "type": "string", "format": "uuid" },
                    "starts_on": { "type": "string", "format": "date" },
                    "expected_end_on": { "type": ["string", "null"], "format": "date" }
                }),
                json!({ "preview": { "type": "object" } }),
                DataSensitivity::Personal,
                "hostel.occupancy",
            ),
        }
    }
}

#[async_trait]
impl Capability for AllocationPreviewCapability {
    type Input = AllocationPreviewInput;
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
        input: Self::Input,
    ) -> Result<Self::Output, CapabilityExecutionError> {
        let principal = context.principal();
        require_campus_scope(
            &self.pool,
            principal.tenant_id(),
            principal.user_id(),
            "hostel.occupancy",
        )
        .await?;
        let preview = HostelOps::allocation_preview(
            &self.pool,
            principal.tenant_id(),
            &AllocationPreviewRequest {
                learner_id: input.learner_id,
                room_id: input.room_id,
                starts_on: input.starts_on,
                expected_end_on: input.expected_end_on,
                replacing_allocation_id: None,
            },
        )
        .await
        .map_err(|_| dependency_failure("The allocation preview could not be loaded."))?;
        Ok(json!({ "preview": preview }))
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct TransferPreviewInput {
    allocation_id: Uuid,
    expected_version: i32,
    new_room_id: Uuid,
    effective_on: NaiveDate,
}

pub(super) struct TransferPreviewCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}
impl TransferPreviewCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "hostel.allocations.transfer_preview",
                "Preview learner room transfer",
                "Checks a current allocation and destination room without changing either record.",
                json!({
                    "allocation_id": { "type": "string", "format": "uuid" },
                    "expected_version": { "type": "integer", "minimum": 1 },
                    "new_room_id": { "type": "string", "format": "uuid" },
                    "effective_on": { "type": "string", "format": "date" }
                }),
                json!({ "preview": { "type": "object" } }),
                DataSensitivity::Personal,
                "hostel.occupancy",
            ),
        }
    }
}

#[async_trait]
impl Capability for TransferPreviewCapability {
    type Input = TransferPreviewInput;
    type Output = Value;
    fn descriptor(&self) -> &CapabilityDescriptor {
        &self.descriptor
    }
    fn scope(&self, input: &Self::Input) -> CapabilityScope {
        resource_scope("hostel_allocation", input.allocation_id)
    }
    async fn execute(
        &self,
        context: AuthorizedCapabilityContext,
        input: Self::Input,
    ) -> Result<Self::Output, CapabilityExecutionError> {
        let principal = context.principal();
        require_campus_scope(
            &self.pool,
            principal.tenant_id(),
            principal.user_id(),
            "hostel.occupancy",
        )
        .await?;
        let preview = HostelOps::transfer_preview(
            &self.pool,
            principal.tenant_id(),
            input.allocation_id,
            &TransferAllocationPreviewRequest {
                expected_version: input.expected_version,
                new_room_id: input.new_room_id,
                effective_on: input.effective_on,
            },
        )
        .await
        .map_err(|_| dependency_failure("The transfer preview could not be loaded."))?;
        Ok(json!({ "preview": preview }))
    }
}

async fn current_scope(
    pool: &PgPool,
    tenant_id: Uuid,
    user_id: Uuid,
    family: &str,
) -> Result<HostelAccessScope, CapabilityExecutionError> {
    let roles = sqlx::query_scalar::<_, Vec<String>>(
        "SELECT roles FROM users WHERE tenant_id=$1 AND id=$2 AND is_active AND deleted_at IS NULL",
    )
    .bind(tenant_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .map_err(|_| dependency_failure("Hostel authority could not be loaded."))?
    .ok_or_else(|| invalid_state("The Hostel account is unavailable."))?;
    let wildcard = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM roles WHERE tenant_id=$1 AND key=ANY($2) AND deleted_at IS NULL AND '*'=ANY(permissions))",
    )
    .bind(tenant_id)
    .bind(&roles)
    .fetch_one(pool)
    .await
    .map_err(|_| dependency_failure("Hostel authority could not be loaded."))?;
    if wildcard {
        return Ok(HostelAccessScope::Campus);
    }
    let grants = RoleRecordScopeOps::effective_for_roles(pool, tenant_id, &roles)
        .await
        .map_err(|_| dependency_failure("Hostel record scope could not be loaded."))?;
    let family = RecordScopeFamilyKey::parse(family)
        .map_err(|_| invalid_state("Hostel record scope is invalid."))?;
    match grants.effective_scope(&family) {
        Some(EffectiveRecordScope::Campus) => Ok(HostelAccessScope::Campus),
        Some(
            EffectiveRecordScope::SelfRecord
            | EffectiveRecordScope::Assigned
            | EffectiveRecordScope::SelfAndAssigned,
        ) => Ok(HostelAccessScope::SelfFor(user_id)),
        None => Err(invalid_state("Hostel record scope is unavailable.")),
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
        HostelAccessScope::Campus
    ) {
        Ok(())
    } else {
        Err(invalid_state("Campus-wide Hostel scope is required."))
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

    use super::{HostelListCapability, HostelListKind};

    #[tokio::test]
    async fn pastoral_records_require_an_approved_sensitive_provider() {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .connect_lazy("postgresql://unused:unused@127.0.0.1:1/unused")
            .unwrap_or_else(|_| unreachable!());
        let capability = HostelListCapability::new(pool, HostelListKind::PastoralRecords);
        assert_eq!(
            capability.descriptor.policy().provider_data_class(),
            ProviderDataClass::SensitiveDataApproved
        );
    }
}

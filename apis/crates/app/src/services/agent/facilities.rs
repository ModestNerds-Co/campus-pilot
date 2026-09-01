//! Exposes tenant- and role-scoped Facilities reads to the Agent broker.

use async_trait::async_trait;
use cp_agent::{
    AuthorizedCapabilityContext, Capability, CapabilityDescriptor, CapabilityExecutionError,
    CapabilityExecutionErrorCode, CapabilityResource, CapabilityScope, DataSensitivity,
};
use cp_common::{EffectiveRecordScope, PaginationMeta, RecordScopeFamilyKey};
use cp_facilities::{
    FacilitiesOps, FacilitiesRequestScope, FacilitiesWorkOrderScope, FacilityLocationKind,
    FacilityLocationQuery, FacilityPriority, FacilityRequestQuery, FacilityRequestStatus,
    FacilityWorkOrderQuery, FacilityWorkOrderStatus,
};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

use crate::services::{
    access::{models::EffectiveAccess, ops::AccessOps},
    users::ops::UserOps,
};

use super::administration::read_descriptor;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct FacilitiesLocationsInput {
    parent_id: Option<Uuid>,
    kind: Option<FacilityLocationKind>,
    status: Option<String>,
    search: Option<String>,
}

pub(super) struct FacilitiesLocationsCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}

impl FacilitiesLocationsCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "facilities.locations.list",
                "List Facilities locations",
                "Returns the current campus location hierarchy using optional operational filters.",
                json!({
                    "parent_id": { "type": ["string", "null"], "format": "uuid" },
                    "kind": { "type": ["string", "null"], "enum": ["site", "building", "floor", "room", "external_area", null] },
                    "status": { "type": ["string", "null"], "maxLength": 40 },
                    "search": { "type": ["string", "null"], "maxLength": 180 }
                }),
                json!({ "locations": { "type": "array" } }),
                DataSensitivity::General,
                "facilities.locations",
            ),
        }
    }
}

#[async_trait]
impl Capability for FacilitiesLocationsCapability {
    type Input = FacilitiesLocationsInput;
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
        let locations = FacilitiesOps::list_locations(
            &self.pool,
            context.principal().tenant_id(),
            &FacilityLocationQuery {
                parent_id: input.parent_id,
                kind: input.kind,
                status: input.status,
                search: input.search,
            },
        )
        .await
        .map_err(|_| dependency_failure("Facilities locations could not be loaded."))?;
        Ok(json!({ "locations": locations }))
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct FacilitiesRequestsInput {
    page: Option<i64>,
    per_page: Option<i64>,
    status: Option<FacilityRequestStatus>,
    priority: Option<FacilityPriority>,
    location_id: Option<Uuid>,
    search: Option<String>,
}

pub(super) struct FacilitiesRequestsCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}

impl FacilitiesRequestsCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "facilities.requests.list",
                "List Facilities service requests",
                "Returns service requests within the authenticated person's Facilities record scope.",
                json!({
                    "page": { "type": ["integer", "null"], "minimum": 1 },
                    "per_page": { "type": ["integer", "null"], "minimum": 1, "maximum": 100 },
                    "status": { "type": ["string", "null"], "enum": ["open", "assigned", "resolved", "closed", "cancelled", null] },
                    "priority": { "type": ["string", "null"], "enum": ["low", "normal", "high", "urgent", null] },
                    "location_id": { "type": ["string", "null"], "format": "uuid" },
                    "search": { "type": ["string", "null"], "maxLength": 180 }
                }),
                json!({ "requests": { "type": "array" }, "pagination": { "type": "object" } }),
                DataSensitivity::Personal,
                "facilities.requests",
            ),
        }
    }
}

#[async_trait]
impl Capability for FacilitiesRequestsCapability {
    type Input = FacilitiesRequestsInput;
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
        let scope = request_scope(&self.pool, principal.tenant_id(), principal.user_id()).await?;
        let page = input.page.unwrap_or(1).max(1);
        let per_page = input.per_page.unwrap_or(25).clamp(1, 100);
        let (requests, total) = FacilitiesOps::list_requests(
            &self.pool,
            principal.tenant_id(),
            scope,
            &FacilityRequestQuery {
                status: input.status,
                priority: input.priority,
                location_id: input.location_id,
                search: input.search,
                page: Some(page),
                per_page: Some(per_page),
            },
        )
        .await
        .map_err(|_| dependency_failure("Facilities service requests could not be loaded."))?;
        Ok(json!({
            "requests": requests,
            "pagination": PaginationMeta::new(page as u32, per_page as u32, total)
        }))
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct FacilitiesWorkOrdersInput {
    page: Option<i64>,
    per_page: Option<i64>,
    status: Option<FacilityWorkOrderStatus>,
    assigned_employee_id: Option<Uuid>,
    location_id: Option<Uuid>,
    search: Option<String>,
}

pub(super) struct FacilitiesWorkOrdersCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}

impl FacilitiesWorkOrdersCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "facilities.work_orders.list",
                "List Facilities work orders",
                "Returns work orders within the authenticated person's Facilities assignment scope.",
                json!({
                    "page": { "type": ["integer", "null"], "minimum": 1 },
                    "per_page": { "type": ["integer", "null"], "minimum": 1, "maximum": 100 },
                    "status": { "type": ["string", "null"], "enum": ["assigned", "in_progress", "ready_for_inspection", "completed", "cancelled", null] },
                    "assigned_employee_id": { "type": ["string", "null"], "format": "uuid" },
                    "location_id": { "type": ["string", "null"], "format": "uuid" },
                    "search": { "type": ["string", "null"], "maxLength": 180 }
                }),
                json!({ "work_orders": { "type": "array" }, "pagination": { "type": "object" } }),
                DataSensitivity::Personal,
                "facilities.work_orders",
            ),
        }
    }
}

#[async_trait]
impl Capability for FacilitiesWorkOrdersCapability {
    type Input = FacilitiesWorkOrdersInput;
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
        let scope =
            work_order_scope(&self.pool, principal.tenant_id(), principal.user_id()).await?;
        let page = input.page.unwrap_or(1).max(1);
        let per_page = input.per_page.unwrap_or(25).clamp(1, 100);
        let (work_orders, total) = FacilitiesOps::list_work_orders(
            &self.pool,
            principal.tenant_id(),
            scope,
            &FacilityWorkOrderQuery {
                status: input.status,
                assigned_employee_id: input.assigned_employee_id,
                location_id: input.location_id,
                search: input.search,
                page: Some(page),
                per_page: Some(per_page),
            },
        )
        .await
        .map_err(|_| dependency_failure("Facilities work orders could not be loaded."))?;
        Ok(json!({
            "work_orders": work_orders,
            "pagination": PaginationMeta::new(page as u32, per_page as u32, total)
        }))
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct FacilitiesReadInput {
    record_id: Uuid,
}

#[derive(Debug, Clone, Copy)]
pub(super) enum FacilitiesReadKind {
    Location,
    Request,
    WorkOrder,
}

impl FacilitiesReadKind {
    pub(super) const fn operation_key(self) -> &'static str {
        match self {
            Self::Location => "facilities.locations.read",
            Self::Request => "facilities.requests.read",
            Self::WorkOrder => "facilities.work_orders.read",
        }
    }

    const fn title(self) -> &'static str {
        match self {
            Self::Location => "Read Facilities location",
            Self::Request => "Read Facilities service request",
            Self::WorkOrder => "Read Facilities work order",
        }
    }

    const fn resource_kind(self) -> &'static str {
        match self {
            Self::Location => "facility_location",
            Self::Request => "facility_service_request",
            Self::WorkOrder => "facility_work_order",
        }
    }

    const fn sensitivity(self) -> DataSensitivity {
        match self {
            Self::Location => DataSensitivity::General,
            Self::Request | Self::WorkOrder => DataSensitivity::Personal,
        }
    }

    const fn usage_tag(self) -> &'static str {
        match self {
            Self::Location => "facilities.locations",
            Self::Request => "facilities.requests",
            Self::WorkOrder => "facilities.work_orders",
        }
    }
}

pub(super) struct FacilitiesReadCapability {
    pool: PgPool,
    kind: FacilitiesReadKind,
    descriptor: CapabilityDescriptor,
}

impl FacilitiesReadCapability {
    pub(super) fn new(pool: PgPool, kind: FacilitiesReadKind) -> Self {
        Self {
            pool,
            kind,
            descriptor: read_descriptor(
                kind.operation_key(),
                kind.title(),
                "Returns one current Facilities record within the authenticated person's record scope.",
                json!({ "record_id": { "type": "string", "format": "uuid" } }),
                json!({ "record": { "type": "object" } }),
                kind.sensitivity(),
                kind.usage_tag(),
            ),
        }
    }
}

#[async_trait]
impl Capability for FacilitiesReadCapability {
    type Input = FacilitiesReadInput;
    type Output = Value;

    fn descriptor(&self) -> &CapabilityDescriptor {
        &self.descriptor
    }

    fn scope(&self, input: &Self::Input) -> CapabilityScope {
        CapabilityScope::resources([CapabilityResource::parse(
            self.kind.resource_kind(),
            input.record_id.to_string(),
        )
        .unwrap_or_else(|_| unreachable!())])
        .unwrap_or_else(|_| unreachable!())
    }

    async fn execute(
        &self,
        context: AuthorizedCapabilityContext,
        input: Self::Input,
    ) -> Result<Self::Output, CapabilityExecutionError> {
        let principal = context.principal();
        let record = match self.kind {
            FacilitiesReadKind::Location => {
                FacilitiesOps::get_location(&self.pool, principal.tenant_id(), input.record_id)
                    .await
                    .map_err(|_| {
                        dependency_failure("The Facilities location could not be loaded.")
                    })?
                    .map(|value| json!(value))
            }
            FacilitiesReadKind::Request => {
                let scope =
                    request_scope(&self.pool, principal.tenant_id(), principal.user_id()).await?;
                FacilitiesOps::get_request(
                    &self.pool,
                    principal.tenant_id(),
                    input.record_id,
                    scope,
                )
                .await
                .map_err(|_| {
                    dependency_failure("The Facilities service request could not be loaded.")
                })?
                .map(|value| json!(value))
            }
            FacilitiesReadKind::WorkOrder => {
                let scope =
                    work_order_scope(&self.pool, principal.tenant_id(), principal.user_id())
                        .await?;
                FacilitiesOps::get_work_order(
                    &self.pool,
                    principal.tenant_id(),
                    input.record_id,
                    scope,
                )
                .await
                .map_err(|_| dependency_failure("The Facilities work order could not be loaded."))?
                .map(|value| json!(value))
            }
        }
        .ok_or_else(|| invalid_state("The Facilities record was not found."))?;
        Ok(json!({ "record": record }))
    }
}

async fn current_access(
    pool: &PgPool,
    tenant_id: Uuid,
    user_id: Uuid,
) -> Result<EffectiveAccess, CapabilityExecutionError> {
    let user = UserOps::get_user_by_id(pool, tenant_id, user_id)
        .await
        .map_err(|_| dependency_failure("Current Facilities authority could not be loaded."))?
        .filter(|user| user.is_active)
        .ok_or_else(|| invalid_state("The current Facilities account is unavailable."))?;
    AccessOps::effective_access(pool, tenant_id, &user.roles)
        .await
        .map_err(|_| dependency_failure("Current Facilities access could not be loaded."))
}

async fn request_scope(
    pool: &PgPool,
    tenant_id: Uuid,
    user_id: Uuid,
) -> Result<FacilitiesRequestScope, CapabilityExecutionError> {
    let access = current_access(pool, tenant_id, user_id).await?;
    if access
        .permissions
        .iter()
        .any(|permission| permission == "*")
    {
        return Ok(FacilitiesRequestScope::Campus);
    }
    let family = RecordScopeFamilyKey::parse("facilities.requests")
        .map_err(|_| invalid_state("The Facilities request scope is invalid."))?;
    match access.record_scopes.effective_scope(&family) {
        Some(EffectiveRecordScope::Campus) => Ok(FacilitiesRequestScope::Campus),
        Some(EffectiveRecordScope::SelfRecord | EffectiveRecordScope::SelfAndAssigned) => {
            Ok(FacilitiesRequestScope::SelfRecord(user_id))
        }
        Some(EffectiveRecordScope::Assigned) | None => {
            Err(invalid_state("Facilities request scope is unavailable."))
        }
    }
}

async fn work_order_scope(
    pool: &PgPool,
    tenant_id: Uuid,
    user_id: Uuid,
) -> Result<FacilitiesWorkOrderScope, CapabilityExecutionError> {
    let access = current_access(pool, tenant_id, user_id).await?;
    if access
        .permissions
        .iter()
        .any(|permission| permission == "*")
    {
        return Ok(FacilitiesWorkOrderScope::Campus);
    }
    let family = RecordScopeFamilyKey::parse("facilities.work_orders")
        .map_err(|_| invalid_state("The Facilities work-order scope is invalid."))?;
    match access.record_scopes.effective_scope(&family) {
        Some(EffectiveRecordScope::Campus) => Ok(FacilitiesWorkOrderScope::Campus),
        Some(EffectiveRecordScope::Assigned | EffectiveRecordScope::SelfAndAssigned) => {
            Ok(FacilitiesWorkOrderScope::AssignedAccount(user_id))
        }
        Some(EffectiveRecordScope::SelfRecord) | None => {
            Err(invalid_state("Facilities work-order scope is unavailable."))
        }
    }
}

fn dependency_failure(message: &'static str) -> CapabilityExecutionError {
    CapabilityExecutionError::new(CapabilityExecutionErrorCode::DependencyUnavailable, message)
}

fn invalid_state(message: &'static str) -> CapabilityExecutionError {
    CapabilityExecutionError::new(CapabilityExecutionErrorCode::InvalidState, message)
}

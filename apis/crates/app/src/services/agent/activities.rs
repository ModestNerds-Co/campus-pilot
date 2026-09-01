//! Exposes role- and record-scoped Activities reads to the Agent broker.

use async_trait::async_trait;
use chrono::NaiveDate;
use cp_activities::{
    ActivitiesOps, ActivitiesScope, ActivityCatalogQuery, ActivityCatalogStatus, ActivityCategory,
    ActivityGroupQuery, ActivityGroupStatus, ActivitySessionQuery, ActivitySessionStatus,
};
use cp_agent::{
    AuthorizedCapabilityContext, Capability, CapabilityDescriptor, CapabilityExecutionError,
    CapabilityExecutionErrorCode, CapabilityResource, CapabilityScope, DataSensitivity,
};
use cp_common::{EffectiveRecordScope, PaginationMeta, RecordScopeFamilyKey};
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
pub(super) struct ActivitiesListInput {
    page: Option<i64>,
    per_page: Option<i64>,
    search: Option<String>,
    category: Option<ActivityCategory>,
    catalog_status: Option<ActivityCatalogStatus>,
    activity_id: Option<Uuid>,
    group_id: Option<Uuid>,
    group_status: Option<ActivityGroupStatus>,
    session_status: Option<ActivitySessionStatus>,
    active_on: Option<NaiveDate>,
    date_from: Option<NaiveDate>,
    date_to: Option<NaiveDate>,
}

#[derive(Debug, Clone, Copy)]
pub(super) enum ActivitiesListKind {
    Catalog,
    Groups,
    Sessions,
}

impl ActivitiesListKind {
    pub(super) const fn operation_key(self) -> &'static str {
        match self {
            Self::Catalog => "activities.catalog.list",
            Self::Groups => "activities.groups.list",
            Self::Sessions => "activities.sessions.list",
        }
    }
    const fn title(self) -> &'static str {
        match self {
            Self::Catalog => "List activities",
            Self::Groups => "List activity groups",
            Self::Sessions => "List activity sessions",
        }
    }
    const fn usage_tag(self) -> &'static str {
        match self {
            Self::Catalog => "activities.catalog",
            Self::Groups => "activities.groups",
            Self::Sessions => "activities.sessions",
        }
    }
    fn input_schema(self) -> Value {
        match self {
            Self::Catalog => json!({
                "search": { "type": ["string", "null"], "maxLength": 180 },
                "category": { "type": ["string", "null"], "enum": ["sport", "club", "arts", "service", "society", "academic_enrichment", "other", null] },
                "catalog_status": { "type": ["string", "null"], "enum": ["active", "archived", null] }
            }),
            Self::Groups => json!({
                "page": { "type": ["integer", "null"], "minimum": 1 },
                "per_page": { "type": ["integer", "null"], "minimum": 1, "maximum": 100 },
                "search": { "type": ["string", "null"], "maxLength": 180 },
                "activity_id": { "type": ["string", "null"], "format": "uuid" },
                "group_status": { "type": ["string", "null"], "enum": ["draft", "active", "closed", "cancelled", null] },
                "active_on": { "type": ["string", "null"], "format": "date" }
            }),
            Self::Sessions => json!({
                "page": { "type": ["integer", "null"], "minimum": 1 },
                "per_page": { "type": ["integer", "null"], "minimum": 1, "maximum": 100 },
                "search": { "type": ["string", "null"], "maxLength": 180 },
                "group_id": { "type": ["string", "null"], "format": "uuid" },
                "session_status": { "type": ["string", "null"], "enum": ["scheduled", "completed", "cancelled", null] },
                "date_from": { "type": ["string", "null"], "format": "date" },
                "date_to": { "type": ["string", "null"], "format": "date" }
            }),
        }
    }
}

pub(super) struct ActivitiesListCapability {
    pool: PgPool,
    kind: ActivitiesListKind,
    descriptor: CapabilityDescriptor,
}

impl ActivitiesListCapability {
    pub(super) fn new(pool: PgPool, kind: ActivitiesListKind) -> Self {
        Self {
            pool,
            kind,
            descriptor: read_descriptor(
                kind.operation_key(),
                kind.title(),
                "Returns current Activities records within the authenticated person's role and record scope.",
                kind.input_schema(),
                json!({ "records": { "type": "array" }, "pagination": { "type": ["object", "null"] } }),
                if matches!(kind, ActivitiesListKind::Catalog) {
                    DataSensitivity::General
                } else {
                    DataSensitivity::Personal
                },
                kind.usage_tag(),
            ),
        }
    }
}

#[async_trait]
impl Capability for ActivitiesListCapability {
    type Input = ActivitiesListInput;
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
        match self.kind {
            ActivitiesListKind::Catalog => {
                let records = ActivitiesOps::list_catalog(
                    &self.pool,
                    principal.tenant_id(),
                    &ActivityCatalogQuery {
                        search: input.search,
                        category: input.category,
                        status: input.catalog_status,
                    },
                )
                .await
                .map_err(|_| dependency_failure("The Activities catalog could not be loaded."))?;
                Ok(json!({ "records": records, "pagination": null }))
            }
            ActivitiesListKind::Groups => {
                let scope = current_scope(
                    &self.pool,
                    principal.tenant_id(),
                    principal.user_id(),
                    "activities.groups",
                )
                .await?;
                let page = input.page.unwrap_or(1).max(1);
                let per_page = input.per_page.unwrap_or(25).clamp(1, 100);
                let (records, total) = ActivitiesOps::list_groups(
                    &self.pool,
                    principal.tenant_id(),
                    scope,
                    &ActivityGroupQuery {
                        page: Some(page),
                        per_page: Some(per_page),
                        search: input.search,
                        activity_id: input.activity_id,
                        status: input.group_status,
                        active_on: input.active_on,
                    },
                )
                .await
                .map_err(|_| dependency_failure("Activity groups could not be loaded."))?;
                Ok(
                    json!({ "records": records, "pagination": PaginationMeta::new(page as u32, per_page as u32, total) }),
                )
            }
            ActivitiesListKind::Sessions => {
                let scope = current_scope(
                    &self.pool,
                    principal.tenant_id(),
                    principal.user_id(),
                    "activities.sessions",
                )
                .await?;
                let page = input.page.unwrap_or(1).max(1);
                let per_page = input.per_page.unwrap_or(25).clamp(1, 100);
                let (records, total) = ActivitiesOps::list_sessions(
                    &self.pool,
                    principal.tenant_id(),
                    scope,
                    &ActivitySessionQuery {
                        page: Some(page),
                        per_page: Some(per_page),
                        search: input.search,
                        group_id: input.group_id,
                        status: input.session_status,
                        date_from: input.date_from,
                        date_to: input.date_to,
                    },
                )
                .await
                .map_err(|_| dependency_failure("Activity sessions could not be loaded."))?;
                Ok(
                    json!({ "records": records, "pagination": PaginationMeta::new(page as u32, per_page as u32, total) }),
                )
            }
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ActivitiesReadInput {
    record_id: Uuid,
}

#[derive(Debug, Clone, Copy)]
pub(super) enum ActivitiesReadKind {
    Catalog,
    Group,
    Session,
}

impl ActivitiesReadKind {
    pub(super) const fn operation_key(self) -> &'static str {
        match self {
            Self::Catalog => "activities.catalog.read",
            Self::Group => "activities.groups.read",
            Self::Session => "activities.sessions.read",
        }
    }
    const fn title(self) -> &'static str {
        match self {
            Self::Catalog => "Read activity",
            Self::Group => "Read activity group",
            Self::Session => "Read activity session",
        }
    }
    const fn resource_kind(self) -> &'static str {
        match self {
            Self::Catalog => "activity_catalog_item",
            Self::Group => "activity_group",
            Self::Session => "activity_session",
        }
    }
    const fn usage_tag(self) -> &'static str {
        match self {
            Self::Catalog => "activities.catalog",
            Self::Group => "activities.groups",
            Self::Session => "activities.sessions",
        }
    }
}

pub(super) struct ActivitiesReadCapability {
    pool: PgPool,
    kind: ActivitiesReadKind,
    descriptor: CapabilityDescriptor,
}

impl ActivitiesReadCapability {
    pub(super) fn new(pool: PgPool, kind: ActivitiesReadKind) -> Self {
        Self {
            pool,
            kind,
            descriptor: read_descriptor(
                kind.operation_key(),
                kind.title(),
                "Returns one Activities record within the authenticated person's role and record scope.",
                json!({ "record_id": { "type": "string", "format": "uuid" } }),
                json!({ "record": { "type": "object" } }),
                if matches!(kind, ActivitiesReadKind::Catalog) {
                    DataSensitivity::General
                } else {
                    DataSensitivity::Personal
                },
                kind.usage_tag(),
            ),
        }
    }
}

#[async_trait]
impl Capability for ActivitiesReadCapability {
    type Input = ActivitiesReadInput;
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
            ActivitiesReadKind::Catalog => {
                ActivitiesOps::get_catalog_item(&self.pool, principal.tenant_id(), input.record_id)
                    .await
                    .map_err(|_| dependency_failure("The activity could not be loaded."))?
                    .map(|value| json!(value))
            }
            ActivitiesReadKind::Group => {
                let scope = current_scope(
                    &self.pool,
                    principal.tenant_id(),
                    principal.user_id(),
                    "activities.groups",
                )
                .await?;
                ActivitiesOps::get_group(&self.pool, principal.tenant_id(), scope, input.record_id)
                    .await
                    .map_err(|_| dependency_failure("The activity group could not be loaded."))?
                    .map(|value| json!(value))
            }
            ActivitiesReadKind::Session => {
                let scope = current_scope(
                    &self.pool,
                    principal.tenant_id(),
                    principal.user_id(),
                    "activities.sessions",
                )
                .await?;
                ActivitiesOps::get_session(
                    &self.pool,
                    principal.tenant_id(),
                    scope,
                    input.record_id,
                )
                .await
                .map_err(|_| dependency_failure("The activity session could not be loaded."))?
                .map(|value| json!(value))
            }
        }
        .ok_or_else(|| invalid_state("The Activities record was not found."))?;
        Ok(json!({ "record": record }))
    }
}

async fn current_scope(
    pool: &PgPool,
    tenant_id: Uuid,
    user_id: Uuid,
    family: &str,
) -> Result<ActivitiesScope, CapabilityExecutionError> {
    let access = current_access(pool, tenant_id, user_id).await?;
    if access
        .permissions
        .iter()
        .any(|permission| permission == "*")
    {
        return Ok(ActivitiesScope::Campus);
    }
    let family = RecordScopeFamilyKey::parse(family)
        .map_err(|_| invalid_state("The Activities record scope is invalid."))?;
    match access.record_scopes.effective_scope(&family) {
        Some(EffectiveRecordScope::Campus) => Ok(ActivitiesScope::Campus),
        Some(EffectiveRecordScope::SelfRecord) => Ok(ActivitiesScope::SelfAccount(user_id)),
        Some(EffectiveRecordScope::Assigned) => Ok(ActivitiesScope::AssignedAccount(user_id)),
        Some(EffectiveRecordScope::SelfAndAssigned) => {
            Ok(ActivitiesScope::SelfAndAssigned(user_id))
        }
        None => Err(invalid_state("Activities record scope is unavailable.")),
    }
}

async fn current_access(
    pool: &PgPool,
    tenant_id: Uuid,
    user_id: Uuid,
) -> Result<EffectiveAccess, CapabilityExecutionError> {
    let user = UserOps::get_user_by_id(pool, tenant_id, user_id)
        .await
        .map_err(|_| dependency_failure("Current Activities authority could not be loaded."))?
        .filter(|user| user.is_active)
        .ok_or_else(|| invalid_state("The current Activities account is unavailable."))?;
    AccessOps::effective_access(pool, tenant_id, &user.roles)
        .await
        .map_err(|_| dependency_failure("Current Activities access could not be loaded."))
}

fn dependency_failure(message: &'static str) -> CapabilityExecutionError {
    CapabilityExecutionError::new(CapabilityExecutionErrorCode::DependencyUnavailable, message)
}
fn invalid_state(message: &'static str) -> CapabilityExecutionError {
    CapabilityExecutionError::new(CapabilityExecutionErrorCode::InvalidState, message)
}

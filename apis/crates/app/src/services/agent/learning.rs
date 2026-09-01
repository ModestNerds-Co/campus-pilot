//! Exposes record-scoped E-learning metadata through the Agent broker.

use async_trait::async_trait;
use cp_agent::{
    AuthorizedCapabilityContext, Capability, CapabilityDescriptor, CapabilityExecutionError,
    CapabilityExecutionErrorCode, CapabilityResource, CapabilityScope, DataSensitivity,
};
use cp_common::{EffectiveRecordScope, PaginationMeta, RecordScopeFamilyKey};
use cp_learning::{
    LearningAccessScope, LearningOps, LearningReferenceData, LearningResourceFileQuery,
    LearningSettingsResponse, LearningSpaceListQuery, LearningSpaceResponse, LearningSpaceStatus,
    LearningSpaceSummary,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use crate::services::{access::ops::AccessOps, users::ops::UserOps};

use super::administration::read_descriptor;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct EmptyInput {}

#[derive(Serialize)]
pub(super) struct LearningSettingsOutput {
    settings: LearningSettingsResponse,
}

pub(super) struct LearningSettingsCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}

impl LearningSettingsCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "learning.settings.read",
                "Read E-learning settings",
                "Returns the governed document series configured for E-learning resources.",
                json!({}),
                json!({ "settings": { "type": "object" } }),
                DataSensitivity::General,
                "learning.settings",
            ),
        }
    }
}

#[async_trait]
impl Capability for LearningSettingsCapability {
    type Input = EmptyInput;
    type Output = LearningSettingsOutput;

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
        let settings = LearningOps::settings(&self.pool, context.principal().tenant_id())
            .await
            .map_err(|_| dependency_failure("E-learning settings could not be loaded."))?;
        Ok(LearningSettingsOutput { settings })
    }
}

#[derive(Serialize)]
pub(super) struct LearningReferencesOutput {
    references: LearningReferenceData,
}

pub(super) struct LearningReferencesCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}

impl LearningReferencesCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "learning.references.read",
                "Read E-learning references",
                "Returns the active term and teaching assignments visible to the current account.",
                json!({}),
                json!({ "references": { "type": "object" } }),
                DataSensitivity::Personal,
                "learning.references",
            ),
        }
    }
}

#[async_trait]
impl Capability for LearningReferencesCapability {
    type Input = EmptyInput;
    type Output = LearningReferencesOutput;

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
        let principal = context.principal();
        let scope = current_scope(&self.pool, principal.tenant_id(), principal.user_id()).await?;
        let references = LearningOps::references(&self.pool, principal.tenant_id(), scope)
            .await
            .map_err(|_| dependency_failure("E-learning references could not be loaded."))?;
        Ok(LearningReferencesOutput { references })
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LearningResourceFilesInput {
    search: Option<String>,
    limit: Option<i64>,
}

#[derive(Serialize)]
pub(super) struct LearningResourceFilesOutput {
    files: Vec<cp_document_registry::EvidenceFileReference>,
}

pub(super) struct LearningResourceFilesCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}

impl LearningResourceFilesCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "learning.resource_files.list",
                "List E-learning resource files",
                "Returns governed, non-restricted file metadata available for a learning resource.",
                json!({
                    "search": { "type": ["string", "null"] },
                    "limit": { "type": ["integer", "null"], "minimum": 1, "maximum": 100 }
                }),
                json!({ "files": { "type": "array" } }),
                DataSensitivity::Sensitive,
                "learning.resource_files",
            ),
        }
    }
}

#[async_trait]
impl Capability for LearningResourceFilesCapability {
    type Input = LearningResourceFilesInput;
    type Output = LearningResourceFilesOutput;

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
        let files = LearningOps::resource_file_candidates(
            &self.pool,
            context.principal().tenant_id(),
            &LearningResourceFileQuery {
                search: input.search,
                limit: input.limit.map(|limit| limit.clamp(1, 100)),
            },
        )
        .await
        .map_err(|_| dependency_failure("E-learning resource files could not be loaded."))?;
        Ok(LearningResourceFilesOutput { files })
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LearningSpacesInput {
    page: Option<i64>,
    per_page: Option<i64>,
    search: Option<String>,
    status: Option<LearningSpaceStatus>,
}

#[derive(Serialize)]
pub(super) struct LearningSpacesOutput {
    spaces: Vec<LearningSpaceSummary>,
    pagination: PaginationMeta,
}

pub(super) struct LearningSpacesCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}

impl LearningSpacesCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "learning.spaces.list",
                "List E-learning spaces",
                "Returns only learning spaces visible through the current teaching assignment or learner enrolment.",
                json!({
                    "page": { "type": ["integer", "null"], "minimum": 1 },
                    "per_page": { "type": ["integer", "null"], "minimum": 1, "maximum": 100 },
                    "search": { "type": ["string", "null"] },
                    "status": { "type": ["string", "null"], "enum": ["draft", "published", "archived", null] }
                }),
                json!({ "spaces": { "type": "array" }, "pagination": { "type": "object" } }),
                DataSensitivity::Personal,
                "learning.spaces",
            ),
        }
    }
}

#[async_trait]
impl Capability for LearningSpacesCapability {
    type Input = LearningSpacesInput;
    type Output = LearningSpacesOutput;

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
        let per_page = input.per_page.unwrap_or(25).clamp(1, 100);
        let (spaces, total) = LearningOps::list_spaces(
            &self.pool,
            principal.tenant_id(),
            scope,
            &LearningSpaceListQuery {
                page: Some(page),
                per_page: Some(per_page),
                search: input.search,
                status: input.status,
            },
        )
        .await
        .map_err(|_| dependency_failure("E-learning spaces could not be loaded."))?;
        Ok(LearningSpacesOutput {
            spaces,
            pagination: PaginationMeta::new(page as u32, per_page as u32, total),
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LearningSpaceInput {
    space_id: Uuid,
}

#[derive(Serialize)]
pub(super) struct LearningSpaceOutput {
    space: LearningSpaceResponse,
}

pub(super) struct LearningSpaceCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}

impl LearningSpaceCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "learning.spaces.read",
                "Read E-learning space",
                "Returns one visible learning space with its units and governed resource metadata.",
                json!({ "space_id": { "type": "string", "format": "uuid" } }),
                json!({ "space": { "type": "object" } }),
                DataSensitivity::Personal,
                "learning.spaces",
            ),
        }
    }
}

#[async_trait]
impl Capability for LearningSpaceCapability {
    type Input = LearningSpaceInput;
    type Output = LearningSpaceOutput;

    fn descriptor(&self) -> &CapabilityDescriptor {
        &self.descriptor
    }

    fn scope(&self, input: &Self::Input) -> CapabilityScope {
        CapabilityScope::resources([CapabilityResource::parse(
            "learning_space",
            input.space_id.to_string(),
        )
        .unwrap_or_else(|error| panic!("invalid built-in capability resource: {error}"))])
        .unwrap_or_else(|error| panic!("invalid built-in capability scope: {error}"))
    }

    async fn execute(
        &self,
        context: AuthorizedCapabilityContext,
        input: Self::Input,
    ) -> Result<Self::Output, CapabilityExecutionError> {
        let principal = context.principal();
        let scope = current_scope(&self.pool, principal.tenant_id(), principal.user_id()).await?;
        let space =
            LearningOps::get_space(&self.pool, principal.tenant_id(), input.space_id, scope)
                .await
                .map_err(|_| dependency_failure("The E-learning space could not be loaded."))?
                .ok_or_else(|| invalid_state("The E-learning space is unavailable."))?;
        Ok(LearningSpaceOutput { space })
    }
}

async fn current_scope(
    pool: &PgPool,
    tenant_id: Uuid,
    user_id: Uuid,
) -> Result<LearningAccessScope, CapabilityExecutionError> {
    let user = UserOps::get_user_by_id(pool, tenant_id, user_id)
        .await
        .map_err(|_| dependency_failure("Current E-learning authority could not be loaded."))?
        .filter(|user| user.is_active)
        .ok_or_else(|| invalid_state("The current E-learning account is unavailable."))?;
    let access = AccessOps::effective_access(pool, tenant_id, &user.roles)
        .await
        .map_err(|_| dependency_failure("Current E-learning access could not be loaded."))?;
    if access
        .permissions
        .iter()
        .any(|permission| permission == "*")
    {
        return Ok(LearningAccessScope::Campus);
    }
    let family = RecordScopeFamilyKey::parse("learning.spaces")
        .map_err(|_| invalid_state("The E-learning record scope is invalid."))?;
    match access.record_scopes.effective_scope(&family) {
        Some(EffectiveRecordScope::Campus) => Ok(LearningAccessScope::Campus),
        Some(EffectiveRecordScope::Assigned) => Ok(LearningAccessScope::AssignedTo(user_id)),
        Some(EffectiveRecordScope::SelfRecord) => Ok(LearningAccessScope::SelfFor(user_id)),
        Some(EffectiveRecordScope::SelfAndAssigned) => {
            Ok(LearningAccessScope::SelfAndAssigned(user_id))
        }
        None => Err(invalid_state("E-learning record scope is unavailable.")),
    }
}

fn dependency_failure(message: &'static str) -> CapabilityExecutionError {
    CapabilityExecutionError::new(CapabilityExecutionErrorCode::DependencyUnavailable, message)
}

fn invalid_state(message: &'static str) -> CapabilityExecutionError {
    CapabilityExecutionError::new(CapabilityExecutionErrorCode::InvalidState, message)
}

#[cfg(test)]
mod tests {
    use cp_agent::Capability as _;
    use sqlx::postgres::PgPoolOptions;

    use super::{LearningSpaceCapability, LearningSpaceInput};

    #[tokio::test]
    async fn exact_space_read_declares_the_requested_resource() {
        let capability = LearningSpaceCapability::new(
            PgPoolOptions::new()
                .connect_lazy("postgresql://campus-pilot.invalid/campus_pilot")
                .unwrap_or_else(|_| unreachable!()),
        );
        let id = uuid::Uuid::new_v4();
        let scope = capability.scope(&LearningSpaceInput { space_id: id });
        let resource = scope.primary_resource().unwrap_or_else(|| unreachable!());
        assert_eq!(resource.kind(), "learning_space");
        assert_eq!(resource.id(), id.to_string());
    }
}

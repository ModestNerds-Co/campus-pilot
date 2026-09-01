//! Exposes Library read capabilities through the current Agent broker.

use async_trait::async_trait;
use cp_agent::{
    AuthorizedCapabilityContext, Capability, CapabilityDescriptor, CapabilityExecutionError,
    CapabilityExecutionErrorCode, CapabilityResource, CapabilityScope, DataSensitivity,
};
use cp_common::{EffectiveRecordScope, PaginationMeta, RecordScopeFamilyKey};
use cp_library::{
    BorrowingListQuery, DirectoryQuery, LibraryAccessScope, LibraryCatalogueOps,
    LibraryCirculationOps, LibraryFineOps, LibraryMemberOps, LibrarySettingsOps,
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
pub(super) struct LibraryListInput {
    page: Option<i64>,
    per_page: Option<i64>,
    search: Option<String>,
    status: Option<String>,
    overdue_only: Option<bool>,
    membership_id: Option<Uuid>,
}

#[derive(Debug, Clone, Copy)]
pub(super) enum LibraryListKind {
    Titles,
    Members,
    Loans,
    Holds,
    Fines,
}

impl LibraryListKind {
    pub(super) const fn operation_key(self) -> &'static str {
        match self {
            Self::Titles => "library.titles.list",
            Self::Members => "library.members.list",
            Self::Loans => "library.loans.list",
            Self::Holds => "library.holds.list",
            Self::Fines => "library.fines.list",
        }
    }
}

pub(super) struct LibraryListCapability {
    pool: PgPool,
    kind: LibraryListKind,
    descriptor: CapabilityDescriptor,
}

impl LibraryListCapability {
    pub(super) fn new(pool: PgPool, kind: LibraryListKind) -> Self {
        let (title, description, collection, sensitivity, resource) = match kind {
            LibraryListKind::Titles => (
                "List Library titles",
                "Returns the current catalogue with exact copy availability.",
                "titles",
                DataSensitivity::General,
                "library.titles",
            ),
            LibraryListKind::Members => (
                "List Library members",
                "Returns Library memberships within the authenticated borrower's record scope.",
                "memberships",
                DataSensitivity::Personal,
                "library.members",
            ),
            LibraryListKind::Loans => (
                "List Library loans",
                "Returns circulation records within the authenticated borrower's record scope.",
                "loans",
                DataSensitivity::Personal,
                "library.loans",
            ),
            LibraryListKind::Holds => (
                "List Library holds",
                "Returns reservation records within the authenticated borrower's record scope.",
                "holds",
                DataSensitivity::Personal,
                "library.holds",
            ),
            LibraryListKind::Fines => (
                "List Library fines",
                "Returns assessed fine evidence without payment credentials or balances.",
                "fines",
                DataSensitivity::Sensitive,
                "library.fines",
            ),
        };
        Self {
            pool,
            kind,
            descriptor: read_descriptor(
                kind.operation_key(),
                title,
                description,
                list_input_schema(kind),
                json!({ (collection): { "type": "array" }, "pagination": { "type": "object" } }),
                sensitivity,
                resource,
            ),
        }
    }
}

#[async_trait]
impl Capability for LibraryListCapability {
    type Input = LibraryListInput;
    type Output = Value;
    fn descriptor(&self) -> &CapabilityDescriptor {
        &self.descriptor
    }
    fn scope(&self, input: &Self::Input) -> CapabilityScope {
        input
            .membership_id
            .map_or(CapabilityScope::TenantWide, |id| {
                resource_scope("library_membership", id)
            })
    }
    async fn execute(
        &self,
        context: AuthorizedCapabilityContext,
        input: Self::Input,
    ) -> Result<Self::Output, CapabilityExecutionError> {
        let principal = context.principal();
        let page = input.page.unwrap_or(1).max(1);
        let per_page = input.per_page.unwrap_or(25).clamp(1, 100);
        match self.kind {
            LibraryListKind::Titles => {
                let query = DirectoryQuery {
                    page: Some(page),
                    per_page: Some(per_page),
                    search: input.search,
                    status: input.status,
                };
                let (titles, total) =
                    LibraryCatalogueOps::list_titles(&self.pool, principal.tenant_id(), &query)
                        .await
                        .map_err(|_| dependency_failure("Library titles could not be loaded."))?;
                Ok(
                    json!({ "titles": titles, "pagination": PaginationMeta::new(page as u32, per_page as u32, total) }),
                )
            }
            LibraryListKind::Members => {
                let scope = current_scope(
                    &self.pool,
                    principal.tenant_id(),
                    principal.user_id(),
                    "library.members",
                )
                .await?;
                let query = DirectoryQuery {
                    page: Some(page),
                    per_page: Some(per_page),
                    search: input.search,
                    status: input.status,
                };
                let (memberships, total) =
                    LibraryMemberOps::list(&self.pool, principal.tenant_id(), scope, &query)
                        .await
                        .map_err(|_| dependency_failure("Library members could not be loaded."))?;
                Ok(
                    json!({ "memberships": memberships, "pagination": PaginationMeta::new(page as u32, per_page as u32, total) }),
                )
            }
            kind => {
                let scope = current_scope(
                    &self.pool,
                    principal.tenant_id(),
                    principal.user_id(),
                    "library.borrowing",
                )
                .await?;
                let query = BorrowingListQuery {
                    page: Some(page),
                    per_page: Some(per_page),
                    search: input.search,
                    status: input.status,
                    overdue_only: input.overdue_only,
                    membership_id: input.membership_id,
                };
                let (records, total, collection) = match kind {
                    LibraryListKind::Loans => {
                        let (values, total) = LibraryCirculationOps::list_loans(
                            &self.pool,
                            principal.tenant_id(),
                            scope,
                            &query,
                        )
                        .await
                        .map_err(|_| dependency_failure("Library loans could not be loaded."))?;
                        (json!(values), total, "loans")
                    }
                    LibraryListKind::Holds => {
                        let (values, total) = LibraryCirculationOps::list_holds(
                            &self.pool,
                            principal.tenant_id(),
                            scope,
                            &query,
                        )
                        .await
                        .map_err(|_| dependency_failure("Library holds could not be loaded."))?;
                        (json!(values), total, "holds")
                    }
                    LibraryListKind::Fines => {
                        let (values, total) =
                            LibraryFineOps::list(&self.pool, principal.tenant_id(), scope, &query)
                                .await
                                .map_err(|_| {
                                    dependency_failure("Library fines could not be loaded.")
                                })?;
                        (json!(values), total, "fines")
                    }
                    LibraryListKind::Titles | LibraryListKind::Members => unreachable!(),
                };
                Ok(
                    json!({ (collection): records, "pagination": PaginationMeta::new(page as u32, per_page as u32, total) }),
                )
            }
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LibraryCopiesInput {
    title_id: Uuid,
    page: Option<i64>,
    per_page: Option<i64>,
    status: Option<String>,
}

pub(super) struct LibraryCopiesCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}
impl LibraryCopiesCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "library.copies.list",
                "List Library copies",
                "Returns physical copies for one catalogue title.",
                json!({ "title_id": uuid_schema(), "page": page_schema(), "per_page": per_page_schema(), "status": nullable_string_schema() }),
                json!({ "copies": { "type": "array" }, "pagination": { "type": "object" } }),
                DataSensitivity::General,
                "library.copies",
            ),
        }
    }
}
#[async_trait]
impl Capability for LibraryCopiesCapability {
    type Input = LibraryCopiesInput;
    type Output = Value;
    fn descriptor(&self) -> &CapabilityDescriptor {
        &self.descriptor
    }
    fn scope(&self, input: &Self::Input) -> CapabilityScope {
        resource_scope("library_title", input.title_id)
    }
    async fn execute(
        &self,
        context: AuthorizedCapabilityContext,
        input: Self::Input,
    ) -> Result<Self::Output, CapabilityExecutionError> {
        let page = input.page.unwrap_or(1).max(1);
        let per_page = input.per_page.unwrap_or(25).clamp(1, 100);
        let (copies, total) = LibraryCatalogueOps::list_copies(
            &self.pool,
            context.principal().tenant_id(),
            input.title_id,
            page,
            per_page,
            input.status.as_deref(),
        )
        .await
        .map_err(|_| dependency_failure("Library copies could not be loaded."))?;
        Ok(
            json!({ "copies": copies, "pagination": PaginationMeta::new(page as u32, per_page as u32, total) }),
        )
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LibraryReadInput {
    record_id: Uuid,
}

#[derive(Debug, Clone, Copy)]
pub(super) enum LibraryReadKind {
    Title,
    Copy,
    Member,
    Loan,
    Hold,
    Fine,
}
impl LibraryReadKind {
    pub(super) const fn operation_key(self) -> &'static str {
        match self {
            Self::Title => "library.titles.read",
            Self::Copy => "library.copies.read",
            Self::Member => "library.members.read",
            Self::Loan => "library.loans.read",
            Self::Hold => "library.holds.read",
            Self::Fine => "library.fines.read",
        }
    }
    const fn resource_kind(self) -> &'static str {
        match self {
            Self::Title => "library_title",
            Self::Copy => "library_copy",
            Self::Member => "library_membership",
            Self::Loan => "library_loan",
            Self::Hold => "library_hold",
            Self::Fine => "library_fine",
        }
    }
}

pub(super) struct LibraryReadCapability {
    pool: PgPool,
    kind: LibraryReadKind,
    descriptor: CapabilityDescriptor,
}
impl LibraryReadCapability {
    pub(super) fn new(pool: PgPool, kind: LibraryReadKind) -> Self {
        let (title, sensitivity) = match kind {
            LibraryReadKind::Title => ("Read Library title", DataSensitivity::General),
            LibraryReadKind::Copy => ("Read Library copy", DataSensitivity::General),
            LibraryReadKind::Member => ("Read Library member", DataSensitivity::Personal),
            LibraryReadKind::Loan => ("Read Library loan", DataSensitivity::Personal),
            LibraryReadKind::Hold => ("Read Library hold", DataSensitivity::Personal),
            LibraryReadKind::Fine => ("Read Library fine", DataSensitivity::Sensitive),
        };
        Self {
            pool,
            kind,
            descriptor: read_descriptor(
                kind.operation_key(),
                title,
                "Returns one current Library record within the authenticated account's scope.",
                json!({ "record_id": uuid_schema() }),
                json!({ "record": { "type": "object" } }),
                sensitivity,
                "library.records",
            ),
        }
    }
}
#[async_trait]
impl Capability for LibraryReadCapability {
    type Input = LibraryReadInput;
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
        let p = context.principal();
        let record = match self.kind {
            LibraryReadKind::Title => {
                LibraryCatalogueOps::get_title(&self.pool, p.tenant_id(), input.record_id)
                    .await
                    .map_err(|_| dependency_failure("The Library title could not be loaded."))?
                    .map(|value| json!(value))
            }
            LibraryReadKind::Copy => {
                LibraryCatalogueOps::get_copy(&self.pool, p.tenant_id(), input.record_id)
                    .await
                    .map_err(|_| dependency_failure("The Library copy could not be loaded."))?
                    .map(|value| json!(value))
            }
            LibraryReadKind::Member => {
                let scope =
                    current_scope(&self.pool, p.tenant_id(), p.user_id(), "library.members")
                        .await?;
                LibraryMemberOps::get(&self.pool, p.tenant_id(), input.record_id, scope)
                    .await
                    .map_err(|_| dependency_failure("The Library member could not be loaded."))?
                    .map(|value| json!(value))
            }
            LibraryReadKind::Loan => {
                let scope =
                    current_scope(&self.pool, p.tenant_id(), p.user_id(), "library.borrowing")
                        .await?;
                LibraryCirculationOps::get_loan(&self.pool, p.tenant_id(), input.record_id, scope)
                    .await
                    .map_err(|_| dependency_failure("The Library loan could not be loaded."))?
                    .map(|value| json!(value))
            }
            LibraryReadKind::Hold => {
                let scope =
                    current_scope(&self.pool, p.tenant_id(), p.user_id(), "library.borrowing")
                        .await?;
                LibraryCirculationOps::get_hold(&self.pool, p.tenant_id(), input.record_id, scope)
                    .await
                    .map_err(|_| dependency_failure("The Library hold could not be loaded."))?
                    .map(|value| json!(value))
            }
            LibraryReadKind::Fine => {
                let scope =
                    current_scope(&self.pool, p.tenant_id(), p.user_id(), "library.borrowing")
                        .await?;
                LibraryFineOps::get(&self.pool, p.tenant_id(), input.record_id, scope)
                    .await
                    .map_err(|_| dependency_failure("The Library fine could not be loaded."))?
                    .map(|value| json!(value))
            }
        }
        .ok_or_else(|| invalid_state("The Library record was not found."))?;
        Ok(json!({ "record": record }))
    }
}

pub(super) struct LibrarySettingsCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}
impl LibrarySettingsCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "library.settings.read",
                "Read Library settings",
                "Returns accession, lending, renewal, and fine policy without credentials.",
                json!({}),
                json!({ "settings": { "type": "object" } }),
                DataSensitivity::General,
                "library.settings",
            ),
        }
    }
}
#[async_trait]
impl Capability for LibrarySettingsCapability {
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
        let p = context.principal();
        require_campus_scope(&self.pool, p.tenant_id(), p.user_id(), "library.members").await?;
        let settings = LibrarySettingsOps::get(&self.pool, p.tenant_id())
            .await
            .map_err(|_| dependency_failure("Library settings could not be loaded."))?;
        Ok(json!({ "settings": settings }))
    }
}

pub(super) struct LibraryReferencesCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}
impl LibraryReferencesCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "library.references.read",
                "Read Library references",
                "Returns current SIS and HR borrower candidates and active currencies for Library administration.",
                json!({}),
                json!({ "references": { "type": "object" } }),
                DataSensitivity::Personal,
                "library.references",
            ),
        }
    }
}
#[async_trait]
impl Capability for LibraryReferencesCapability {
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
        let p = context.principal();
        require_campus_scope(&self.pool, p.tenant_id(), p.user_id(), "library.members").await?;
        let references = LibraryMemberOps::reference_data(&self.pool, p.tenant_id(), None)
            .await
            .map_err(|_| dependency_failure("Library references could not be loaded."))?;
        Ok(json!({ "references": references }))
    }
}

async fn current_scope(
    pool: &PgPool,
    tenant_id: Uuid,
    user_id: Uuid,
    family: &str,
) -> Result<LibraryAccessScope, CapabilityExecutionError> {
    let roles = sqlx::query_scalar::<_, Vec<String>>("SELECT roles FROM users WHERE tenant_id = $1 AND id = $2 AND is_active AND deleted_at IS NULL").bind(tenant_id).bind(user_id).fetch_optional(pool).await.map_err(|_| dependency_failure("Library authority could not be loaded."))?.ok_or_else(|| invalid_state("The Library account is unavailable."))?;
    let wildcard = sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM roles WHERE tenant_id = $1 AND key = ANY($2) AND deleted_at IS NULL AND '*' = ANY(permissions))").bind(tenant_id).bind(&roles).fetch_one(pool).await.map_err(|_| dependency_failure("Library authority could not be loaded."))?;
    if wildcard {
        return Ok(LibraryAccessScope::Campus);
    }
    let grants = RoleRecordScopeOps::effective_for_roles(pool, tenant_id, &roles)
        .await
        .map_err(|_| dependency_failure("Library authority could not be loaded."))?;
    let family = RecordScopeFamilyKey::parse(family)
        .map_err(|_| dependency_failure("Library authority could not be loaded."))?;
    match grants.effective_scope(&family) {
        Some(EffectiveRecordScope::Campus) => Ok(LibraryAccessScope::Campus),
        Some(
            EffectiveRecordScope::SelfRecord
            | EffectiveRecordScope::Assigned
            | EffectiveRecordScope::SelfAndAssigned,
        ) => Ok(LibraryAccessScope::SelfFor(user_id)),
        None => Err(invalid_state("Library record scope is unavailable.")),
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
        LibraryAccessScope::Campus
    ) {
        Ok(())
    } else {
        Err(invalid_state("Campus-wide Library scope is required."))
    }
}

fn list_input_schema(kind: LibraryListKind) -> Value {
    let mut properties = serde_json::Map::from_iter([
        ("page".to_string(), page_schema()),
        ("per_page".to_string(), per_page_schema()),
        ("search".to_string(), nullable_string_schema()),
        ("status".to_string(), nullable_string_schema()),
    ]);
    if matches!(kind, LibraryListKind::Loans) {
        properties.insert(
            "overdue_only".to_string(),
            json!({ "type": ["boolean", "null"] }),
        );
    }
    if !matches!(kind, LibraryListKind::Titles | LibraryListKind::Members) {
        properties.insert(
            "membership_id".to_string(),
            json!({ "type": ["string", "null"], "format": "uuid" }),
        );
    }
    Value::Object(properties)
}
fn resource_scope(kind: &str, id: Uuid) -> CapabilityScope {
    CapabilityScope::resources([
        CapabilityResource::parse(kind, id.to_string()).unwrap_or_else(|_| unreachable!())
    ])
    .unwrap_or_else(|_| unreachable!())
}
fn uuid_schema() -> Value {
    json!({ "type": "string", "format": "uuid" })
}
fn page_schema() -> Value {
    json!({ "type": ["integer", "null"], "minimum": 1 })
}
fn per_page_schema() -> Value {
    json!({ "type": ["integer", "null"], "minimum": 1, "maximum": 100 })
}
fn nullable_string_schema() -> Value {
    json!({ "type": ["string", "null"], "maxLength": 180 })
}
fn dependency_failure(message: &'static str) -> CapabilityExecutionError {
    CapabilityExecutionError::new(CapabilityExecutionErrorCode::DependencyUnavailable, message)
}
fn invalid_state(message: &'static str) -> CapabilityExecutionError {
    CapabilityExecutionError::new(CapabilityExecutionErrorCode::InvalidState, message)
}

#[cfg(test)]
mod tests {
    use super::{LibraryListKind, LibraryReadKind};
    #[test]
    fn every_library_read_kind_has_a_stable_operation_key() {
        assert_eq!(LibraryListKind::Fines.operation_key(), "library.fines.list");
        assert_eq!(LibraryReadKind::Fine.operation_key(), "library.fines.read");
    }
}

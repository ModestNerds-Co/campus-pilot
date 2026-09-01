//! Exposes authorized Document Registry metadata to the Agent broker.
//!
//! Raw private document bytes and download URLs are intentionally absent.

use async_trait::async_trait;
use cp_agent::{
    AuthorizedCapabilityContext, Capability, CapabilityDescriptor, CapabilityExecutionError,
    CapabilityExecutionErrorCode, CapabilityResource, CapabilityScope, DataSensitivity,
};
use cp_common::{EffectiveRecordScope, RecordScopeFamilyKey};
use cp_document_registry::{DocumentRegistryOps, RegistryListQuery};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

use super::administration::read_descriptor;
use crate::services::access::record_scopes::RoleRecordScopeOps;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RegistryReadInput {
    record_id: Option<Uuid>,
    page: Option<i64>,
    per_page: Option<i64>,
    search: Option<String>,
    status: Option<String>,
    series_id: Option<Uuid>,
    sensitivity: Option<String>,
    file_id: Option<Uuid>,
}

#[derive(Debug, Clone, Copy)]
pub(super) enum RegistryReadKind {
    NumberingPolicy,
    SeriesList,
    SeriesRead,
    FilesList,
    FileRead,
    FileActivity,
    RetentionDue,
    ReviewsList,
    ReviewRead,
    LegalHoldsList,
    LegalHoldRead,
}

impl RegistryReadKind {
    pub(super) const fn operation_key(self) -> &'static str {
        match self {
            Self::NumberingPolicy => "document_registry.numbering_policy.read",
            Self::SeriesList => "document_registry.series.list",
            Self::SeriesRead => "document_registry.series.read",
            Self::FilesList => "document_registry.files.list",
            Self::FileRead => "document_registry.files.read",
            Self::FileActivity => "document_registry.files.activity.list",
            Self::RetentionDue => "document_registry.retention_due.list",
            Self::ReviewsList => "document_registry.disposition_reviews.list",
            Self::ReviewRead => "document_registry.disposition_reviews.read",
            Self::LegalHoldsList => "document_registry.legal_holds.list",
            Self::LegalHoldRead => "document_registry.legal_holds.read",
        }
    }
    const fn title(self) -> &'static str {
        match self {
            Self::NumberingPolicy => "Read document numbering policy",
            Self::SeriesList => "List document classifications",
            Self::SeriesRead => "Read document classification",
            Self::FilesList => "List registered documents",
            Self::FileRead => "Read registered document metadata",
            Self::FileActivity => "Read registered document activity",
            Self::RetentionDue => "List retention-due documents",
            Self::ReviewsList => "List document disposition reviews",
            Self::ReviewRead => "Read document disposition review",
            Self::LegalHoldsList => "List document legal holds",
            Self::LegalHoldRead => "Read document legal hold",
        }
    }
    const fn resource_kind(self) -> Option<&'static str> {
        match self {
            Self::SeriesRead => Some("document_registry_series"),
            Self::FileRead | Self::FileActivity => Some("document_registry_file"),
            Self::ReviewRead => Some("document_registry_disposition_review"),
            Self::LegalHoldRead => Some("document_registry_legal_hold"),
            _ => None,
        }
    }
    const fn sensitivity(self) -> DataSensitivity {
        match self {
            Self::NumberingPolicy | Self::SeriesList | Self::SeriesRead => DataSensitivity::General,
            _ => DataSensitivity::Sensitive,
        }
    }
}

pub(super) struct RegistryReadCapability {
    pool: PgPool,
    kind: RegistryReadKind,
    descriptor: CapabilityDescriptor,
}

impl RegistryReadCapability {
    pub(super) fn new(pool: PgPool, kind: RegistryReadKind) -> Self {
        Self {
            pool,
            kind,
            descriptor: read_descriptor(
                kind.operation_key(),
                kind.title(),
                "Returns authorized Document Registry metadata without exposing private document bytes or download links.",
                json!({
                    "record_id": {"type":["string","null"],"format":"uuid"},
                    "page": {"type":["integer","null"],"minimum":1},
                    "per_page": {"type":["integer","null"],"minimum":1,"maximum":100},
                    "search": {"type":["string","null"],"maxLength":240},
                    "status": {"type":["string","null"],"maxLength":40},
                    "series_id": {"type":["string","null"],"format":"uuid"},
                    "sensitivity": {"type":["string","null"],"maxLength":40},
                    "file_id": {"type":["string","null"],"format":"uuid"}
                }),
                json!({"result":{"type":"object"}}),
                kind.sensitivity(),
                "document_registry.records",
            ),
        }
    }
}

#[async_trait]
impl Capability for RegistryReadCapability {
    type Input = RegistryReadInput;
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
        require_campus_scope(&self.pool, principal.tenant_id(), principal.user_id()).await?;
        let restricted =
            can_view_restricted(&self.pool, principal.tenant_id(), principal.user_id()).await?;
        let query = RegistryListQuery {
            page: input.page,
            per_page: input.per_page,
            search: input.search,
            status: input.status,
            series_id: input.series_id,
            sensitivity: input.sensitivity,
            file_id: input.file_id,
        };
        let result = match self.kind {
            RegistryReadKind::NumberingPolicy => json!(
                DocumentRegistryOps::numbering_policy(&self.pool, principal.tenant_id())
                    .await
                    .map_err(|_| dependency_failure())?
            ),
            RegistryReadKind::SeriesList => {
                let (values, total) = DocumentRegistryOps::list_series(
                    &self.pool,
                    principal.tenant_id(),
                    &query,
                    restricted,
                )
                .await
                .map_err(|_| dependency_failure())?;
                json!({"series":values,"total":total})
            }
            RegistryReadKind::SeriesRead => json!(
                DocumentRegistryOps::get_series(
                    &self.pool,
                    principal.tenant_id(),
                    required_id(input.record_id)?,
                    restricted,
                )
                .await
                .map_err(|_| dependency_failure())?
                .ok_or_else(not_found)?
            ),
            RegistryReadKind::FilesList => {
                let (values, total) = DocumentRegistryOps::list_files(
                    &self.pool,
                    principal.tenant_id(),
                    &query,
                    restricted,
                )
                .await
                .map_err(|_| dependency_failure())?;
                json!({"files":values,"total":total})
            }
            RegistryReadKind::FileRead => json!(
                DocumentRegistryOps::get_file(
                    &self.pool,
                    principal.tenant_id(),
                    required_id(input.record_id)?,
                    restricted
                )
                .await
                .map_err(|_| dependency_failure())?
                .ok_or_else(not_found)?
            ),
            RegistryReadKind::FileActivity => {
                json!({"activity":DocumentRegistryOps::activity(&self.pool,principal.tenant_id(),required_id(input.record_id)?,restricted).await.map_err(|_|dependency_failure())?})
            }
            RegistryReadKind::RetentionDue => {
                json!({"files":DocumentRegistryOps::retention_due(&self.pool,principal.tenant_id(),restricted).await.map_err(|_|dependency_failure())?})
            }
            RegistryReadKind::ReviewsList => {
                let (values, total) = DocumentRegistryOps::list_reviews(
                    &self.pool,
                    principal.tenant_id(),
                    &query,
                    restricted,
                )
                .await
                .map_err(|_| dependency_failure())?;
                json!({"reviews":values,"total":total})
            }
            RegistryReadKind::ReviewRead => json!(
                DocumentRegistryOps::get_review(
                    &self.pool,
                    principal.tenant_id(),
                    required_id(input.record_id)?,
                    restricted
                )
                .await
                .map_err(|_| dependency_failure())?
                .ok_or_else(not_found)?
            ),
            RegistryReadKind::LegalHoldsList => {
                let (values, total) = DocumentRegistryOps::list_legal_holds(
                    &self.pool,
                    principal.tenant_id(),
                    &query,
                    restricted,
                )
                .await
                .map_err(|_| dependency_failure())?;
                json!({"legal_holds":values,"total":total})
            }
            RegistryReadKind::LegalHoldRead => json!(
                DocumentRegistryOps::get_legal_hold(
                    &self.pool,
                    principal.tenant_id(),
                    required_id(input.record_id)?,
                    restricted
                )
                .await
                .map_err(|_| dependency_failure())?
                .ok_or_else(not_found)?
            ),
        };
        Ok(json!({"result":result}))
    }
}

async fn require_campus_scope(
    pool: &PgPool,
    tenant_id: Uuid,
    user_id: Uuid,
) -> Result<(), CapabilityExecutionError> {
    let roles = roles(pool, tenant_id, user_id).await?;
    let grants = RoleRecordScopeOps::effective_for_roles(pool, tenant_id, &roles)
        .await
        .map_err(|_| dependency_failure())?;
    let family =
        RecordScopeFamilyKey::parse("document_registry.records").map_err(|_| invalid_state())?;
    if matches!(
        grants.effective_scope(&family),
        Some(EffectiveRecordScope::Campus)
    ) {
        Ok(())
    } else {
        Err(invalid_state())
    }
}
async fn can_view_restricted(
    pool: &PgPool,
    tenant_id: Uuid,
    user_id: Uuid,
) -> Result<bool, CapabilityExecutionError> {
    let roles = roles(pool, tenant_id, user_id).await?;
    sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM roles WHERE tenant_id=$1 AND key=ANY($2) AND deleted_at IS NULL AND ('*'=ANY(permissions) OR 'document_registry:restricted'=ANY(permissions)))")
        .bind(tenant_id).bind(roles).fetch_one(pool).await.map_err(|_|dependency_failure())
}
async fn roles(
    pool: &PgPool,
    tenant_id: Uuid,
    user_id: Uuid,
) -> Result<Vec<String>, CapabilityExecutionError> {
    sqlx::query_scalar(
        "SELECT roles FROM users WHERE tenant_id=$1 AND id=$2 AND is_active AND deleted_at IS NULL",
    )
    .bind(tenant_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .map_err(|_| dependency_failure())?
    .ok_or_else(invalid_state)
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
        "Document Registry metadata could not be loaded.",
    )
}
fn invalid_state() -> CapabilityExecutionError {
    CapabilityExecutionError::new(
        CapabilityExecutionErrorCode::InvalidState,
        "Document Registry access or input is invalid.",
    )
}
fn not_found() -> CapabilityExecutionError {
    CapabilityExecutionError::new(
        CapabilityExecutionErrorCode::InvalidState,
        "The Document Registry record was not found.",
    )
}

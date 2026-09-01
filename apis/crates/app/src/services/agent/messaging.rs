//! Exposes scoped Communication reads through the Agent broker.

use async_trait::async_trait;
use cp_agent::{
    AuthorizedCapabilityContext, Capability, CapabilityDescriptor, CapabilityExecutionError,
    CapabilityExecutionErrorCode, CapabilityResource, CapabilityScope, DataSensitivity,
};
use cp_common::{EffectiveRecordScope, PaginationMeta, RecordScopeFamilyKey};
use cp_messaging::{
    AnnouncementDetail, AnnouncementListQuery, AnnouncementStatus, AnnouncementSummary,
    AudiencePreview, CommunicationAccessScope, CommunicationOps, CommunicationReferenceData,
    DeliveryRecord, InboxItem, InboxListQuery,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use super::administration::read_descriptor;
use crate::services::access::record_scopes::RoleRecordScopeOps;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct EmptyInput {}

async fn current_scope(
    pool: &PgPool,
    tenant_id: Uuid,
    user_id: Uuid,
) -> Result<CommunicationAccessScope, CapabilityExecutionError> {
    let roles = sqlx::query_scalar::<_, Vec<String>>("SELECT roles FROM users WHERE tenant_id = $1 AND id = $2 AND is_active AND deleted_at IS NULL")
        .bind(tenant_id).bind(user_id).fetch_optional(pool).await.map_err(|_| dependency_failure("Communication authority could not be loaded."))?
        .ok_or_else(|| invalid_state("The communication account is unavailable."))?;
    let wildcard = sqlx::query_scalar::<_, bool>(
        r#"SELECT EXISTS(
        SELECT 1 FROM roles WHERE tenant_id = $1 AND key = ANY($2)
          AND deleted_at IS NULL AND '*' = ANY(permissions))"#,
    )
    .bind(tenant_id)
    .bind(&roles)
    .fetch_one(pool)
    .await
    .map_err(|_| dependency_failure("Communication authority could not be loaded."))?;
    if wildcard {
        return Ok(CommunicationAccessScope::Campus);
    }
    let grants = RoleRecordScopeOps::effective_for_roles(pool, tenant_id, &roles)
        .await
        .map_err(|_| dependency_failure("Communication authority could not be loaded."))?;
    let family = RecordScopeFamilyKey::parse("messaging.announcements")
        .map_err(|_| dependency_failure("Communication authority could not be loaded."))?;
    match grants.effective_scope(&family) {
        Some(EffectiveRecordScope::Campus) => Ok(CommunicationAccessScope::Campus),
        Some(EffectiveRecordScope::Assigned | EffectiveRecordScope::SelfAndAssigned) => {
            Ok(CommunicationAccessScope::AssignedTo(user_id))
        }
        Some(EffectiveRecordScope::SelfRecord) => Ok(CommunicationAccessScope::SelfFor(user_id)),
        None => Err(invalid_state("Communication record scope is unavailable.")),
    }
}

#[derive(Serialize)]
pub(super) struct ReferencesOutput {
    references: CommunicationReferenceData,
}
pub(super) struct CommunicationReferencesCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}
impl CommunicationReferencesCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "messaging.references.read",
                "Read communication references",
                "Returns the current audiences available to the authenticated account.",
                json!({}),
                json!({"references":{"type":"object"}}),
                DataSensitivity::Personal,
                "messaging.references",
            ),
        }
    }
}
#[async_trait]
impl Capability for CommunicationReferencesCapability {
    type Input = EmptyInput;
    type Output = ReferencesOutput;
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
        let scope = current_scope(&self.pool, principal.tenant_id(), principal.user_id()).await?;
        let references = CommunicationOps::reference_data(&self.pool, principal.tenant_id(), scope)
            .await
            .map_err(|_| dependency_failure("Communication references could not be loaded."))?;
        Ok(ReferencesOutput { references })
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ListAnnouncementsInput {
    page: Option<i64>,
    per_page: Option<i64>,
    status: Option<AnnouncementStatus>,
    search: Option<String>,
}
#[derive(Serialize)]
pub(super) struct ListAnnouncementsOutput {
    announcements: Vec<AnnouncementSummary>,
    pagination: PaginationMeta,
}
pub(super) struct CommunicationAnnouncementsListCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}
impl CommunicationAnnouncementsListCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "messaging.announcements.list",
                "List announcements",
                "Returns communication drafts and publications within the authenticated account's record scope.",
                json!({"page":{"type":["integer","null"],"minimum":1},"per_page":{"type":["integer","null"],"minimum":1,"maximum":100},"status":{"type":["string","null"],"enum":["draft","submitted","published","cancelled",null]},"search":{"type":["string","null"],"maxLength":180}}),
                json!({"announcements":{"type":"array"},"pagination":{"type":"object"}}),
                DataSensitivity::Personal,
                "messaging.announcements",
            ),
        }
    }
}
#[async_trait]
impl Capability for CommunicationAnnouncementsListCapability {
    type Input = ListAnnouncementsInput;
    type Output = ListAnnouncementsOutput;
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
        let scope = current_scope(&self.pool, principal.tenant_id(), principal.user_id()).await?;
        let page = input.page.unwrap_or(1).max(1);
        let per_page = input.per_page.unwrap_or(25).clamp(1, 100);
        let query = AnnouncementListQuery {
            page: Some(page),
            per_page: Some(per_page),
            status: input.status,
            search: input.search,
        };
        let (announcements, total) =
            CommunicationOps::list(&self.pool, principal.tenant_id(), scope, &query)
                .await
                .map_err(|_| dependency_failure("Announcements could not be loaded."))?;
        Ok(ListAnnouncementsOutput {
            announcements,
            pagination: PaginationMeta::new(page as u32, per_page as u32, total),
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct AnnouncementInput {
    announcement_id: Uuid,
}
#[derive(Serialize)]
pub(super) struct AnnouncementOutput {
    announcement: AnnouncementDetail,
}
pub(super) struct CommunicationAnnouncementReadCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}
impl CommunicationAnnouncementReadCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "messaging.announcements.read",
                "Read announcement",
                "Returns one scoped announcement with its reviewed audiences and lifecycle state.",
                json!({"announcement_id":{"type":"string","format":"uuid"}}),
                json!({"announcement":{"type":"object"}}),
                DataSensitivity::Personal,
                "messaging.announcements",
            ),
        }
    }
}
#[async_trait]
impl Capability for CommunicationAnnouncementReadCapability {
    type Input = AnnouncementInput;
    type Output = AnnouncementOutput;
    fn descriptor(&self) -> &CapabilityDescriptor {
        &self.descriptor
    }
    fn scope(&self, input: &Self::Input) -> CapabilityScope {
        resource_scope("communication_announcement", input.announcement_id)
    }
    async fn execute(
        &self,
        context: AuthorizedCapabilityContext,
        input: Self::Input,
    ) -> Result<Self::Output, CapabilityExecutionError> {
        let p = context.principal();
        let scope = current_scope(&self.pool, p.tenant_id(), p.user_id()).await?;
        let announcement =
            CommunicationOps::get(&self.pool, p.tenant_id(), input.announcement_id, scope)
                .await
                .map_err(|_| dependency_failure("The announcement could not be loaded."))?
                .ok_or_else(|| invalid_state("The announcement was not found."))?;
        Ok(AnnouncementOutput { announcement })
    }
}

#[derive(Serialize)]
pub(super) struct AudiencePreviewOutput {
    preview: AudiencePreview,
}
pub(super) struct CommunicationAudiencePreviewCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}
impl CommunicationAudiencePreviewCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "messaging.announcements.audience_preview.read",
                "Preview announcement audience",
                "Returns the current or frozen recipient preview without publishing.",
                json!({"announcement_id":{"type":"string","format":"uuid"}}),
                json!({"preview":{"type":"object"}}),
                DataSensitivity::Personal,
                "messaging.announcements",
            ),
        }
    }
}
#[async_trait]
impl Capability for CommunicationAudiencePreviewCapability {
    type Input = AnnouncementInput;
    type Output = AudiencePreviewOutput;
    fn descriptor(&self) -> &CapabilityDescriptor {
        &self.descriptor
    }
    fn scope(&self, input: &Self::Input) -> CapabilityScope {
        resource_scope("communication_announcement", input.announcement_id)
    }
    async fn execute(
        &self,
        context: AuthorizedCapabilityContext,
        input: Self::Input,
    ) -> Result<Self::Output, CapabilityExecutionError> {
        let p = context.principal();
        let scope = current_scope(&self.pool, p.tenant_id(), p.user_id()).await?;
        let preview = CommunicationOps::audience_preview(
            &self.pool,
            p.tenant_id(),
            input.announcement_id,
            scope,
        )
        .await
        .map_err(|_| dependency_failure("The audience preview could not be loaded."))?
        .ok_or_else(|| invalid_state("The announcement was not found."))?;
        Ok(AudiencePreviewOutput { preview })
    }
}

#[derive(Serialize)]
pub(super) struct DeliveriesOutput {
    deliveries: Vec<DeliveryRecord>,
}
pub(super) struct CommunicationDeliveriesCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}
impl CommunicationDeliveriesCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "messaging.deliveries.list",
                "List announcement deliveries",
                "Returns in-app delivery and read state for one announcement.",
                json!({"announcement_id":{"type":"string","format":"uuid"}}),
                json!({"deliveries":{"type":"array"}}),
                DataSensitivity::Personal,
                "messaging.deliveries",
            ),
        }
    }
}
#[async_trait]
impl Capability for CommunicationDeliveriesCapability {
    type Input = AnnouncementInput;
    type Output = DeliveriesOutput;
    fn descriptor(&self) -> &CapabilityDescriptor {
        &self.descriptor
    }
    fn scope(&self, input: &Self::Input) -> CapabilityScope {
        resource_scope("communication_announcement", input.announcement_id)
    }
    async fn execute(
        &self,
        context: AuthorizedCapabilityContext,
        input: Self::Input,
    ) -> Result<Self::Output, CapabilityExecutionError> {
        let p = context.principal();
        if !matches!(
            current_scope(&self.pool, p.tenant_id(), p.user_id()).await?,
            CommunicationAccessScope::Campus
        ) {
            return Err(invalid_state(
                "Delivery history requires campus communication scope.",
            ));
        }
        let deliveries =
            CommunicationOps::deliveries(&self.pool, p.tenant_id(), input.announcement_id)
                .await
                .map_err(|_| dependency_failure("Delivery history could not be loaded."))?;
        Ok(DeliveriesOutput { deliveries })
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct InboxListInput {
    page: Option<i64>,
    per_page: Option<i64>,
    unread_only: Option<bool>,
}
#[derive(Serialize)]
pub(super) struct InboxListOutput {
    messages: Vec<InboxItem>,
    pagination: PaginationMeta,
}
pub(super) struct CommunicationInboxListCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}
impl CommunicationInboxListCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "messaging.inbox.list",
                "List personal inbox",
                "Returns only the authenticated account's delivered communication.",
                json!({"page":{"type":["integer","null"],"minimum":1},"per_page":{"type":["integer","null"],"minimum":1,"maximum":100},"unread_only":{"type":["boolean","null"]}}),
                json!({"messages":{"type":"array"},"pagination":{"type":"object"}}),
                DataSensitivity::Personal,
                "messaging.inbox",
            ),
        }
    }
}
#[async_trait]
impl Capability for CommunicationInboxListCapability {
    type Input = InboxListInput;
    type Output = InboxListOutput;
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
        let p = context.principal();
        let page = input.page.unwrap_or(1).max(1);
        let per_page = input.per_page.unwrap_or(25).clamp(1, 100);
        let query = InboxListQuery {
            page: Some(page),
            per_page: Some(per_page),
            unread_only: input.unread_only,
        };
        let (messages, total) =
            CommunicationOps::inbox(&self.pool, p.tenant_id(), p.user_id(), &query)
                .await
                .map_err(|_| dependency_failure("The inbox could not be loaded."))?;
        Ok(InboxListOutput {
            messages,
            pagination: PaginationMeta::new(page as u32, per_page as u32, total),
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct InboxMessageInput {
    delivery_id: Uuid,
}
#[derive(Serialize)]
pub(super) struct InboxMessageOutput {
    message: InboxItem,
}
pub(super) struct CommunicationInboxReadCapability {
    pool: PgPool,
    descriptor: CapabilityDescriptor,
}
impl CommunicationInboxReadCapability {
    pub(super) fn new(pool: PgPool) -> Self {
        Self {
            pool,
            descriptor: read_descriptor(
                "messaging.inbox.read",
                "Read personal inbox message",
                "Returns one delivered message only when it belongs to the authenticated account.",
                json!({"delivery_id":{"type":"string","format":"uuid"}}),
                json!({"message":{"type":"object"}}),
                DataSensitivity::Personal,
                "messaging.inbox",
            ),
        }
    }
}
#[async_trait]
impl Capability for CommunicationInboxReadCapability {
    type Input = InboxMessageInput;
    type Output = InboxMessageOutput;
    fn descriptor(&self) -> &CapabilityDescriptor {
        &self.descriptor
    }
    fn scope(&self, input: &Self::Input) -> CapabilityScope {
        resource_scope("communication_delivery", input.delivery_id)
    }
    async fn execute(
        &self,
        context: AuthorizedCapabilityContext,
        input: Self::Input,
    ) -> Result<Self::Output, CapabilityExecutionError> {
        let p = context.principal();
        let message = CommunicationOps::inbox_message(
            &self.pool,
            p.tenant_id(),
            p.user_id(),
            input.delivery_id,
        )
        .await
        .map_err(|_| dependency_failure("The inbox message could not be loaded."))?
        .ok_or_else(|| invalid_state("The inbox message was not found."))?;
        Ok(InboxMessageOutput { message })
    }
}

fn resource_scope(kind: &str, id: Uuid) -> CapabilityScope {
    CapabilityScope::resources([CapabilityResource::parse(kind, id.to_string())
        .unwrap_or_else(|error| panic!("invalid built-in communication resource: {error}"))])
    .unwrap_or_else(|error| panic!("invalid built-in communication scope: {error}"))
}
fn dependency_failure(message: &'static str) -> CapabilityExecutionError {
    CapabilityExecutionError::new(CapabilityExecutionErrorCode::DependencyUnavailable, message)
}
fn invalid_state(message: &'static str) -> CapabilityExecutionError {
    CapabilityExecutionError::new(CapabilityExecutionErrorCode::InvalidState, message)
}

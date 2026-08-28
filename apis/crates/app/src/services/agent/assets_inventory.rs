//! Agent read adapters for Assets and inventory item and store catalogues.
//!
//! These handlers call the module-owned operations and expose reduced records;
//! internal actor and idempotency identifiers never enter model-visible data.

use async_trait::async_trait;
use cp_agent::{
    AuthorizedCapabilityContext, Capability, CapabilityDescriptor, CapabilityExecutionError,
    CapabilityExecutionErrorCode, CapabilityResource, CapabilityScope, DataSensitivity,
};
use cp_assets_inventory::{AssetStatus, ItemOps, ItemResponse, StoreOps, StoreResponse};
use cp_common::PaginationMeta;
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

use super::administration::read_descriptor;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct AssetsInventoryListInput {
    page: Option<i64>,
    per_page: Option<i64>,
    search: Option<String>,
    status: Option<AssetStatus>,
}

#[derive(Debug, Clone, Copy)]
pub(super) enum AssetsInventoryListKind {
    Items,
    Stores,
}

impl AssetsInventoryListKind {
    pub(super) const fn operation_key(self) -> &'static str {
        match self {
            Self::Items => "assets_inventory.items.list",
            Self::Stores => "assets_inventory.stores.list",
        }
    }
}

pub(super) struct AssetsInventoryListCapability {
    pool: PgPool,
    kind: AssetsInventoryListKind,
    descriptor: CapabilityDescriptor,
}

impl AssetsInventoryListCapability {
    pub(super) fn new(pool: PgPool, kind: AssetsInventoryListKind) -> Self {
        let (title, description, collection, resource) = match kind {
            AssetsInventoryListKind::Items => (
                "List inventory items",
                "Returns bounded item catalogue records without internal actor or idempotency identifiers.",
                "items",
                "assets_inventory.items",
            ),
            AssetsInventoryListKind::Stores => (
                "List inventory stores",
                "Returns bounded store catalogue records without internal actor or idempotency identifiers.",
                "stores",
                "assets_inventory.stores",
            ),
        };
        let output_schema = if collection == "items" {
            json!({
                "items": { "type": "array" },
                "pagination": { "type": "object" }
            })
        } else {
            json!({
                "stores": { "type": "array" },
                "pagination": { "type": "object" }
            })
        };
        Self {
            pool,
            kind,
            descriptor: read_descriptor(
                kind.operation_key(),
                title,
                description,
                json!({
                    "page": page_schema(),
                    "per_page": per_page_schema(),
                    "search": search_schema(),
                    "status": {
                        "type": ["string", "null"],
                        "enum": ["active", "inactive", null]
                    }
                }),
                output_schema,
                DataSensitivity::General,
                resource,
            ),
        }
    }
}

#[async_trait]
impl Capability for AssetsInventoryListCapability {
    type Input = AssetsInventoryListInput;
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
        let (page, per_page) = bounded_page(input.page, input.per_page);
        let tenant_id = context.principal().tenant_id();
        match self.kind {
            AssetsInventoryListKind::Items => {
                let (items, total) = ItemOps::list(
                    &self.pool,
                    tenant_id,
                    page,
                    per_page,
                    trimmed(input.search.as_deref()),
                    input.status.map(AssetStatus::as_str),
                )
                .await
                .map_err(|_| dependency_failure("Inventory items could not be loaded."))?;
                Ok(json!({
                    "items": items.iter().map(item_projection).collect::<Vec<_>>(),
                    "pagination": PaginationMeta::new(page as u32, per_page as u32, total)
                }))
            }
            AssetsInventoryListKind::Stores => {
                let (stores, total) = StoreOps::list(
                    &self.pool,
                    tenant_id,
                    page,
                    per_page,
                    trimmed(input.search.as_deref()),
                    input.status.map(AssetStatus::as_str),
                )
                .await
                .map_err(|_| dependency_failure("Inventory stores could not be loaded."))?;
                Ok(json!({
                    "stores": stores.iter().map(store_projection).collect::<Vec<_>>(),
                    "pagination": PaginationMeta::new(page as u32, per_page as u32, total)
                }))
            }
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct AssetsInventoryRecordInput {
    record_id: Uuid,
}

#[derive(Debug, Clone, Copy)]
pub(super) enum AssetsInventoryReadKind {
    Item,
    Store,
}

impl AssetsInventoryReadKind {
    pub(super) const fn operation_key(self) -> &'static str {
        match self {
            Self::Item => "assets_inventory.items.read",
            Self::Store => "assets_inventory.stores.read",
        }
    }
}

pub(super) struct AssetsInventoryReadCapability {
    pool: PgPool,
    kind: AssetsInventoryReadKind,
    descriptor: CapabilityDescriptor,
}

impl AssetsInventoryReadCapability {
    pub(super) fn new(pool: PgPool, kind: AssetsInventoryReadKind) -> Self {
        let (title, description, resource) = match kind {
            AssetsInventoryReadKind::Item => (
                "Read inventory item",
                "Returns one item without internal actor or idempotency identifiers.",
                "assets_inventory.items",
            ),
            AssetsInventoryReadKind::Store => (
                "Read inventory store",
                "Returns one store without internal actor or idempotency identifiers.",
                "assets_inventory.stores",
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
                DataSensitivity::General,
                resource,
            ),
        }
    }
}

#[async_trait]
impl Capability for AssetsInventoryReadCapability {
    type Input = AssetsInventoryRecordInput;
    type Output = Value;

    fn descriptor(&self) -> &CapabilityDescriptor {
        &self.descriptor
    }

    fn scope(&self, input: &Self::Input) -> CapabilityScope {
        let resource_kind = match self.kind {
            AssetsInventoryReadKind::Item => "assets_inventory_item",
            AssetsInventoryReadKind::Store => "assets_inventory_store",
        };
        resource_scope(resource_kind, input.record_id)
    }

    async fn execute(
        &self,
        context: AuthorizedCapabilityContext,
        input: Self::Input,
    ) -> Result<Self::Output, CapabilityExecutionError> {
        let tenant_id = context.principal().tenant_id();
        let record = match self.kind {
            AssetsInventoryReadKind::Item => ItemOps::get(&self.pool, tenant_id, input.record_id)
                .await
                .map_err(|_| dependency_failure("The inventory item could not be loaded."))?
                .map(|item| item_projection(&item)),
            AssetsInventoryReadKind::Store => StoreOps::get(&self.pool, tenant_id, input.record_id)
                .await
                .map_err(|_| dependency_failure("The inventory store could not be loaded."))?
                .map(|store| store_projection(&store)),
        }
        .ok_or_else(|| {
            CapabilityExecutionError::new(
                CapabilityExecutionErrorCode::InvalidState,
                "The Assets and inventory record was not found.",
            )
        })?;
        Ok(json!({ "record": record }))
    }
}

fn item_projection(item: &ItemResponse) -> Value {
    json!({
        "id": item.id,
        "item_number": item.item_number,
        "name": item.name,
        "description": item.description,
        "barcode": item.barcode,
        "unit_label": item.unit_label,
        "quantity_scale": item.quantity_scale,
        "reorder_level_minor": item.reorder_level_minor,
        "status": item.status,
        "version": item.version,
        "created_at": item.created_at,
        "updated_at": item.updated_at
    })
}

fn store_projection(store: &StoreResponse) -> Value {
    json!({
        "id": store.id,
        "store_number": store.store_number,
        "name": store.name,
        "location_label": store.location_label,
        "notes": store.notes,
        "status": store.status,
        "version": store.version,
        "created_at": store.created_at,
        "updated_at": store.updated_at
    })
}

fn bounded_page(page: Option<i64>, per_page: Option<i64>) -> (i64, i64) {
    (
        page.unwrap_or(1).clamp(1, 1_000_000),
        per_page.unwrap_or(25).clamp(1, 100),
    )
}

fn trimmed(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

fn resource_scope(kind: &str, id: Uuid) -> CapabilityScope {
    CapabilityScope::resources([CapabilityResource::parse(kind, id.to_string())
        .unwrap_or_else(|error| panic!("invalid built-in capability resource: {error}"))])
    .unwrap_or_else(|error| panic!("invalid built-in capability scope: {error}"))
}

fn dependency_failure(message: &'static str) -> CapabilityExecutionError {
    CapabilityExecutionError::new(CapabilityExecutionErrorCode::DependencyUnavailable, message)
}

fn page_schema() -> Value {
    json!({ "type": ["integer", "null"], "minimum": 1, "maximum": 1_000_000 })
}

fn per_page_schema() -> Value {
    json!({ "type": ["integer", "null"], "minimum": 1, "maximum": 100 })
}

fn search_schema() -> Value {
    json!({ "type": ["string", "null"], "maxLength": 200 })
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use cp_assets_inventory::{ItemResponse, StoreResponse};
    use uuid::Uuid;

    use super::{bounded_page, item_projection, store_projection, trimmed};

    #[test]
    fn list_inputs_are_bounded_and_trimmed_before_domain_reads() {
        assert_eq!(bounded_page(None, None), (1, 25));
        assert_eq!(bounded_page(Some(-4), Some(900)), (1, 100));
        assert_eq!(bounded_page(Some(i64::MAX), Some(25)), (1_000_000, 25));
        assert_eq!(trimmed(Some("  science  ")), Some("science"));
        assert_eq!(trimmed(Some("   ")), None);
    }

    #[test]
    fn projections_omit_actor_and_idempotency_identifiers() {
        let now = Utc::now();
        let item = ItemResponse {
            id: Uuid::new_v4(),
            item_number: "ITM-000001".to_string(),
            name: "Science beaker".to_string(),
            description: None,
            barcode: Some("BEAKER-001".to_string()),
            unit_label: "each".to_string(),
            quantity_scale: 0,
            reorder_level_minor: Some(10),
            status: "active".to_string(),
            version: 1,
            created_by: Uuid::new_v4(),
            updated_by: Uuid::new_v4(),
            created_at: now,
            updated_at: now,
        };
        let item_json = item_projection(&item);
        for omitted in ["created_by", "updated_by", "idempotency_key"] {
            assert!(item_json.get(omitted).is_none(), "{omitted}");
        }
        assert_eq!(item_json["item_number"], "ITM-000001");

        let store = StoreResponse {
            id: Uuid::new_v4(),
            store_number: "STR-000001".to_string(),
            name: "Main store".to_string(),
            location_label: Some("Block A".to_string()),
            notes: None,
            status: "active".to_string(),
            version: 1,
            created_by: Uuid::new_v4(),
            updated_by: Uuid::new_v4(),
            created_at: now,
            updated_at: now,
        };
        let store_json = store_projection(&store);
        for omitted in ["created_by", "updated_by", "idempotency_key"] {
            assert!(store_json.get(omitted).is_none(), "{omitted}");
        }
        assert_eq!(store_json["store_number"], "STR-000001");
    }
}

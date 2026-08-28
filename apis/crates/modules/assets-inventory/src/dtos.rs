//! HTTP DTOs for versioned Assets and inventory catalogue operations.
//!
//! Quantities use exact signed 64-bit minor values; `quantity_scale` defines
//! their immutable decimal scale and is never accepted by an update request.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

use crate::models::{ItemRecord, StoreRecord};

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetStatus {
    Active,
    Inactive,
}

impl AssetStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Inactive => "inactive",
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct ItemListQuery {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
    pub search: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreateItemRequest {
    #[validate(length(min = 1, max = 180))]
    pub name: String,
    #[validate(length(max = 2000))]
    pub description: Option<String>,
    #[validate(length(max = 200))]
    pub barcode: Option<String>,
    #[validate(length(min = 1, max = 40))]
    pub unit_label: String,
    #[validate(range(min = 0, max = 6))]
    pub quantity_scale: i16,
    #[validate(range(min = 0i64, max = 9007199254740991i64))]
    pub reorder_level_minor: Option<i64>,
    #[validate(length(min = 1, max = 200))]
    pub idempotency_key: String,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateItemRequest {
    #[validate(length(min = 1, max = 180))]
    pub name: String,
    #[validate(length(max = 2000))]
    pub description: Option<String>,
    #[validate(length(max = 200))]
    pub barcode: Option<String>,
    #[validate(range(min = 0i64, max = 9007199254740991i64))]
    pub reorder_level_minor: Option<i64>,
    pub status: AssetStatus,
    #[validate(range(min = 1))]
    pub expected_version: i32,
}

#[derive(Debug, Clone, Serialize)]
pub struct ItemResponse {
    pub id: Uuid,
    pub item_number: String,
    pub name: String,
    pub description: Option<String>,
    pub barcode: Option<String>,
    pub unit_label: String,
    pub quantity_scale: i16,
    /// Exact reorder threshold in the item's immutable quantity scale.
    pub reorder_level_minor: Option<i64>,
    pub status: String,
    pub version: i32,
    pub created_by: Uuid,
    pub updated_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<ItemRecord> for ItemResponse {
    fn from(item: ItemRecord) -> Self {
        Self {
            id: item.id,
            item_number: item.item_number,
            name: item.name,
            description: item.description,
            barcode: item.barcode,
            unit_label: item.unit_label,
            quantity_scale: item.quantity_scale,
            reorder_level_minor: item.reorder_level_minor,
            status: item.status,
            version: item.version,
            created_by: item.created_by,
            updated_by: item.updated_by,
            created_at: item.created_at,
            updated_at: item.updated_at,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct PaginatedItemsResponse {
    pub items: Vec<ItemResponse>,
}

#[derive(Debug, Deserialize)]
pub struct StoreListQuery {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
    pub search: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreateStoreRequest {
    #[validate(length(min = 1, max = 180))]
    pub name: String,
    #[validate(length(max = 200))]
    pub location_label: Option<String>,
    #[validate(length(max = 2000))]
    pub notes: Option<String>,
    #[validate(length(min = 1, max = 200))]
    pub idempotency_key: String,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateStoreRequest {
    #[validate(length(min = 1, max = 180))]
    pub name: String,
    #[validate(length(max = 200))]
    pub location_label: Option<String>,
    #[validate(length(max = 2000))]
    pub notes: Option<String>,
    pub status: AssetStatus,
    #[validate(range(min = 1))]
    pub expected_version: i32,
}

#[derive(Debug, Clone, Serialize)]
pub struct StoreResponse {
    pub id: Uuid,
    pub store_number: String,
    pub name: String,
    pub location_label: Option<String>,
    pub notes: Option<String>,
    pub status: String,
    pub version: i32,
    pub created_by: Uuid,
    pub updated_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<StoreRecord> for StoreResponse {
    fn from(store: StoreRecord) -> Self {
        Self {
            id: store.id,
            store_number: store.store_number,
            name: store.name,
            location_label: store.location_label,
            notes: store.notes,
            status: store.status,
            version: store.version,
            created_by: store.created_by,
            updated_by: store.updated_by,
            created_at: store.created_at,
            updated_at: store.updated_at,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct PaginatedStoresResponse {
    pub stores: Vec<StoreResponse>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct DeleteAssetQuery {
    #[validate(range(min = 1))]
    pub expected_version: i32,
}

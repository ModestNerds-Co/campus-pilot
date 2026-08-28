//! Persistence records for Assets and inventory catalogues.
//!
//! These records retain tenant and idempotency identity used by transactional
//! operations; public API projections live in `dtos`.

use chrono::{DateTime, Utc};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, FromRow)]
pub(crate) struct ItemRecord {
    pub(crate) id: Uuid,
    pub(crate) item_number: String,
    pub(crate) name: String,
    pub(crate) description: Option<String>,
    pub(crate) barcode: Option<String>,
    pub(crate) unit_label: String,
    pub(crate) quantity_scale: i16,
    pub(crate) reorder_level_minor: Option<i64>,
    pub(crate) status: String,
    pub(crate) version: i32,
    pub(crate) create_request_fingerprint: String,
    pub(crate) created_by: Uuid,
    pub(crate) updated_by: Uuid,
    pub(crate) deleted_at: Option<DateTime<Utc>>,
    pub(crate) created_at: DateTime<Utc>,
    pub(crate) updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
pub(crate) struct StoreRecord {
    pub(crate) id: Uuid,
    pub(crate) store_number: String,
    pub(crate) name: String,
    pub(crate) location_label: Option<String>,
    pub(crate) notes: Option<String>,
    pub(crate) status: String,
    pub(crate) version: i32,
    pub(crate) create_request_fingerprint: String,
    pub(crate) created_by: Uuid,
    pub(crate) updated_by: Uuid,
    pub(crate) deleted_at: Option<DateTime<Utc>>,
    pub(crate) created_at: DateTime<Utc>,
    pub(crate) updated_at: DateTime<Utc>,
}

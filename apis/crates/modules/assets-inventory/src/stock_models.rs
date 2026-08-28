//! Persistence projections for the append-only stock ledger.
//!
//! Public responses are separate so transaction-internal draft fields never
//! cross the Assets and inventory boundary.

use chrono::{DateTime, NaiveDate, Utc};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, FromRow)]
pub(crate) struct StockBalanceRecord {
    pub(crate) item_id: Uuid,
    pub(crate) item_number: String,
    pub(crate) item_name: String,
    pub(crate) store_id: Uuid,
    pub(crate) store_number: String,
    pub(crate) store_name: String,
    pub(crate) on_hand_minor: i64,
    pub(crate) quantity_scale: i16,
    pub(crate) unit_label: String,
    pub(crate) version: i32,
    pub(crate) updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
pub(crate) struct StockMovementSummaryRecord {
    pub(crate) id: Uuid,
    pub(crate) movement_number: String,
    pub(crate) kind: String,
    pub(crate) effective_on: NaiveDate,
    pub(crate) reference: Option<String>,
    pub(crate) reason: Option<String>,
    pub(crate) source_goods_receipt_id: Option<Uuid>,
    pub(crate) source_goods_receipt_number: Option<String>,
    pub(crate) reverses_movement_id: Option<Uuid>,
    pub(crate) reverses_movement_number: Option<String>,
    pub(crate) reversed_by_movement_id: Option<Uuid>,
    pub(crate) reversed_by_movement_number: Option<String>,
    pub(crate) status: String,
    pub(crate) version: i32,
    pub(crate) line_count: i64,
    pub(crate) created_by: Uuid,
    pub(crate) posted_by: Uuid,
    pub(crate) posted_at: DateTime<Utc>,
    pub(crate) created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
pub(crate) struct StockMovementLineRecord {
    pub(crate) id: Uuid,
    pub(crate) line_number: i32,
    pub(crate) item_id: Uuid,
    pub(crate) item_number: String,
    pub(crate) item_name: String,
    pub(crate) store_id: Uuid,
    pub(crate) store_number: String,
    pub(crate) store_name: String,
    pub(crate) quantity_delta_minor: i64,
    pub(crate) quantity_scale: i16,
    pub(crate) unit_label: String,
    pub(crate) on_hand_before_minor: i64,
    pub(crate) on_hand_after_minor: i64,
    pub(crate) source_goods_receipt_line_id: Option<Uuid>,
    pub(crate) source_goods_receipt_line_number: Option<i32>,
    pub(crate) source_goods_receipt_description: Option<String>,
}

#[derive(Debug, Clone, FromRow)]
pub(crate) struct ItemStockSnapshot {
    pub(crate) id: Uuid,
    pub(crate) item_number: String,
    pub(crate) name: String,
    pub(crate) unit_label: String,
    pub(crate) quantity_scale: i16,
}

#[derive(Debug, Clone, FromRow)]
pub(crate) struct StoreStockSnapshot {
    pub(crate) id: Uuid,
    pub(crate) store_number: String,
    pub(crate) name: String,
}

#[derive(Debug, Clone, FromRow)]
pub(crate) struct OriginalMovementRecord {
    pub(crate) id: Uuid,
    pub(crate) movement_number: String,
    pub(crate) kind: String,
    pub(crate) source_goods_receipt_id: Option<Uuid>,
    pub(crate) source_goods_receipt_number: Option<String>,
}

#[derive(Debug, Clone, FromRow)]
pub(crate) struct OriginalMovementLineRecord {
    pub(crate) id: Uuid,
    pub(crate) item_id: Uuid,
    pub(crate) item_number: String,
    pub(crate) item_name: String,
    pub(crate) store_id: Uuid,
    pub(crate) store_number: String,
    pub(crate) store_name: String,
    pub(crate) quantity_delta_minor: i64,
    pub(crate) quantity_scale: i16,
    pub(crate) unit_label: String,
    pub(crate) source_goods_receipt_line_id: Option<Uuid>,
    pub(crate) source_goods_receipt_line_number: Option<i32>,
    pub(crate) source_goods_receipt_description: Option<String>,
}

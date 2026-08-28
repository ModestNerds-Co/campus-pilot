//! HTTP contracts for exact stock balances, movements, and receipt allocation.
//!
//! Quantities are integer minor units bounded to JavaScript's exact range.
//! Draft movement state is transaction-internal and never appears here.

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

#[derive(Debug, Deserialize)]
pub struct StockBalanceListQuery {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
    pub search: Option<String>,
    pub item_id: Option<Uuid>,
    pub store_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize)]
pub struct StockBalanceResponse {
    pub item_id: Uuid,
    pub item_number: String,
    pub item_name: String,
    pub store_id: Uuid,
    pub store_number: String,
    pub store_name: String,
    pub on_hand_minor: i64,
    pub quantity_scale: i16,
    pub unit_label: String,
    pub version: i32,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct PaginatedStockBalancesResponse {
    pub balances: Vec<StockBalanceResponse>,
}

#[derive(Debug, Deserialize)]
pub struct StockMovementListQuery {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
    pub search: Option<String>,
    pub kind: Option<String>,
    pub item_id: Option<Uuid>,
    pub store_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize)]
pub struct StockMovementSummaryResponse {
    pub id: Uuid,
    pub movement_number: String,
    pub kind: String,
    pub effective_on: NaiveDate,
    pub reference: Option<String>,
    pub reason: Option<String>,
    pub source_goods_receipt_id: Option<Uuid>,
    pub source_goods_receipt_number: Option<String>,
    pub reverses_movement_id: Option<Uuid>,
    pub reverses_movement_number: Option<String>,
    pub reversed_by_movement_id: Option<Uuid>,
    pub reversed_by_movement_number: Option<String>,
    pub status: String,
    pub version: i32,
    pub line_count: i64,
    pub created_by: Uuid,
    pub posted_by: Uuid,
    pub posted_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct StockMovementLineResponse {
    pub id: Uuid,
    pub line_number: i32,
    pub item_id: Uuid,
    pub item_number: String,
    pub item_name: String,
    pub store_id: Uuid,
    pub store_number: String,
    pub store_name: String,
    pub quantity_delta_minor: i64,
    pub quantity_scale: i16,
    pub unit_label: String,
    pub on_hand_before_minor: i64,
    pub on_hand_after_minor: i64,
    pub source_goods_receipt_line_id: Option<Uuid>,
    pub source_goods_receipt_line_number: Option<i32>,
    pub source_goods_receipt_description: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct StockMovementResponse {
    #[serde(flatten)]
    pub summary: StockMovementSummaryResponse,
    pub lines: Vec<StockMovementLineResponse>,
}

#[derive(Debug, Serialize)]
pub struct PaginatedStockMovementsResponse {
    pub movements: Vec<StockMovementSummaryResponse>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Validate)]
pub struct StockQuantityLineInput {
    pub item_id: Uuid,
    pub store_id: Uuid,
    #[validate(range(min = 1i64, max = 9007199254740991i64))]
    pub quantity_minor: i64,
}

#[derive(Debug, Deserialize, Serialize, Validate)]
pub struct ManualReceiptRequest {
    pub effective_on: NaiveDate,
    #[validate(length(max = 200))]
    pub reference: Option<String>,
    #[validate(length(max = 2000))]
    pub reason: Option<String>,
    #[validate(length(min = 1, max = 200))]
    pub idempotency_key: String,
    #[validate(length(min = 1, max = 200), nested)]
    pub lines: Vec<StockQuantityLineInput>,
}

#[derive(Debug, Deserialize, Serialize, Validate)]
pub struct IssueStockRequest {
    pub effective_on: NaiveDate,
    #[validate(length(max = 200))]
    pub reference: Option<String>,
    #[validate(length(max = 2000))]
    pub reason: Option<String>,
    #[validate(length(min = 1, max = 200))]
    pub idempotency_key: String,
    #[validate(length(min = 1, max = 200), nested)]
    pub lines: Vec<StockQuantityLineInput>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Validate)]
pub struct TransferStockLineInput {
    pub item_id: Uuid,
    pub from_store_id: Uuid,
    pub to_store_id: Uuid,
    #[validate(range(min = 1i64, max = 9007199254740991i64))]
    pub quantity_minor: i64,
}

#[derive(Debug, Deserialize, Serialize, Validate)]
pub struct TransferStockRequest {
    pub effective_on: NaiveDate,
    #[validate(length(max = 200))]
    pub reference: Option<String>,
    #[validate(length(max = 2000))]
    pub reason: Option<String>,
    #[validate(length(min = 1, max = 200))]
    pub idempotency_key: String,
    #[validate(length(min = 1, max = 200), nested)]
    pub lines: Vec<TransferStockLineInput>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Validate)]
pub struct AdjustStockLineInput {
    pub item_id: Uuid,
    pub store_id: Uuid,
    #[validate(range(min = 0i64, max = 9007199254740991i64))]
    pub expected_on_hand_minor: i64,
    #[validate(range(min = 0i64, max = 9007199254740991i64))]
    pub counted_on_hand_minor: i64,
}

#[derive(Debug, Deserialize, Serialize, Validate)]
pub struct AdjustStockRequest {
    pub effective_on: NaiveDate,
    #[validate(length(max = 200))]
    pub reference: Option<String>,
    #[validate(length(min = 1, max = 2000))]
    pub reason: String,
    #[validate(length(min = 1, max = 200))]
    pub idempotency_key: String,
    #[validate(length(min = 1, max = 200), nested)]
    pub lines: Vec<AdjustStockLineInput>,
}

#[derive(Debug, Deserialize, Serialize, Validate)]
pub struct ReverseStockMovementRequest {
    pub effective_on: NaiveDate,
    #[validate(length(min = 1, max = 2000))]
    pub reason: String,
    #[validate(length(min = 1, max = 200))]
    pub idempotency_key: String,
}

#[derive(Debug, Deserialize)]
pub struct GoodsReceiptAllocationListQuery {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
    pub search: Option<String>,
    pub goods_receipt_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GoodsReceiptAllocationLineResponse {
    pub id: Uuid,
    pub line_number: i32,
    pub description: String,
    pub unit_label: Option<String>,
    pub quantity_minor: i64,
    pub quantity_scale: i16,
    pub allocated_quantity_minor: i64,
    pub remaining_quantity_minor: i64,
    pub mapped_item_id: Option<Uuid>,
    pub mapped_item_number: Option<String>,
    pub mapped_item_name: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GoodsReceiptAllocationResponse {
    pub id: Uuid,
    pub goods_receipt_number: String,
    pub purchase_order_id: Uuid,
    pub purchase_order_number: String,
    pub supplier_id: Uuid,
    pub supplier_number: String,
    pub supplier_name: String,
    pub received_on: NaiveDate,
    pub delivery_reference: Option<String>,
    pub lines: Vec<GoodsReceiptAllocationLineResponse>,
}

#[derive(Debug, Serialize)]
pub struct PaginatedGoodsReceiptAllocationsResponse {
    pub goods_receipts: Vec<GoodsReceiptAllocationResponse>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Validate)]
pub struct AllocateGoodsReceiptLineInput {
    pub goods_receipt_line_id: Uuid,
    pub item_id: Uuid,
    pub store_id: Uuid,
    #[validate(range(min = 1i64, max = 9007199254740991i64))]
    pub quantity_minor: i64,
}

#[derive(Debug, Deserialize, Serialize, Validate)]
pub struct AllocateGoodsReceiptRequest {
    pub goods_receipt_id: Uuid,
    pub effective_on: NaiveDate,
    #[validate(length(max = 2000))]
    pub reason: Option<String>,
    #[validate(length(min = 1, max = 200))]
    pub idempotency_key: String,
    #[validate(length(min = 1, max = 200), nested)]
    pub lines: Vec<AllocateGoodsReceiptLineInput>,
}

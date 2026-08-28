/** Stock-ledger API contracts. Quantities remain exact scaled integers end to end. */

export type StockMovementKind =
  | "manual_receipt"
  | "issue"
  | "transfer"
  | "adjustment"
  | "goods_receipt_allocation"
  | "reversal";

export type StockMovementStatus = "posted";

export interface StockBalance {
  item_id: string;
  item_number: string;
  item_name: string;
  store_id: string;
  store_number: string;
  store_name: string;
  on_hand_minor: number;
  quantity_scale: number;
  unit_label: string;
  version: number;
  updated_at: string;
}

export interface StockBalancesResponse {
  balances: StockBalance[];
}

export interface StockBalanceListParams {
  page?: number;
  per_page?: number;
  search?: string;
  item_id?: string;
  store_id?: string;
}

export interface StockMovementSummary {
  id: string;
  movement_number: string;
  kind: StockMovementKind;
  effective_on: string;
  reference: string | null;
  reason: string | null;
  source_goods_receipt_id: string | null;
  source_goods_receipt_number: string | null;
  reverses_movement_id: string | null;
  reverses_movement_number: string | null;
  reversed_by_movement_id: string | null;
  reversed_by_movement_number: string | null;
  status: StockMovementStatus;
  line_count: number;
  version: number;
  created_by: string;
  posted_by: string;
  posted_at: string;
  created_at: string;
}

export interface StockMovementLine {
  id: string;
  line_number: number;
  item_id: string;
  item_number: string;
  item_name: string;
  store_id: string;
  store_number: string;
  store_name: string;
  quantity_delta_minor: number;
  quantity_scale: number;
  unit_label: string;
  on_hand_before_minor: number;
  on_hand_after_minor: number;
  source_goods_receipt_line_id: string | null;
  source_goods_receipt_line_number: number | null;
  source_goods_receipt_description: string | null;
}

export interface StockMovement extends StockMovementSummary {
  lines: StockMovementLine[];
}

export interface StockMovementsResponse {
  movements: StockMovementSummary[];
}

export interface StockMovementListParams {
  page?: number;
  per_page?: number;
  search?: string;
  kind?: StockMovementKind;
  item_id?: string;
  store_id?: string;
}

interface StockCommandHeader {
  effective_on: string;
  reference: string | null;
  reason: string | null;
  idempotency_key: string;
}

export interface StockQuantityLineInput {
  item_id: string;
  store_id: string;
  quantity_minor: number;
}

export interface ManualReceiptInput extends StockCommandHeader {
  lines: StockQuantityLineInput[];
}

export interface StockIssueInput extends StockCommandHeader {
  lines: StockQuantityLineInput[];
}

export interface StockTransferLineInput {
  item_id: string;
  from_store_id: string;
  to_store_id: string;
  quantity_minor: number;
}

export interface StockTransferInput extends StockCommandHeader {
  lines: StockTransferLineInput[];
}

export interface StockAdjustmentLineInput {
  item_id: string;
  store_id: string;
  expected_on_hand_minor: number;
  counted_on_hand_minor: number;
}

export interface StockAdjustmentInput extends StockCommandHeader {
  reason: string;
  lines: StockAdjustmentLineInput[];
}

export interface ReverseStockMovementInput {
  effective_on: string;
  reason: string;
  idempotency_key: string;
}

export interface GoodsReceiptAllocationLine {
  id: string;
  line_number: number;
  description: string;
  unit_label: string | null;
  quantity_minor: number;
  quantity_scale: number;
  allocated_quantity_minor: number;
  remaining_quantity_minor: number;
  mapped_item_id: string | null;
  mapped_item_number: string | null;
  mapped_item_name: string | null;
}

export interface GoodsReceiptAllocationSource {
  id: string;
  goods_receipt_number: string;
  purchase_order_id: string;
  purchase_order_number: string;
  supplier_id: string;
  supplier_number: string;
  supplier_name: string;
  received_on: string;
  delivery_reference: string | null;
  lines: GoodsReceiptAllocationLine[];
}

export interface GoodsReceiptAllocationSourcesResponse {
  goods_receipts: GoodsReceiptAllocationSource[];
}

export interface GoodsReceiptAllocationListParams {
  page?: number;
  per_page?: number;
  search?: string;
  goods_receipt_id?: string;
}

export interface GoodsReceiptAllocationInputLine {
  goods_receipt_line_id: string;
  item_id: string;
  store_id: string;
  quantity_minor: number;
}

export interface GoodsReceiptAllocationInput {
  goods_receipt_id: string;
  effective_on: string;
  reason: string | null;
  idempotency_key: string;
  lines: GoodsReceiptAllocationInputLine[];
}

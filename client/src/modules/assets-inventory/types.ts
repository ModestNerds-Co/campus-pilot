/** Assets and Inventory API contracts. Quantities use exact scaled integers. */

export type InventoryItemStatus = "active" | "inactive";
export type InventoryStoreStatus = "active" | "inactive";

export interface PaginationMeta {
  current_page: number;
  per_page: number;
  total: number;
  total_pages: number;
  has_next: boolean;
  has_prev: boolean;
}

export interface ApiEnvelope<T> {
  success: boolean;
  message: string | null;
  data: T | null;
  pagination: PaginationMeta | null;
  issues: Array<string | { detail?: string }> | null;
  /** Transport status retained for conflict and not-found UI without leaking Axios. */
  http_status?: number;
}

export interface ListParams {
  page?: number;
  per_page?: number;
  search?: string;
  status?: string;
}

export interface InventoryItem {
  id: string;
  item_number: string;
  name: string;
  description: string | null;
  barcode: string | null;
  unit_label: string;
  quantity_scale: number;
  reorder_level_minor: number | null;
  status: InventoryItemStatus;
  version: number;
  created_by: string;
  updated_by: string;
  created_at: string;
  updated_at: string;
}

export interface CreateInventoryItemInput {
  name: string;
  description: string | null;
  barcode: string | null;
  unit_label: string;
  quantity_scale: number;
  reorder_level_minor: number | null;
}

export interface UpdateInventoryItemInput {
  name: string;
  description: string | null;
  barcode: string | null;
  reorder_level_minor: number | null;
  status: InventoryItemStatus;
}

export interface InventoryItemsResponse {
  items: InventoryItem[];
}

export interface InventoryStore {
  id: string;
  store_number: string;
  name: string;
  location_label: string | null;
  notes: string | null;
  status: InventoryStoreStatus;
  version: number;
  created_by: string;
  updated_by: string;
  created_at: string;
  updated_at: string;
}

export interface CreateInventoryStoreInput {
  name: string;
  location_label: string | null;
  notes: string | null;
}

export interface UpdateInventoryStoreInput extends CreateInventoryStoreInput {
  status: InventoryStoreStatus;
}

export interface InventoryStoresResponse {
  stores: InventoryStore[];
}

/** Client boundary for licensed Assets and Inventory master-data operations. */

import { AxiosError } from "axios";

import { httpClient } from "@/lib/http-client";

import type {
  ApiEnvelope, CreateInventoryItemInput, CreateInventoryStoreInput, InventoryItem,
  InventoryItemsResponse, InventoryStore, InventoryStoresResponse, ListParams,
  UpdateInventoryStoreInput,
  UpdateInventoryItemInput,
} from "./types";
import type {
  GoodsReceiptAllocationInput, GoodsReceiptAllocationListParams,
  GoodsReceiptAllocationSourcesResponse, ManualReceiptInput, ReverseStockMovementInput,
  StockAdjustmentInput, StockBalanceListParams, StockBalancesResponse, StockIssueInput,
  StockMovement, StockMovementListParams, StockMovementsResponse, StockTransferInput,
} from "./stock-types";
import type {
  ApproveStockRequestInput, CloseStockRequestInput, CreateStockRequestInput,
  FulfilStockRequestInput, FulfilStockRequestResponse, StockRequest,
  StockRequestDepartmentsResponse, StockRequestFulfilmentPreview, StockRequestListParams,
  StockRequesterCandidatesResponse, StockRequestsResponse, StockRequestReasonCommand,
  StockRequestVersionCommand, UpdateStockRequestInput,
} from "./stock-request-types";

const BASE_URL = "/api/1.0/assets-inventory";

async function request<T>(work: () => Promise<{ data: ApiEnvelope<T>; status: number }>): Promise<ApiEnvelope<T>> {
  try {
    const response = await work();
    return { ...response.data, http_status: response.status };
  } catch (error) {
    if (error instanceof AxiosError && error.response) {
      return { ...(error.response.data as ApiEnvelope<T>), http_status: error.response.status };
    }
    throw error;
  }
}

export const assetsInventoryService = {
  listItems: (params?: ListParams) => request<InventoryItemsResponse>(() => httpClient.get(`${BASE_URL}/items`, { params })),
  createItem: (data: CreateInventoryItemInput & { idempotency_key: string }) => request<InventoryItem>(() => httpClient.post(`${BASE_URL}/items`, data)),
  updateItem: (id: string, data: UpdateInventoryItemInput & { expected_version: number }) => request<InventoryItem>(() => httpClient.put(`${BASE_URL}/items/${id}`, data)),
  deleteItem: (id: string, expectedVersion: number) => request<{ deleted: boolean }>(() => httpClient.delete(`${BASE_URL}/items/${id}`, { params: { expected_version: expectedVersion } })),

  listStores: (params?: ListParams) => request<InventoryStoresResponse>(() => httpClient.get(`${BASE_URL}/stores`, { params })),
  createStore: (data: CreateInventoryStoreInput & { idempotency_key: string }) => request<InventoryStore>(() => httpClient.post(`${BASE_URL}/stores`, data)),
  updateStore: (id: string, data: UpdateInventoryStoreInput & { expected_version: number }) => request<InventoryStore>(() => httpClient.put(`${BASE_URL}/stores/${id}`, data)),
  deleteStore: (id: string, expectedVersion: number) => request<{ deleted: boolean }>(() => httpClient.delete(`${BASE_URL}/stores/${id}`, { params: { expected_version: expectedVersion } })),

  listStockBalances: (params?: StockBalanceListParams) => request<StockBalancesResponse>(() => httpClient.get(`${BASE_URL}/stock-balances`, { params })),
  listStockMovements: (params?: StockMovementListParams) => request<StockMovementsResponse>(() => httpClient.get(`${BASE_URL}/stock-movements`, { params })),
  readStockMovement: (id: string) => request<StockMovement>(() => httpClient.get(`${BASE_URL}/stock-movements/${id}`)),
  createManualReceipt: (data: ManualReceiptInput) => request<StockMovement>(() => httpClient.post(`${BASE_URL}/manual-receipts`, data)),
  createIssue: (data: StockIssueInput) => request<StockMovement>(() => httpClient.post(`${BASE_URL}/issues`, data)),
  createTransfer: (data: StockTransferInput) => request<StockMovement>(() => httpClient.post(`${BASE_URL}/transfers`, data)),
  createAdjustment: (data: StockAdjustmentInput) => request<StockMovement>(() => httpClient.post(`${BASE_URL}/adjustments`, data)),
  reverseStockMovement: (id: string, data: ReverseStockMovementInput) => request<StockMovement>(() => httpClient.post(`${BASE_URL}/stock-movements/${id}/reverse`, data)),
  listGoodsReceiptAllocations: (params?: GoodsReceiptAllocationListParams) => request<GoodsReceiptAllocationSourcesResponse>(() => httpClient.get(`${BASE_URL}/goods-receipt-allocations`, { params })),
  createGoodsReceiptAllocation: (data: GoodsReceiptAllocationInput) => request<StockMovement>(() => httpClient.post(`${BASE_URL}/goods-receipt-allocations`, data)),

  listStockRequesters: (params?: { search?: string; department_id?: string }) => request<StockRequesterCandidatesResponse>(() => httpClient.get(`${BASE_URL}/stock-request-requesters`, { params })),
  listStockRequestDepartments: (params?: { search?: string }) => request<StockRequestDepartmentsResponse>(() => httpClient.get(`${BASE_URL}/stock-request-departments`, { params })),
  listStockRequests: (params?: StockRequestListParams) => request<StockRequestsResponse>(() => httpClient.get(`${BASE_URL}/stock-requests`, { params })),
  readStockRequest: (id: string) => request<StockRequest>(() => httpClient.get(`${BASE_URL}/stock-requests/${id}`)),
  readStockRequestFulfilmentPreview: (id: string) => request<StockRequestFulfilmentPreview>(() => httpClient.get(`${BASE_URL}/stock-requests/${id}/fulfilment-preview`)),
  createStockRequest: (data: CreateStockRequestInput) => request<StockRequest>(() => httpClient.post(`${BASE_URL}/stock-requests`, data)),
  updateStockRequest: (id: string, data: UpdateStockRequestInput) => request<StockRequest>(() => httpClient.put(`${BASE_URL}/stock-requests/${id}`, data)),
  deleteStockRequest: (id: string, data: StockRequestVersionCommand) => request<{ deleted: boolean }>(() => httpClient.delete(`${BASE_URL}/stock-requests/${id}`, { data })),
  submitStockRequest: (id: string, data: StockRequestVersionCommand) => request<StockRequest>(() => httpClient.post(`${BASE_URL}/stock-requests/${id}/submit`, data)),
  approveStockRequest: (id: string, data: ApproveStockRequestInput) => request<StockRequest>(() => httpClient.post(`${BASE_URL}/stock-requests/${id}/approve`, data)),
  rejectStockRequest: (id: string, data: StockRequestReasonCommand) => request<StockRequest>(() => httpClient.post(`${BASE_URL}/stock-requests/${id}/reject`, data)),
  cancelStockRequest: (id: string, data: StockRequestReasonCommand) => request<StockRequest>(() => httpClient.post(`${BASE_URL}/stock-requests/${id}/cancel`, data)),
  closeStockRequest: (id: string, data: CloseStockRequestInput) => request<StockRequest>(() => httpClient.post(`${BASE_URL}/stock-requests/${id}/close`, data)),
  fulfilStockRequest: (id: string, data: FulfilStockRequestInput) => request<FulfilStockRequestResponse>(() => httpClient.post(`${BASE_URL}/stock-requests/${id}/fulfilments`, data)),
};

export function responseMessage(response: Pick<ApiEnvelope<unknown>, "issues" | "message">, fallback: string) {
  const issue = response.issues?.[0];
  if (typeof issue === "string") return issue;
  return issue?.detail || response.message || fallback;
}

/** Client boundary for licensed Procurement operations. */

import { AxiosError } from "axios";

import { httpClient } from "@/lib/http-client";

import type {
  ApiEnvelope,
  GoodsReceipt,
  GoodsReceiptInput,
  GoodsReceiptUpdateInput,
  GoodsReceiptsResponse,
  ProcurementReferenceData,
  Requisition,
  RequisitionInput,
  RequisitionListParams,
  RequisitionsResponse,
  RequesterCandidatesResponse,
  Supplier,
  SupplierInput,
  SuppliersResponse,
  ListParams,
  PurchaseOrder,
  PurchaseOrderInput,
  PurchaseOrderUpdateInput,
  PurchaseOrdersResponse,
  SupplierStatus,
} from "./types";

const BASE_URL = "/api/1.0/procurement";

async function request<T>(work: () => Promise<{ data: ApiEnvelope<T> }>): Promise<ApiEnvelope<T>> {
  try {
    return (await work()).data;
  } catch (error) {
    if (error instanceof AxiosError && error.response) return error.response.data as ApiEnvelope<T>;
    throw error;
  }
}

export const procurementService = {
  referenceData: () =>
    request<ProcurementReferenceData>(() => httpClient.get(`${BASE_URL}/reference-data`)),
  requesterCandidates: (search?: string) =>
    request<RequesterCandidatesResponse>(() => httpClient.get(`${BASE_URL}/requester-candidates`, { params: { search } })),

  listSuppliers: (params?: ListParams) =>
    request<SuppliersResponse>(() => httpClient.get(`${BASE_URL}/suppliers`, { params })),
  readSupplier: (id: string) =>
    request<Supplier>(() => httpClient.get(`${BASE_URL}/suppliers/${id}`)),
  createSupplier: (data: SupplierInput & { idempotency_key: string }) =>
    request<Supplier>(() => httpClient.post(`${BASE_URL}/suppliers`, data)),
  updateSupplier: (id: string, data: SupplierInput & { status: SupplierStatus; expected_version: number }) =>
    request<Supplier>(() => httpClient.put(`${BASE_URL}/suppliers/${id}`, data)),
  deleteSupplier: (id: string, expectedVersion: number) =>
    request<{ deleted: boolean }>(() => httpClient.delete(`${BASE_URL}/suppliers/${id}`, { params: { expected_version: expectedVersion } })),

  listRequisitions: (params?: RequisitionListParams) =>
    request<RequisitionsResponse>(() => httpClient.get(`${BASE_URL}/requisitions`, { params })),
  readRequisition: (id: string) =>
    request<Requisition>(() => httpClient.get(`${BASE_URL}/requisitions/${id}`)),
  createRequisition: (data: RequisitionInput & { idempotency_key: string }) =>
    request<Requisition>(() => httpClient.post(`${BASE_URL}/requisitions`, data)),
  updateRequisition: (id: string, data: RequisitionInput & { expected_version: number }) =>
    request<Requisition>(() => httpClient.put(`${BASE_URL}/requisitions/${id}`, data)),
  deleteRequisition: (id: string, expectedVersion: number) =>
    request<{ deleted: boolean }>(() => httpClient.delete(`${BASE_URL}/requisitions/${id}`, { params: { expected_version: expectedVersion } })),
  submitRequisition: (id: string, expectedVersion: number) =>
    request<Requisition>(() => httpClient.post(`${BASE_URL}/requisitions/${id}/submit`, { expected_version: expectedVersion })),
  approveRequisition: (id: string, expectedVersion: number, note: string | null) =>
    request<Requisition>(() => httpClient.post(`${BASE_URL}/requisitions/${id}/approve`, { expected_version: expectedVersion, note })),
  rejectRequisition: (id: string, expectedVersion: number, note: string | null) =>
    request<Requisition>(() => httpClient.post(`${BASE_URL}/requisitions/${id}/reject`, { expected_version: expectedVersion, note })),
  cancelRequisition: (id: string, expectedVersion: number, note: string | null) =>
    request<Requisition>(() => httpClient.post(`${BASE_URL}/requisitions/${id}/cancel`, { expected_version: expectedVersion, note })),

  listPurchaseOrders: (params?: ListParams) =>
    request<PurchaseOrdersResponse>(() => httpClient.get(`${BASE_URL}/purchase-orders`, { params })),
  readPurchaseOrder: (id: string) =>
    request<PurchaseOrder>(() => httpClient.get(`${BASE_URL}/purchase-orders/${id}`)),
  createPurchaseOrder: (data: PurchaseOrderInput & { idempotency_key: string }) =>
    request<PurchaseOrder>(() => httpClient.post(`${BASE_URL}/purchase-orders`, data)),
  updatePurchaseOrder: (id: string, data: PurchaseOrderUpdateInput & { expected_version: number }) =>
    request<PurchaseOrder>(() => httpClient.put(`${BASE_URL}/purchase-orders/${id}`, data)),
  issuePurchaseOrder: (id: string, expectedVersion: number) =>
    request<PurchaseOrder>(() => httpClient.post(`${BASE_URL}/purchase-orders/${id}/issue`, { expected_version: expectedVersion })),
  cancelPurchaseOrder: (id: string, expectedVersion: number, note: string) =>
    request<PurchaseOrder>(() => httpClient.post(`${BASE_URL}/purchase-orders/${id}/cancel`, { expected_version: expectedVersion, note })),

  listGoodsReceipts: (params?: ListParams) =>
    request<GoodsReceiptsResponse>(() => httpClient.get(`${BASE_URL}/goods-receipts`, { params })),
  readGoodsReceipt: (id: string) =>
    request<GoodsReceipt>(() => httpClient.get(`${BASE_URL}/goods-receipts/${id}`)),
  createGoodsReceipt: (data: GoodsReceiptInput & { idempotency_key: string }) =>
    request<GoodsReceipt>(() => httpClient.post(`${BASE_URL}/goods-receipts`, data)),
  updateGoodsReceipt: (id: string, data: GoodsReceiptUpdateInput & { expected_version: number }) =>
    request<GoodsReceipt>(() => httpClient.put(`${BASE_URL}/goods-receipts/${id}`, data)),
  postGoodsReceipt: (id: string, expectedVersion: number) =>
    request<GoodsReceipt>(() => httpClient.post(`${BASE_URL}/goods-receipts/${id}/post`, { expected_version: expectedVersion })),
};

export function responseMessage(response: Pick<ApiEnvelope<unknown>, "issues" | "message">, fallback: string) {
  const issue = response.issues?.[0];
  if (typeof issue === "string") return issue;
  return issue?.detail || response.message || fallback;
}

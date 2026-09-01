/** Typed HTTP boundary for Facilities. */

import { AxiosError } from "axios";

import { httpClient } from "@/lib/http-client";

import type {
  ApiEnvelope,
  FacilityInspectionOutcome,
  FacilityLocation,
  FacilityLocationKind,
  FacilityPriority,
  FacilityReferences,
  FacilityRequestStatus,
  FacilityServiceRequestRecord,
  FacilityServiceRequestSummary,
  FacilityWorkOrderRecord,
  FacilityWorkOrderStatus,
  FacilityWorkOrderSummary,
  LocationPayload,
  ServiceRequestPayload,
  WorkOrderPayload,
} from "./types";

const BASE = "/api/1.0/facilities";

async function request<T>(work: () => Promise<{ data: ApiEnvelope<T> }>): Promise<ApiEnvelope<T>> {
  try {
    return (await work()).data;
  } catch (error) {
    if (error instanceof AxiosError && error.response) return error.response.data as ApiEnvelope<T>;
    throw error;
  }
}

export const facilitiesService = {
  locations: (params?: { parent_id?: string; kind?: FacilityLocationKind; status?: string; search?: string }) =>
    request<FacilityLocation[]>(() => httpClient.get(`${BASE}/locations`, { params })),
  location: (id: string) => request<FacilityLocation>(() => httpClient.get(`${BASE}/locations/${id}`)),
  createLocation: (payload: LocationPayload) => request<FacilityLocation>(() => httpClient.post(`${BASE}/locations`, payload)),
  updateLocation: (record: FacilityLocation, payload: LocationPayload) => request<FacilityLocation>(() =>
    httpClient.put(`${BASE}/locations/${record.id}`, { ...payload, expected_version: record.version })),
  archiveLocation: (record: FacilityLocation, reason: string) => request<FacilityLocation>(() =>
    httpClient.post(`${BASE}/locations/${record.id}/archive`, { expected_version: record.version, reason })),
  references: (search?: string) => request<FacilityReferences>(() =>
    httpClient.get(`${BASE}/references`, { params: { search: search?.trim() || undefined } })),
  requests: (params?: { page?: number; per_page?: number; status?: FacilityRequestStatus; priority?: FacilityPriority; location_id?: string; search?: string }) =>
    request<FacilityServiceRequestSummary[]>(() => httpClient.get(`${BASE}/requests`, { params })),
  serviceRequest: (id: string) => request<FacilityServiceRequestRecord>(() => httpClient.get(`${BASE}/requests/${id}`)),
  createRequest: (payload: ServiceRequestPayload) => request<FacilityServiceRequestRecord>(() => httpClient.post(`${BASE}/requests`, payload)),
  cancelRequest: (record: FacilityServiceRequestSummary, reason: string) => request<FacilityServiceRequestRecord>(() =>
    httpClient.post(`${BASE}/requests/${record.id}/cancel`, { expected_version: record.version, reason })),
  closeRequest: (record: FacilityServiceRequestSummary, reason: string) => request<FacilityServiceRequestRecord>(() =>
    httpClient.post(`${BASE}/requests/${record.id}/close`, { expected_version: record.version, reason })),
  workOrders: (params?: { page?: number; per_page?: number; status?: FacilityWorkOrderStatus; assigned_employee_id?: string; location_id?: string; search?: string }) =>
    request<FacilityWorkOrderSummary[]>(() => httpClient.get(`${BASE}/work-orders`, { params })),
  workOrder: (id: string) => request<FacilityWorkOrderRecord>(() => httpClient.get(`${BASE}/work-orders/${id}`)),
  createWorkOrder: (payload: WorkOrderPayload) => request<FacilityWorkOrderRecord>(() => httpClient.post(`${BASE}/work-orders`, payload)),
  startWorkOrder: (record: FacilityWorkOrderSummary) => request<FacilityWorkOrderRecord>(() =>
    httpClient.post(`${BASE}/work-orders/${record.id}/start`, { expected_version: record.version })),
  submitCompletion: (record: FacilityWorkOrderSummary, summary: string) => request<FacilityWorkOrderRecord>(() =>
    httpClient.post(`${BASE}/work-orders/${record.id}/submit-completion`, { expected_version: record.version, summary })),
  cancelWorkOrder: (record: FacilityWorkOrderSummary, reason: string) => request<FacilityWorkOrderRecord>(() =>
    httpClient.post(`${BASE}/work-orders/${record.id}/cancel`, { expected_version: record.version, reason })),
  inspectWorkOrder: (record: FacilityWorkOrderSummary, outcome: FacilityInspectionOutcome, notes: string) => request<FacilityWorkOrderRecord>(() =>
    httpClient.post(`${BASE}/work-orders/${record.id}/inspections`, { expected_version: record.version, outcome, notes })),
};

export function responseMessage(response: ApiEnvelope<unknown>, fallback: string) {
  const first = response.issues?.[0];
  if (typeof first === "string" && first.trim()) return first;
  if (first && typeof first === "object" && first.detail) return first.detail;
  return response.message || fallback;
}

/**
 * HTTP adapter for Hostel workspaces.
 * Every allocation mutation uses a server-generated preview or current record version.
 */

import { AxiosError } from "axios";

import { httpClient } from "@/lib/http-client";

import type {
  Allocation,
  AllocationPreview,
  AllocationsResponse,
  ApiEnvelope,
  HostelReferences,
  PastoralCategory,
  PastoralRecord,
  PastoralRecordsResponse,
  PastoralSeverity,
  Residence,
  ResidencesResponse,
  ResidenceStatus,
  Room,
  RoomsResponse,
  RoomStatus,
} from "./types";

const BASE = "/api/1.0/hostel";
async function request<T>(work: () => Promise<{ data: ApiEnvelope<T> }>): Promise<ApiEnvelope<T>> {
  try {
    return (await work()).data;
  } catch (error) {
    if (error instanceof AxiosError && error.response) return error.response.data as ApiEnvelope<T>;
    throw error;
  }
}

export type HostelListParams = {
  page?: number;
  per_page?: number;
  search?: string;
  status?: string;
  residence_id?: string;
  room_id?: string;
  learner_id?: string;
  category?: string;
};
export type ResidencePayload = { code: string; name: string; description: string | null };
export type RoomPayload = { residence_id: string; code: string; floor_label: string | null; capacity: number };
export type AllocationPreviewPayload = {
  learner_id: string;
  room_id: string;
  starts_on: string;
  expected_end_on: string | null;
  replacing_allocation_id?: string | null;
};
export type PastoralPayload = {
  learner_id: string;
  allocation_id: string | null;
  category: PastoralCategory;
  severity: PastoralSeverity;
  subject: string;
  details: string;
  occurred_at: string;
};

export const hostelService = {
  references: (search?: string) => request<HostelReferences>(() => httpClient.get(`${BASE}/references`, { params: { search: search || undefined } })),
  residences: (params?: HostelListParams) => request<ResidencesResponse>(() => httpClient.get(`${BASE}/residences`, { params })),
  residence: (id: string) => request<Residence>(() => httpClient.get(`${BASE}/residences/${id}`)),
  createResidence: (payload: ResidencePayload) => request<Residence>(() => httpClient.post(`${BASE}/residences`, payload)),
  updateResidence: (record: Residence, payload: ResidencePayload & { status: ResidenceStatus }) => request<Residence>(() => httpClient.put(`${BASE}/residences/${record.id}`, { ...payload, expected_version: record.version })),
  rooms: (params?: HostelListParams) => request<RoomsResponse>(() => httpClient.get(`${BASE}/rooms`, { params })),
  room: (id: string) => request<Room>(() => httpClient.get(`${BASE}/rooms/${id}`)),
  createRoom: (payload: RoomPayload) => request<Room>(() => httpClient.post(`${BASE}/rooms`, payload)),
  updateRoom: (record: Room, payload: Omit<RoomPayload, "residence_id"> & { status: RoomStatus }) => request<Room>(() => httpClient.put(`${BASE}/rooms/${record.id}`, { ...payload, expected_version: record.version })),
  allocationPreview: (payload: AllocationPreviewPayload) => request<AllocationPreview>(() => httpClient.post(`${BASE}/allocations/preview`, payload)),
  allocations: (params?: HostelListParams) => request<AllocationsResponse>(() => httpClient.get(`${BASE}/allocations`, { params })),
  allocation: (id: string) => request<Allocation>(() => httpClient.get(`${BASE}/allocations/${id}`)),
  createAllocation: (payload: AllocationPreviewPayload & { preview_fingerprint: string }) => request<Allocation>(() => httpClient.post(`${BASE}/allocations`, payload)),
  activateAllocation: (record: Allocation) => request<Allocation>(() => httpClient.post(`${BASE}/allocations/${record.id}/activate`, { expected_version: record.version })),
  endAllocation: (record: Allocation, ended_on: string, reason: string) => request<Allocation>(() => httpClient.post(`${BASE}/allocations/${record.id}/end`, { expected_version: record.version, ended_on, reason })),
  cancelAllocation: (record: Allocation, reason: string) => request<Allocation>(() => httpClient.post(`${BASE}/allocations/${record.id}/cancel`, { expected_version: record.version, reason })),
  transferPreview: (record: Allocation, new_room_id: string, effective_on: string) => request<AllocationPreview>(() => httpClient.post(`${BASE}/allocations/${record.id}/transfer-preview`, { expected_version: record.version, new_room_id, effective_on })),
  transferAllocation: (record: Allocation, new_room_id: string, effective_on: string, reason: string, preview_fingerprint: string) => request<Allocation>(() => httpClient.post(`${BASE}/allocations/${record.id}/transfer`, { expected_version: record.version, new_room_id, effective_on, reason, preview_fingerprint })),
  pastoralRecords: (params?: HostelListParams) => request<PastoralRecordsResponse>(() => httpClient.get(`${BASE}/pastoral-records`, { params })),
  pastoralRecord: (id: string) => request<PastoralRecord>(() => httpClient.get(`${BASE}/pastoral-records/${id}`)),
  createPastoralRecord: (payload: PastoralPayload) => request<PastoralRecord>(() => httpClient.post(`${BASE}/pastoral-records`, payload)),
  updatePastoralRecord: (record: PastoralRecord, payload: Omit<PastoralPayload, "learner_id" | "allocation_id">) => request<PastoralRecord>(() => httpClient.put(`${BASE}/pastoral-records/${record.id}`, { ...payload, expected_version: record.version })),
  resolvePastoralRecord: (record: PastoralRecord, resolution: string) => request<PastoralRecord>(() => httpClient.post(`${BASE}/pastoral-records/${record.id}/resolve`, { expected_version: record.version, resolution })),
};

export function responseMessage(response: ApiEnvelope<unknown>, fallback: string) {
  const first = response.issues?.[0];
  if (typeof first === "string" && first.trim()) return first;
  if (first && typeof first === "object" && first.detail) return first.detail;
  return response.message || fallback;
}

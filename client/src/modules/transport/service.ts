/** Typed HTTP boundary for school Transport. */

import { AxiosError } from "axios";
import { httpClient } from "@/lib/http-client";
import type { ApiEnvelope, ManifestExceptionKind, ManifestStatus, RiderAssignment, RiderStatus, RidersResponse, RouteDirection, RoutePayload, RouteRecord, RouteStatus, RoutesResponse, RunRecord, RunsResponse, RunStatus, StopPayload, TransportReferences } from "./types";

const BASE = "/api/1.0/transport";

async function request<T>(work: () => Promise<{ data: ApiEnvelope<T> }>): Promise<ApiEnvelope<T>> {
  try { return (await work()).data; }
  catch (error) { if (error instanceof AxiosError && error.response) return error.response.data as ApiEnvelope<T>; throw error; }
}

export const transportService = {
  references: (search?: string) => request<TransportReferences>(() => httpClient.get(`${BASE}/references`, { params: { search: search?.trim() || undefined } })),
  routes: (params?: { page?: number; per_page?: number; search?: string; status?: RouteStatus; direction?: RouteDirection }) => request<RoutesResponse>(() => httpClient.get(`${BASE}/routes`, { params })),
  route: (id: string) => request<RouteRecord>(() => httpClient.get(`${BASE}/routes/${id}`)),
  createRoute: (payload: RoutePayload) => request<RouteRecord>(() => httpClient.post(`${BASE}/routes`, payload)),
  updateRoute: (id: string, payload: RoutePayload & { status: RouteStatus; expected_version: number }) => request<RouteRecord>(() => httpClient.put(`${BASE}/routes/${id}`, payload)),
  createStop: (routeId: string, payload: StopPayload) => request<RouteRecord>(() => httpClient.post(`${BASE}/routes/${routeId}/stops`, payload)),
  updateStop: (routeId: string, stopId: string, payload: StopPayload & { expected_version: number }) => request<RouteRecord>(() => httpClient.put(`${BASE}/routes/${routeId}/stops/${stopId}`, payload)),
  removeStop: (routeId: string, stopId: string, expectedVersion: number) => request<RouteRecord>(() => httpClient.post(`${BASE}/routes/${routeId}/stops/${stopId}/remove`, { expected_version: expectedVersion })),
  riders: (params?: { page?: number; per_page?: number; search?: string; route_id?: string; status?: RiderStatus; on_date?: string }) => request<RidersResponse>(() => httpClient.get(`${BASE}/riders`, { params })),
  assignRider: (payload: { learner_id: string; route_id: string; boarding_stop_id: string; alighting_stop_id: string; effective_from: string; effective_until: string | null }) => request<RiderAssignment>(() => httpClient.post(`${BASE}/riders`, payload)),
  endRider: (id: string, effectiveUntil: string, reason: string, expectedVersion: number) => request<RiderAssignment>(() => httpClient.post(`${BASE}/riders/${id}/end`, { effective_until: effectiveUntil, reason, expected_version: expectedVersion })),
  runs: (params?: { page?: number; per_page?: number; route_id?: string; status?: RunStatus; date_from?: string; date_to?: string }) => request<RunsResponse>(() => httpClient.get(`${BASE}/runs`, { params })),
  run: (id: string) => request<RunRecord>(() => httpClient.get(`${BASE}/runs/${id}`)),
  createRun: (payload: { route_id: string; service_date: string; vehicle_id: string; driver_id: string }) => request<RunRecord>(() => httpClient.post(`${BASE}/runs`, payload)),
  transitionRun: (id: string, transition: "boarding" | "depart" | "complete", expectedVersion: number) => request<RunRecord>(() => httpClient.post(`${BASE}/runs/${id}/${transition}`, { expected_version: expectedVersion })),
  cancelRun: (id: string, reason: string, expectedVersion: number) => request<RunRecord>(() => httpClient.post(`${BASE}/runs/${id}/cancel`, { reason, expected_version: expectedVersion })),
  markManifest: (runId: string, entryId: string, payload: { status: ManifestStatus; exception_kind: ManifestExceptionKind | null; note: string | null; expected_version: number }) => request<RunRecord>(() => httpClient.put(`${BASE}/runs/${runId}/manifest/${entryId}`, payload)),
};

export function responseMessage(response: ApiEnvelope<unknown>, fallback: string) {
  const first = response.issues?.[0];
  if (typeof first === "string" && first.trim()) return first;
  if (first && typeof first === "object" && first.detail) return first.detail;
  return response.message || fallback;
}


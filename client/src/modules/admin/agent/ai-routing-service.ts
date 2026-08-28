/**
 * HTTP boundary for tenant Agent routing. Route targets contain connection and
 * model identifiers only; provider credentials never enter this client.
 */

import { AxiosError } from "axios";

import { httpClient } from "@/lib/http-client";
import type { ApiEnvelope } from "@/modules/users/types";

import type {
  AiTaskRoute,
  ArchivedAiTaskRoute,
  AiRoutingOptions,
  CreateAiTaskRouteInput,
  ResolvedAiTaskRoute,
  ResolveAiTaskRouteInput,
  UpdateAiTaskRouteInput,
} from "./types";

const BASE_URL = "/api/1.0/ai/routes";

async function request<T>(work: () => Promise<{ data: ApiEnvelope<T> }>): Promise<ApiEnvelope<T>> {
  try {
    return (await work()).data;
  } catch (error) {
    if (error instanceof AxiosError && error.response) {
      return error.response.data as ApiEnvelope<T>;
    }
    throw error;
  }
}

export const aiRoutingService = {
  listRoutes: () => request<AiTaskRoute[]>(() => httpClient.get(BASE_URL)),

  listOptions: () => request<AiRoutingOptions>(() => httpClient.get(`${BASE_URL}/options`)),

  getRoute: (routeId: string) =>
    request<AiTaskRoute>(() => httpClient.get(`${BASE_URL}/${routeId}`)),

  createRoute: (input: CreateAiTaskRouteInput) =>
    request<AiTaskRoute>(() => httpClient.post(BASE_URL, input)),

  updateRoute: (routeId: string, input: UpdateAiTaskRouteInput) =>
    request<AiTaskRoute>(() => httpClient.put(`${BASE_URL}/${routeId}`, input)),

  archiveRoute: (routeId: string, expectedVersion: number, auditReason: string) =>
    request<ArchivedAiTaskRoute>(() =>
      httpClient.delete(`${BASE_URL}/${routeId}`, {
        params: { expectedVersion, auditReason },
      }),
    ),

  resolveRoute: (input: ResolveAiTaskRouteInput) =>
    request<ResolvedAiTaskRoute>(() => httpClient.post(`${BASE_URL}/resolve`, input)),
};

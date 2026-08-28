/**
 * HTTP boundary for Administration AI provider operations.
 * Secrets are accepted only for create/rotate calls and never cached or returned.
 */

import { AxiosError } from "axios";

import { httpClient } from "@/lib/http-client";
import type { ApiEnvelope } from "@/modules/users/types";

import type {
  AiProviderConnection,
  CreateProviderConnectionInput,
  ProviderCatalogEntry,
  ProviderModelSnapshot,
  ProviderTestOutcome,
  RotateProviderCredentialInput,
  UpdateProviderConnectionInput,
} from "./types";

const BASE_URL = "/api/1.0/ai";

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

export const aiProviderService = {
  listProviders: () =>
    request<ProviderCatalogEntry[]>(() => httpClient.get(`${BASE_URL}/providers`)),

  listConnections: () =>
    request<AiProviderConnection[]>(() => httpClient.get(`${BASE_URL}/connections`)),

  getConnection: (connectionId: string) =>
    request<AiProviderConnection>(() => httpClient.get(`${BASE_URL}/connections/${connectionId}`)),

  createConnection: (input: CreateProviderConnectionInput) =>
    request<AiProviderConnection>(() => httpClient.post(`${BASE_URL}/connections`, input)),

  updateConnection: (connectionId: string, input: UpdateProviderConnectionInput) =>
    request<AiProviderConnection>(() => httpClient.put(`${BASE_URL}/connections/${connectionId}`, input)),

  rotateCredential: (connectionId: string, input: RotateProviderCredentialInput) =>
    request<AiProviderConnection>(() =>
      httpClient.post(`${BASE_URL}/connections/${connectionId}/credentials/rotate`, input),
    ),

  testConnection: (connectionId: string, expectedVersion: number) =>
    request<ProviderTestOutcome>(() =>
      httpClient.post(`${BASE_URL}/connections/${connectionId}/test`, {
        expected_version: expectedVersion,
      }),
    ),

  listModels: (connectionId: string) =>
    request<ProviderModelSnapshot>(() => httpClient.get(`${BASE_URL}/connections/${connectionId}/models`)),

  refreshModels: (connectionId: string, expectedVersion: number) =>
    request<ProviderModelSnapshot>(() =>
      httpClient.post(`${BASE_URL}/connections/${connectionId}/models/refresh`, {
        expected_version: expectedVersion,
      }),
    ),

  disconnect: (connectionId: string, expectedVersion: number) =>
    request<{ disconnected_id: string }>(() =>
      httpClient.delete(`${BASE_URL}/connections/${connectionId}`, {
        params: { expectedVersion },
      }),
    ),
};

export function aiProviderErrorMessage(
  response: Pick<ApiEnvelope<unknown>, "issues" | "message">,
  fallback: string,
) {
  const issue = response.issues?.[0];
  if (typeof issue === "string") return issue;
  return issue?.detail || response.message || fallback;
}


/**
 * Owns Agent Administration HTTP reads and bounded CSV export.
 * JSON responses retain their HTTP status so pages can distinguish forbidden access from a
 * transient read failure; exported files contain only the server's reduced usage projection.
 */

import { AxiosError } from "axios";

import { httpClient } from "@/lib/http-client";
import type { ApiEnvelope } from "@/modules/users/types";

import type {
  AgentCapabilityFilters,
  AgentCapabilityInventoryPage,
  AgentReadiness,
  AgentRunAuditDetail,
  AgentRunAuditPage,
  AgentRunFilters,
  AgentUsageFilterOptions,
  AgentUsageFilters,
  AgentUsageReport,
} from "./types";

const BASE_URL = "/api/1.0/agent-governance";

export type GovernanceResponse<T> = ApiEnvelope<T> & { http_status?: number };

async function request<T>(
  work: () => Promise<{ data: ApiEnvelope<T>; status: number }>,
): Promise<GovernanceResponse<T>> {
  try {
    const response = await work();
    return { ...response.data, http_status: response.status };
  } catch (error) {
    if (error instanceof AxiosError && error.response) {
      return {
        ...(error.response.data as ApiEnvelope<T>),
        http_status: error.response.status,
      };
    }
    throw error;
  }
}

export const agentGovernanceService = {
  readiness: () =>
    request<AgentReadiness>(() => httpClient.get(`${BASE_URL}/readiness`)),

  capabilities: (filters: AgentCapabilityFilters) =>
    request<AgentCapabilityInventoryPage>(() =>
      httpClient.get(`${BASE_URL}/capabilities`, { params: compactParams(filters) }),
    ),

  usageOptions: () =>
    request<AgentUsageFilterOptions>(() => httpClient.get(`${BASE_URL}/usage/options`)),

  usage: (filters: AgentUsageFilters) =>
    request<AgentUsageReport>(() =>
      httpClient.get(`${BASE_URL}/usage`, { params: compactParams(filters) }),
    ),

  runs: (filters: AgentRunFilters) =>
    request<AgentRunAuditPage>(() =>
      httpClient.get(`${BASE_URL}/runs`, { params: compactParams(filters) }),
    ),

  run: (runId: string) =>
    request<AgentRunAuditDetail>(() => httpClient.get(`${BASE_URL}/runs/${runId}`)),

  exportUsage: async (filters: AgentUsageFilters) => {
    const response = await httpClient.getInstance().get<Blob>(`${BASE_URL}/usage/export`, {
      params: compactParams(filters),
      responseType: "blob",
    });
    return {
      blob: response.data,
      truncated: response.headers["x-export-truncated"] === "true",
    };
  },
};

export function governanceErrorMessage(
  response: Pick<GovernanceResponse<unknown>, "issues" | "message">,
  fallback: string,
) {
  const issue = response.issues?.[0];
  if (typeof issue === "string") return issue;
  return issue?.detail || response.message || fallback;
}

export function isGovernanceForbidden(response: Pick<GovernanceResponse<unknown>, "http_status">) {
  return response.http_status === 403;
}

function compactParams(filters: object) {
  return Object.fromEntries(
    Object.entries(filters).filter(([, value]) => value !== undefined && value !== ""),
  );
}

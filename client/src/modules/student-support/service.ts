/** Typed HTTP boundary for Student Support. */

import { AxiosError } from "axios";

import { httpClient } from "@/lib/http-client";

import type {
  ApiEnvelope,
  CaseAction,
  CaseActionKind,
  CaseActionsResponse,
  CasePayload,
  CaseRecord,
  CasesResponse,
  CaseStatus,
  CaseTeamRole,
  ConcernCategory,
  CaseSeverity,
  StudentSupportReferences,
  UpdateCasePayload,
} from "./types";

const BASE = "/api/1.0/student-support";

async function request<T>(work: () => Promise<{ data: ApiEnvelope<T> }>): Promise<ApiEnvelope<T>> {
  try {
    return (await work()).data;
  } catch (error) {
    if (error instanceof AxiosError && error.response) return error.response.data as ApiEnvelope<T>;
    throw error;
  }
}

export interface CaseListParams {
  page?: number;
  per_page?: number;
  search?: string;
  status?: CaseStatus;
  category?: ConcernCategory;
  severity?: CaseSeverity;
  learner_id?: string;
}

export const studentSupportService = {
  references: (search?: string) => request<StudentSupportReferences>(() =>
    httpClient.get(`${BASE}/references`, { params: { search: search?.trim() || undefined } }),
  ),
  cases: (params?: CaseListParams) => request<CasesResponse>(() => httpClient.get(`${BASE}/cases`, { params })),
  case: (id: string) => request<CaseRecord>(() => httpClient.get(`${BASE}/cases/${id}`)),
  createCase: (payload: CasePayload) => request<CaseRecord>(() => httpClient.post(`${BASE}/cases`, payload)),
  updateCase: (id: string, payload: UpdateCasePayload) => request<CaseRecord>(() => httpClient.put(`${BASE}/cases/${id}`, payload)),
  actions: (caseId: string) => request<CaseActionsResponse>(() => httpClient.get(`${BASE}/cases/${caseId}/actions`)),
  createAction: (caseId: string, payload: { action_kind: CaseActionKind; summary: string; details: string | null; occurred_at: string; expected_version: number }) =>
    request<CaseAction>(() => httpClient.post(`${BASE}/cases/${caseId}/actions`, payload)),
  assignTeamMember: (caseId: string, userId: string, memberRole: CaseTeamRole, expectedVersion: number) =>
    request<CaseRecord>(() => httpClient.post(`${BASE}/cases/${caseId}/team`, { user_id: userId, member_role: memberRole, expected_version: expectedVersion })),
  removeTeamMember: (caseId: string, userId: string, expectedVersion: number) =>
    request<CaseRecord>(() => httpClient.post(`${BASE}/cases/${caseId}/team/${userId}/remove`, undefined, { params: { expected_version: expectedVersion } })),
  transition: (caseId: string, action: "escalate" | "resolve" | "close", reason: string, expectedVersion: number) =>
    request<CaseRecord>(() => httpClient.post(`${BASE}/cases/${caseId}/${action}`, { reason, expected_version: expectedVersion })),
};

export function responseMessage(response: ApiEnvelope<unknown>, fallback: string) {
  const first = response.issues?.[0];
  if (typeof first === "string" && first.trim()) return first;
  if (first && typeof first === "object" && first.detail) return first.detail;
  return response.message || fallback;
}

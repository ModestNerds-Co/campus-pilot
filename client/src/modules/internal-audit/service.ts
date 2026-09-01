// HTTP adapter for exact Internal Audit operations.

import { AxiosError } from "axios";

import { httpClient } from "@/lib/http-client";

import type {
  ApiEnvelope,
  AuditEngagement,
  AuditFinding,
  AuditPlan,
  AuditorCandidate,
  EngagementPayload,
  EngagementsResponse,
  EvidenceResponse,
  FindingPayload,
  FindingRating,
  FindingsResponse,
  NumberingPolicy,
  PlanPayload,
  PlansResponse,
} from "./types";

const BASE = "/api/1.0/internal-audit";

async function request<T>(work: () => Promise<{ data: ApiEnvelope<T> }>): Promise<ApiEnvelope<T>> {
  try {
    return (await work()).data;
  } catch (error) {
    if (error instanceof AxiosError && error.response) return error.response.data as ApiEnvelope<T>;
    throw error;
  }
}

export interface AuditListParams {
  page?: number;
  per_page?: number;
  search?: string;
  status?: string;
  plan_id?: string;
  engagement_id?: string;
  rating?: FindingRating;
}

export const internalAuditService = {
  numbering: () => request<NumberingPolicy>(() => httpClient.get(`${BASE}/numbering-policy`)),
  updateNumbering: (record: NumberingPolicy, values: Omit<NumberingPolicy, "next_plan_reference" | "next_engagement_reference" | "next_finding_reference" | "version" | "updated_at">) =>
    request<NumberingPolicy>(() => httpClient.put(`${BASE}/numbering-policy`, { ...values, version: record.version })),

  plans: (params?: AuditListParams) => request<PlansResponse>(() => httpClient.get(`${BASE}/plans`, { params })),
  plan: (id: string) => request<AuditPlan>(() => httpClient.get(`${BASE}/plans/${id}`)),
  createPlan: (payload: PlanPayload) => request<AuditPlan>(() => httpClient.post(`${BASE}/plans`, payload)),
  updatePlan: (record: AuditPlan, payload: PlanPayload) => request<AuditPlan>(() => httpClient.put(`${BASE}/plans/${record.id}`, { ...payload, expected_version: record.version })),
  deletePlan: (record: AuditPlan) => request<{ deleted: boolean }>(() => httpClient.delete(`${BASE}/plans/${record.id}`, { params: { expected_version: record.version } })),
  approvePlan: (record: AuditPlan) => request<AuditPlan>(() => httpClient.post(`${BASE}/plans/${record.id}/approve`, { expected_version: record.version })),
  closePlan: (record: AuditPlan, summary: string) => request<AuditPlan>(() => httpClient.post(`${BASE}/plans/${record.id}/close`, { expected_version: record.version, summary })),

  auditors: (search?: string) => request<AuditorCandidate[]>(() => httpClient.get(`${BASE}/auditor-candidates`, { params: { search: search || undefined } })),
  engagements: (params?: AuditListParams) => request<EngagementsResponse>(() => httpClient.get(`${BASE}/engagements`, { params })),
  engagement: (id: string) => request<AuditEngagement>(() => httpClient.get(`${BASE}/engagements/${id}`)),
  createEngagement: (payload: EngagementPayload) => request<AuditEngagement>(() => httpClient.post(`${BASE}/engagements`, payload)),
  updateEngagement: (record: AuditEngagement, payload: EngagementPayload) => request<AuditEngagement>(() => httpClient.put(`${BASE}/engagements/${record.id}`, {
    title: payload.title,
    objective: payload.objective,
    scope_text: payload.scope_text,
    lead_auditor_user_id: payload.lead_auditor_user_id,
    starts_on: payload.starts_on,
    due_on: payload.due_on,
    expected_version: record.version,
  })),
  deleteEngagement: (record: AuditEngagement) => request<{ deleted: boolean }>(() => httpClient.delete(`${BASE}/engagements/${record.id}`, { params: { expected_version: record.version } })),
  startEngagement: (record: AuditEngagement) => request<AuditEngagement>(() => httpClient.post(`${BASE}/engagements/${record.id}/start`, { expected_version: record.version })),
  beginReporting: (record: AuditEngagement) => request<AuditEngagement>(() => httpClient.post(`${BASE}/engagements/${record.id}/begin-reporting`, { expected_version: record.version })),
  closeEngagement: (record: AuditEngagement, summary: string) => request<AuditEngagement>(() => httpClient.post(`${BASE}/engagements/${record.id}/close`, { expected_version: record.version, summary })),

  evidence: (engagementId: string) => request<EvidenceResponse>(() => httpClient.get(`${BASE}/engagements/${engagementId}/evidence`)),
  linkEvidence: (engagementId: string, documentFileId: string, purpose: string) => request<EvidenceResponse["evidence"][number]>(() => httpClient.post(`${BASE}/engagements/${engagementId}/evidence`, { document_file_id: documentFileId, purpose })),

  findings: (params?: AuditListParams) => request<FindingsResponse>(() => httpClient.get(`${BASE}/findings`, { params })),
  finding: (id: string) => request<AuditFinding>(() => httpClient.get(`${BASE}/findings/${id}`)),
  createFinding: (engagementId: string, payload: FindingPayload) => request<AuditFinding>(() => httpClient.post(`${BASE}/engagements/${engagementId}/findings`, payload)),
  updateFinding: (record: AuditFinding, payload: FindingPayload) => request<AuditFinding>(() => httpClient.put(`${BASE}/findings/${record.id}`, { ...payload, expected_version: record.version })),
  deleteFinding: (record: AuditFinding) => request<{ deleted: boolean }>(() => httpClient.delete(`${BASE}/findings/${record.id}`, { params: { expected_version: record.version } })),
  issueFinding: (record: AuditFinding) => request<AuditFinding>(() => httpClient.post(`${BASE}/findings/${record.id}/issue`, { expected_version: record.version })),
};

export function responseMessage(response: ApiEnvelope<unknown>, fallback: string) {
  const first = response.issues?.[0];
  if (typeof first === "string" && first.trim()) return first;
  if (first && typeof first === "object" && first.detail) return first.detail;
  return response.message || fallback;
}

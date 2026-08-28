import { AxiosError } from "axios";

import { httpClient } from "@/lib/http-client";

import type {
  AccountCandidatesResponse, ApiEnvelope, Application, ApplicationInput, ApplicationsResponse,
  Enrolment, EnrolmentInput, EnrolmentsResponse, Guardian, GuardianInput,
  GuardianRelationship, GuardianRelationshipInput, GuardianRelationshipsResponse,
  GuardiansResponse, Learner, LearnerInput, LearnersResponse, ListParams,
  LearnerNumberingPolicy, LearnerNumberingPolicyInput,
  SisImportCommit, SisImportMapping, SisImportPreview, SisImportRecord,
  SisImportsResponse, SisImportTarget,
} from "./types";

const BASE_URL = "/api/1.0/sis";

async function request<T>(work: () => Promise<{ data: ApiEnvelope<T> }>): Promise<ApiEnvelope<T>> {
  try {
    return (await work()).data;
  } catch (error) {
    if (error instanceof AxiosError && error.response) return error.response.data as ApiEnvelope<T>;
    throw error;
  }
}

export const sisService = {
  getLearnerNumberingPolicy: () => request<LearnerNumberingPolicy>(() => httpClient.get(`${BASE_URL}/learner-numbering`)),
  updateLearnerNumberingPolicy: (data: LearnerNumberingPolicyInput) => request<LearnerNumberingPolicy>(() => httpClient.put(`${BASE_URL}/learner-numbering`, data)),

  listImports: (params?: { page?: number; per_page?: number; target?: SisImportTarget }) => request<SisImportsResponse>(() => httpClient.get(`${BASE_URL}/imports`, { params })),
  uploadImport: (target: SisImportTarget, file: File) => {
    const form = new FormData();
    form.append("target", target);
    form.append("file", file);
    return request<SisImportRecord>(() => httpClient.post(`${BASE_URL}/imports`, form));
  },
  getImport: (id: string) => request<SisImportRecord>(() => httpClient.get(`${BASE_URL}/imports/${id}`)),
  createImportPreview: (id: string, mapping: SisImportMapping) => request<SisImportPreview>(() => httpClient.put(`${BASE_URL}/imports/${id}/mapping`, mapping)),
  getImportPreview: (id: string, params?: { page?: number; per_page?: number }) => request<SisImportPreview>(() => httpClient.get(`${BASE_URL}/imports/${id}/preview`, { params })),
  commitImport: (id: string, previewId: string) => request<SisImportCommit>(() => httpClient.post(`${BASE_URL}/imports/${id}/commit`, { preview_id: previewId })),

  listLearners: (params?: ListParams) => request<LearnersResponse>(() => httpClient.get(`${BASE_URL}/learners`, { params })),
  createLearner: (data: LearnerInput) => request<Learner>(() => httpClient.post(`${BASE_URL}/learners`, data)),
  updateLearner: (id: string, data: LearnerInput) => request<Learner>(() => httpClient.put(`${BASE_URL}/learners/${id}`, data)),
  linkLearnerAccount: (id: string, accountId: string | null) => request<Learner>(() => httpClient.put(`${BASE_URL}/learners/${id}/account`, { account_id: accountId })),
  deleteLearner: (id: string) => request<{ deleted: boolean }>(() => httpClient.delete(`${BASE_URL}/learners/${id}`)),

  listGuardians: (params?: ListParams) => request<GuardiansResponse>(() => httpClient.get(`${BASE_URL}/guardians`, { params })),
  createGuardian: (data: GuardianInput) => request<Guardian>(() => httpClient.post(`${BASE_URL}/guardians`, data)),
  updateGuardian: (id: string, data: GuardianInput) => request<Guardian>(() => httpClient.put(`${BASE_URL}/guardians/${id}`, data)),
  linkGuardianAccount: (id: string, accountId: string | null) => request<Guardian>(() => httpClient.put(`${BASE_URL}/guardians/${id}/account`, { account_id: accountId })),
  deleteGuardian: (id: string) => request<{ deleted: boolean }>(() => httpClient.delete(`${BASE_URL}/guardians/${id}`)),

  listAccountCandidates: (profileKind: "learner" | "guardian", profileId?: string, search?: string) => request<AccountCandidatesResponse>(() => httpClient.get(`${BASE_URL}/account-candidates`, { params: { profile_kind: profileKind, profile_id: profileId, search: search || undefined } })),

  listGuardianRelationships: (params?: ListParams) => request<GuardianRelationshipsResponse>(() => httpClient.get(`${BASE_URL}/guardian-relationships`, { params })),
  createGuardianRelationship: (data: GuardianRelationshipInput) => request<GuardianRelationship>(() => httpClient.post(`${BASE_URL}/guardian-relationships`, data)),
  updateGuardianRelationship: (id: string, data: Omit<GuardianRelationshipInput, "learner_id" | "guardian_id">) => request<GuardianRelationship>(() => httpClient.put(`${BASE_URL}/guardian-relationships/${id}`, data)),
  deleteGuardianRelationship: (id: string) => request<{ deleted: boolean }>(() => httpClient.delete(`${BASE_URL}/guardian-relationships/${id}`)),

  listApplications: (params?: ListParams) => request<ApplicationsResponse>(() => httpClient.get(`${BASE_URL}/applications`, { params })),
  createApplication: (data: ApplicationInput) => request<Application>(() => httpClient.post(`${BASE_URL}/applications`, data)),
  updateApplication: (id: string, data: ApplicationInput) => request<Application>(() => httpClient.put(`${BASE_URL}/applications/${id}`, data)),
  deleteApplication: (id: string) => request<{ deleted: boolean }>(() => httpClient.delete(`${BASE_URL}/applications/${id}`)),

  listEnrolments: (params?: ListParams) => request<EnrolmentsResponse>(() => httpClient.get(`${BASE_URL}/enrolments`, { params })),
  createEnrolment: (data: EnrolmentInput) => request<Enrolment>(() => httpClient.post(`${BASE_URL}/enrolments`, data)),
  updateEnrolment: (id: string, data: EnrolmentInput) => request<Enrolment>(() => httpClient.put(`${BASE_URL}/enrolments/${id}`, data)),
};

export function responseMessage(response: Pick<ApiEnvelope<unknown>, "issues" | "message">, fallback: string) {
  const issue = response.issues?.[0];
  if (typeof issue === "string") return issue;
  return issue?.detail || response.message || fallback;
}

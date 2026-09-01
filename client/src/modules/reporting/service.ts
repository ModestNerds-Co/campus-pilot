import { AxiosError } from "axios";

import { httpClient } from "@/lib/http-client";

import type {
  AcademicReportBatchStatus, ApiEnvelope, GradingBandInput, GradingScheme,
  LearnerTranscript, ProgressionOutcome, ReportBatch, ReportBatchesResponse,
  ReportingReferenceData,
} from "./types";

const BASE_URL = "/api/1.0/academics/reporting";

async function request<T>(work: () => Promise<{ data: ApiEnvelope<T> }>): Promise<ApiEnvelope<T>> {
  try {
    return (await work()).data;
  } catch (error) {
    if (error instanceof AxiosError && error.response) return error.response.data as ApiEnvelope<T>;
    throw error;
  }
}

export const reportingService = {
  references: () => request<ReportingReferenceData>(() => httpClient.get(`${BASE_URL}/references`)),
  listGradingSchemes: (status?: "active" | "retired") => request<GradingScheme[]>(() => httpClient.get(`${BASE_URL}/grading-schemes`, { params: { status } })),
  createGradingScheme: (payload: { name: string; description: string | null; is_default: boolean; bands: GradingBandInput[] }) => request<GradingScheme>(() => httpClient.post(`${BASE_URL}/grading-schemes`, payload)),
  updateGradingScheme: (id: string, payload: { expected_version: number; name: string; description: string | null; is_default: boolean; bands: GradingBandInput[] }) => request<GradingScheme>(() => httpClient.put(`${BASE_URL}/grading-schemes/${id}`, payload)),
  retireGradingScheme: (id: string, expectedVersion: number) => request<GradingScheme>(() => httpClient.post(`${BASE_URL}/grading-schemes/${id}/retire`, { expected_version: expectedVersion })),
  deleteGradingScheme: (id: string, expectedVersion: number) => request<{ deleted: boolean }>(() => httpClient.delete(`${BASE_URL}/grading-schemes/${id}`, { params: { expected_version: expectedVersion } })),
  listReportBatches: (params?: { page?: number; per_page?: number; status?: AcademicReportBatchStatus }) => request<ReportBatchesResponse>(() => httpClient.get(`${BASE_URL}/report-batches`, { params })),
  generateReportBatch: (payload: { assessment_cycle_id: string; class_group_id: string; grading_scheme_id: string; idempotency_key: string }) => request<ReportBatch>(() => httpClient.post(`${BASE_URL}/report-batches`, payload)),
  readReportBatch: (id: string) => request<ReportBatch>(() => httpClient.get(`${BASE_URL}/report-batches/${id}`)),
  updateTeacherComment: (id: string, expectedVersion: number, teacherComment: string | null) => request<ReportBatch>(() => httpClient.put(`${BASE_URL}/report-cards/${id}/teacher-comment`, { expected_version: expectedVersion, teacher_comment: teacherComment })),
  updateReportReview: (id: string, payload: { expected_version: number; reviewer_comment: string | null; progression_outcome: ProgressionOutcome; target_grade_level_id: string | null }) => request<ReportBatch>(() => httpClient.put(`${BASE_URL}/report-cards/${id}/review`, payload)),
  reviewReportBatch: (id: string, expectedVersion: number) => request<ReportBatch>(() => httpClient.post(`${BASE_URL}/report-batches/${id}/review`, { expected_version: expectedVersion })),
  publishReportBatch: (id: string, expectedVersion: number) => request<ReportBatch>(() => httpClient.post(`${BASE_URL}/report-batches/${id}/publish`, { expected_version: expectedVersion })),
  reopenReportBatch: (id: string, expectedVersion: number, reason: string) => request<ReportBatch>(() => httpClient.post(`${BASE_URL}/report-batches/${id}/reopen`, { expected_version: expectedVersion, reason })),
  deleteReportBatch: (id: string, expectedVersion: number) => request<{ deleted: boolean }>(() => httpClient.delete(`${BASE_URL}/report-batches/${id}`, { params: { expected_version: expectedVersion } })),
  learnerTranscript: (learnerId: string) => request<LearnerTranscript>(() => httpClient.get(`${BASE_URL}/learners/${learnerId}/transcript`)),
};

export function responseMessage(response: Pick<ApiEnvelope<unknown>, "issues" | "message">, fallback: string) {
  const issue = response.issues?.[0];
  if (typeof issue === "string") return issue;
  return issue?.detail || response.message || fallback;
}

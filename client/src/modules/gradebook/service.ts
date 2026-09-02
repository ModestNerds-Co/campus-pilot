import { AxiosError } from "axios";

import { httpClient } from "@/lib/http-client";

import type {
  ApiEnvelope,
  GradebookMarkInput,
  GradebookMarkImportCommit,
  GradebookMarkImportMapping,
  GradebookMarkImportPreview,
  GradebookMarkImportRecord,
  GradebookMarkImportsResponse,
  GradebookReferenceData,
  GradebookSheet,
  GradebookSheetsResponse,
  GradebookSheetStatus,
} from "./types";

const BASE_URL = "/api/1.0/academics/gradebook";

async function request<T>(work: () => Promise<{ data: ApiEnvelope<T> }>): Promise<ApiEnvelope<T>> {
  try {
    return (await work()).data;
  } catch (error) {
    if (error instanceof AxiosError && error.response) return error.response.data as ApiEnvelope<T>;
    throw error;
  }
}

export const gradebookService = {
  references: () => request<GradebookReferenceData>(() => httpClient.get(`${BASE_URL}/references`)),
  listMarkSheets: (params?: { page?: number; per_page?: number; status?: GradebookSheetStatus }) => request<GradebookSheetsResponse>(() => httpClient.get(`${BASE_URL}/mark-sheets`, { params })),
  createMarkSheet: (assessmentComponentId: string, rosterOn: string) => request<GradebookSheet>(() => httpClient.post(`${BASE_URL}/mark-sheets`, {
    assessment_component_id: assessmentComponentId,
    roster_on: rosterOn,
    idempotency_key: crypto.randomUUID(),
  })),
  readMarkSheet: (id: string) => request<GradebookSheet>(() => httpClient.get(`${BASE_URL}/mark-sheets/${id}`)),
  updateMarks: (id: string, expectedVersion: number, marks: GradebookMarkInput[]) => request<GradebookSheet>(() => httpClient.put(`${BASE_URL}/mark-sheets/${id}/marks`, { expected_version: expectedVersion, marks })),
  submitMarkSheet: (id: string, expectedVersion: number) => request<GradebookSheet>(() => httpClient.post(`${BASE_URL}/mark-sheets/${id}/submit`, { expected_version: expectedVersion })),
  publishMarkSheet: (id: string, expectedVersion: number) => request<GradebookSheet>(() => httpClient.post(`${BASE_URL}/mark-sheets/${id}/publish`, { expected_version: expectedVersion })),
  reopenMarkSheet: (id: string, expectedVersion: number, reason: string) => request<GradebookSheet>(() => httpClient.post(`${BASE_URL}/mark-sheets/${id}/reopen`, { expected_version: expectedVersion, reason })),
  deleteMarkSheet: (id: string, expectedVersion: number) => request<{ deleted: boolean }>(() => httpClient.delete(`${BASE_URL}/mark-sheets/${id}`, { params: { expected_version: expectedVersion } })),
  listMarkImports: (markSheetId: string, params?: { page?: number; per_page?: number }) => request<GradebookMarkImportsResponse>(() => httpClient.get(`${BASE_URL}/mark-sheets/${markSheetId}/imports`, { params })),
  uploadMarkImport: (markSheetId: string, file: File) => {
    const form = new FormData();
    form.append("file", file);
    return request<GradebookMarkImportRecord>(() => httpClient.post(`${BASE_URL}/mark-sheets/${markSheetId}/imports`, form));
  },
  readMarkImport: (markSheetId: string, importId: string) => request<GradebookMarkImportRecord>(() => httpClient.get(`${BASE_URL}/mark-sheets/${markSheetId}/imports/${importId}`)),
  createMarkImportPreview: (markSheetId: string, importId: string, mapping: GradebookMarkImportMapping) => request<GradebookMarkImportPreview>(() => httpClient.put(`${BASE_URL}/mark-sheets/${markSheetId}/imports/${importId}/mapping`, mapping)),
  readMarkImportPreview: (markSheetId: string, importId: string, params?: { page?: number; per_page?: number }) => request<GradebookMarkImportPreview>(() => httpClient.get(`${BASE_URL}/mark-sheets/${markSheetId}/imports/${importId}/preview`, { params })),
  commitMarkImport: (markSheetId: string, importId: string, previewId: string) => request<GradebookMarkImportCommit>(() => httpClient.post(`${BASE_URL}/mark-sheets/${markSheetId}/imports/${importId}/commit`, { preview_id: previewId })),
};

export function responseMessage(response: Pick<ApiEnvelope<unknown>, "issues" | "message">, fallback: string) {
  const issue = response.issues?.[0];
  if (typeof issue === "string") return issue;
  return issue?.detail || response.message || fallback;
}

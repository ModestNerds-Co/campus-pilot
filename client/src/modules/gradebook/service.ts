import { AxiosError } from "axios";

import { httpClient } from "@/lib/http-client";

import type {
  ApiEnvelope,
  GradebookMarkInput,
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
};

export function responseMessage(response: Pick<ApiEnvelope<unknown>, "issues" | "message">, fallback: string) {
  const issue = response.issues?.[0];
  if (typeof issue === "string") return issue;
  return issue?.detail || response.message || fallback;
}

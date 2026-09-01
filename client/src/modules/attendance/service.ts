// Campus Pilot Attendance HTTP client.

import { AxiosError } from "axios";

import { httpClient } from "@/lib/http-client";

import type {
  ApiEnvelope,
  AttendanceMarkInput,
  AttendanceReferenceData,
  AttendanceRegister,
  AttendanceRegisterInput,
  AttendanceRegisterListParams,
  AttendanceRegistersResponse,
  LearnerAttendanceHistory,
  LearnerAttendanceHistoryParams,
} from "./types";

const BASE_URL = "/api/1.0/attendance";

async function request<T>(work: () => Promise<{ data: ApiEnvelope<T> }>): Promise<ApiEnvelope<T>> {
  try {
    return (await work()).data;
  } catch (error) {
    if (error instanceof AxiosError && error.response) return error.response.data as ApiEnvelope<T>;
    throw error;
  }
}

export const attendanceService = {
  references: () => request<AttendanceReferenceData | null>(() => httpClient.get(`${BASE_URL}/references`)),
  listRegisters: (params?: AttendanceRegisterListParams) => request<AttendanceRegistersResponse>(() => httpClient.get(`${BASE_URL}/registers`, { params })),
  createRegister: (data: AttendanceRegisterInput) => request<AttendanceRegister>(() => httpClient.post(`${BASE_URL}/registers`, data)),
  readRegister: (id: string) => request<AttendanceRegister>(() => httpClient.get(`${BASE_URL}/registers/${id}`)),
  learnerHistory: (id: string, params?: LearnerAttendanceHistoryParams) => request<LearnerAttendanceHistory>(() => httpClient.get(`${BASE_URL}/learners/${id}/history`, { params })),
  updateMarks: (id: string, expectedVersion: number, marks: AttendanceMarkInput[]) => request<AttendanceRegister>(() => httpClient.put(`${BASE_URL}/registers/${id}/marks`, { expected_version: expectedVersion, marks })),
  submitRegister: (id: string, expectedVersion: number) => request<AttendanceRegister>(() => httpClient.post(`${BASE_URL}/registers/${id}/submit`, { expected_version: expectedVersion })),
  reopenRegister: (id: string, expectedVersion: number, reason: string) => request<AttendanceRegister>(() => httpClient.post(`${BASE_URL}/registers/${id}/reopen`, { expected_version: expectedVersion, reason })),
  deleteRegister: (id: string, expectedVersion: number) => request<{ deleted: boolean }>(() => httpClient.delete(`${BASE_URL}/registers/${id}`, { params: { expected_version: expectedVersion } })),
};

export function responseMessage(response: Pick<ApiEnvelope<unknown>, "issues" | "message">, fallback: string) {
  const issue = response.issues?.[0];
  if (typeof issue === "string") return issue;
  return issue?.detail || response.message || fallback;
}

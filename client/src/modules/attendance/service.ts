// Campus Pilot Attendance HTTP client.

import { AxiosError } from "axios";

import { httpClient } from "@/lib/http-client";

import type {
  ApiEnvelope,
  AttendanceException,
  AttendanceExceptionListParams,
  AttendanceExceptionsResponse,
  AttendanceLessonSession,
  AttendanceLessonSessionListParams,
  AttendanceLessonSessionsResponse,
  AttendanceMarkInput,
  AttendanceReferenceData,
  AttendanceRegister,
  AttendanceRegisterInput,
  AttendanceRegisterListParams,
  AttendanceRegistersResponse,
  LearnerAttendanceHistory,
  LearnerAttendanceHistoryParams,
  SyncAttendanceLessonSessionsResponse,
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
  listLessonSessions: (params?: AttendanceLessonSessionListParams) => request<AttendanceLessonSessionsResponse>(() => httpClient.get(`${BASE_URL}/lesson-sessions`, { params })),
  readLessonSession: (id: string) => request<AttendanceLessonSession>(() => httpClient.get(`${BASE_URL}/lesson-sessions/${id}`)),
  syncLessonSessions: (dateFrom: string, dateTo: string) => request<SyncAttendanceLessonSessionsResponse>(() => httpClient.post(`${BASE_URL}/lesson-sessions/sync`, { date_from: dateFrom, date_to: dateTo })),
  openLessonSession: (id: string, expectedVersion: number) => request<AttendanceLessonSession>(() => httpClient.post(`${BASE_URL}/lesson-sessions/${id}/open`, { expected_version: expectedVersion, idempotency_key: crypto.randomUUID() })),
  cancelLessonSession: (id: string, expectedVersion: number, reason: string) => request<AttendanceLessonSession>(() => httpClient.post(`${BASE_URL}/lesson-sessions/${id}/cancel`, { expected_version: expectedVersion, reason })),
  listExceptions: (params?: AttendanceExceptionListParams) => request<AttendanceExceptionsResponse>(() => httpClient.get(`${BASE_URL}/exceptions`, { params })),
  readException: (id: string) => request<AttendanceException>(() => httpClient.get(`${BASE_URL}/exceptions/${id}`)),
  acknowledgeException: (id: string, expectedVersion: number, note: string) => request<AttendanceException>(() => httpClient.post(`${BASE_URL}/exceptions/${id}/acknowledge`, { expected_version: expectedVersion, note })),
  resolveException: (id: string, expectedVersion: number, resolution: string) => request<AttendanceException>(() => httpClient.post(`${BASE_URL}/exceptions/${id}/resolve`, { expected_version: expectedVersion, resolution })),
  reopenException: (id: string, expectedVersion: number, reason: string) => request<AttendanceException>(() => httpClient.post(`${BASE_URL}/exceptions/${id}/reopen`, { expected_version: expectedVersion, reason })),
};

export function responseMessage(response: Pick<ApiEnvelope<unknown>, "issues" | "message">, fallback: string) {
  const issue = response.issues?.[0];
  if (typeof issue === "string") return issue;
  return issue?.detail || response.message || fallback;
}

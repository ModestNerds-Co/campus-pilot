/** Typed HTTP boundary for Activities. */

import { AxiosError } from "axios";
import { httpClient } from "@/lib/http-client";
import type {
  ActivitiesReferences, ActivityCatalogItem, ActivityCatalogStatus, ActivityCategory,
  ActivityConsentStatus, ActivityGroupRecord, ActivityGroupStatus, ActivityGroupSummary,
  ActivityLeaderRole, ActivityMembership, ActivityMembershipStatus, ActivityParticipation,
  ActivityParticipationMark, ActivitySessionRecord, ActivitySessionStatus, ActivitySessionSummary,
  ApiEnvelope, CatalogPayload, GroupPayload, SessionPayload,
} from "./types";

const BASE = "/api/1.0/activities";

async function request<T>(work: () => Promise<{ data: ApiEnvelope<T> }>): Promise<ApiEnvelope<T>> {
  try { return (await work()).data; }
  catch (error) { if (error instanceof AxiosError && error.response) return error.response.data as ApiEnvelope<T>; throw error; }
}

function unwrapList<T>(response: ApiEnvelope<Record<string, T[]>>, key: string): ApiEnvelope<T[]> {
  return {
    ...response,
    data: response.data?.[key] ?? null,
  };
}

export const activitiesService = {
  catalog: (params?: { search?: string; category?: ActivityCategory; status?: ActivityCatalogStatus }) => request<ActivityCatalogItem[]>(() => httpClient.get(`${BASE}/catalog`, { params })),
  catalogItem: (id: string) => request<ActivityCatalogItem>(() => httpClient.get(`${BASE}/catalog/${id}`)),
  createCatalogItem: (payload: CatalogPayload) => request<ActivityCatalogItem>(() => httpClient.post(`${BASE}/catalog`, payload)),
  updateCatalogItem: (record: ActivityCatalogItem, payload: CatalogPayload) => request<ActivityCatalogItem>(() => httpClient.put(`${BASE}/catalog/${record.id}`, { ...payload, expected_version: record.version })),
  archiveCatalogItem: (record: ActivityCatalogItem, reason: string) => request<ActivityCatalogItem>(() => httpClient.post(`${BASE}/catalog/${record.id}/archive`, { expected_version: record.version, reason })),
  references: (search?: string) => request<ActivitiesReferences>(() => httpClient.get(`${BASE}/references`, { params: { search: search?.trim() || undefined } })),
  groups: async (params?: { page?: number; per_page?: number; search?: string; activity_id?: string; status?: ActivityGroupStatus; active_on?: string }) =>
    unwrapList(await request<{ groups: ActivityGroupSummary[] }>(() => httpClient.get(`${BASE}/groups`, { params })), "groups"),
  group: (id: string) => request<ActivityGroupRecord>(() => httpClient.get(`${BASE}/groups/${id}`)),
  createGroup: (payload: GroupPayload) => request<ActivityGroupRecord>(() => httpClient.post(`${BASE}/groups`, payload)),
  updateGroup: (record: ActivityGroupSummary, payload: GroupPayload) => request<ActivityGroupRecord>(() => httpClient.put(`${BASE}/groups/${record.id}`, { ...payload, expected_version: record.version })),
  activateGroup: (record: ActivityGroupSummary) => request<ActivityGroupRecord>(() => httpClient.post(`${BASE}/groups/${record.id}/activate`, { expected_version: record.version, reason: null })),
  closeGroup: (record: ActivityGroupSummary, reason: string) => request<ActivityGroupRecord>(() => httpClient.post(`${BASE}/groups/${record.id}/close`, { expected_version: record.version, reason })),
  cancelGroup: (record: ActivityGroupSummary, reason: string) => request<ActivityGroupRecord>(() => httpClient.post(`${BASE}/groups/${record.id}/cancel`, { expected_version: record.version, reason })),
  addLeader: (groupId: string, payload: { employee_id: string; role: ActivityLeaderRole; starts_on: string; ends_on: string | null }) => request<ActivityGroupRecord>(() => httpClient.post(`${BASE}/groups/${groupId}/leaders`, payload)),
  endLeader: (groupId: string, leader: { id: string; version: number }, ends_on: string, reason: string) => request<ActivityGroupRecord>(() => httpClient.post(`${BASE}/groups/${groupId}/leaders/${leader.id}/end`, { expected_version: leader.version, ends_on, reason })),
  addMember: (groupId: string, learner_id: string, joined_on: string) => request<ActivityGroupRecord>(() => httpClient.post(`${BASE}/groups/${groupId}/members`, { learner_id, joined_on })),
  updateMember: (groupId: string, member: ActivityMembership, consent_status: ActivityConsentStatus, consent_notes: string | null) => request<ActivityGroupRecord>(() => httpClient.put(`${BASE}/groups/${groupId}/members/${member.id}`, { expected_version: member.version, consent_status, consent_notes })),
  endMember: (groupId: string, member: ActivityMembership, ended_on: string, outcome: Exclude<ActivityMembershipStatus, "active">, reason: string) => request<ActivityGroupRecord>(() => httpClient.post(`${BASE}/groups/${groupId}/members/${member.id}/end`, { expected_version: member.version, ended_on, outcome, reason })),
  sessions: async (params?: { page?: number; per_page?: number; search?: string; group_id?: string; status?: ActivitySessionStatus; date_from?: string; date_to?: string }) =>
    unwrapList(await request<{ sessions: ActivitySessionSummary[] }>(() => httpClient.get(`${BASE}/sessions`, { params })), "sessions"),
  session: (id: string) => request<ActivitySessionRecord>(() => httpClient.get(`${BASE}/sessions/${id}`)),
  createSession: (payload: SessionPayload) => request<ActivitySessionRecord>(() => httpClient.post(`${BASE}/sessions`, payload)),
  updateSession: (record: ActivitySessionSummary, payload: Omit<SessionPayload, "group_id">) => request<ActivitySessionRecord>(() => httpClient.put(`${BASE}/sessions/${record.id}`, { ...payload, expected_version: record.version })),
  markParticipation: (sessionId: string, participation: ActivityParticipation, mark: ActivityParticipationMark, notes: string | null) => request<ActivitySessionRecord>(() => httpClient.put(`${BASE}/sessions/${sessionId}/participation/${participation.membership_id}`, { expected_version: participation.version, mark, notes })),
  completeSession: (record: ActivitySessionSummary, summary: string) => request<ActivitySessionRecord>(() => httpClient.post(`${BASE}/sessions/${record.id}/complete`, { expected_version: record.version, summary })),
  cancelSession: (record: ActivitySessionSummary, reason: string) => request<ActivitySessionRecord>(() => httpClient.post(`${BASE}/sessions/${record.id}/cancel`, { expected_version: record.version, reason })),
};

export function responseMessage(response: ApiEnvelope<unknown>, fallback: string) {
  const first = response.issues?.[0];
  if (typeof first === "string" && first.trim()) return first;
  if (first && typeof first === "object" && first.detail) return first.detail;
  return response.message || fallback;
}

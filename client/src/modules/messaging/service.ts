import { AxiosError } from "axios";
import { httpClient } from "@/lib/http-client";
import type { AnnouncementDetail, AnnouncementPayload, AnnouncementsResponse, AnnouncementStatus, ApiEnvelope, AudiencePreview, CommunicationReferenceData, DeliveryRecord, InboxItem, InboxResponse } from "./types";

const BASE_URL = "/api/1.0/messaging";
async function request<T>(work: () => Promise<{ data: ApiEnvelope<T> }>): Promise<ApiEnvelope<T>> { try { return (await work()).data; } catch (error) { if (error instanceof AxiosError && error.response) return error.response.data as ApiEnvelope<T>; throw error; } }

export const communicationService = {
  references: () => request<CommunicationReferenceData>(() => httpClient.get(`${BASE_URL}/references`)),
  listAnnouncements: (params?: { page?: number; per_page?: number; status?: AnnouncementStatus; search?: string }) => request<AnnouncementsResponse>(() => httpClient.get(`${BASE_URL}/announcements`, { params })),
  createAnnouncement: (payload: AnnouncementPayload) => request<AnnouncementDetail>(() => httpClient.post(`${BASE_URL}/announcements`, payload)),
  readAnnouncement: (id: string) => request<AnnouncementDetail>(() => httpClient.get(`${BASE_URL}/announcements/${id}`)),
  updateAnnouncement: (id: string, expectedVersion: number, payload: AnnouncementPayload) => request<AnnouncementDetail>(() => httpClient.put(`${BASE_URL}/announcements/${id}`, { ...payload, expected_version: expectedVersion })),
  audiencePreview: (id: string) => request<AudiencePreview>(() => httpClient.get(`${BASE_URL}/announcements/${id}/audience-preview`)),
  submitAnnouncement: (id: string, expectedVersion: number) => request<AnnouncementDetail>(() => httpClient.post(`${BASE_URL}/announcements/${id}/submit`, { expected_version: expectedVersion })),
  reopenAnnouncement: (id: string, expectedVersion: number, reason: string) => request<AnnouncementDetail>(() => httpClient.post(`${BASE_URL}/announcements/${id}/reopen`, { expected_version: expectedVersion, reason })),
  publishAnnouncement: (id: string, expectedVersion: number) => request<AnnouncementDetail>(() => httpClient.post(`${BASE_URL}/announcements/${id}/publish`, { expected_version: expectedVersion })),
  cancelAnnouncement: (id: string, expectedVersion: number, reason: string) => request<AnnouncementDetail>(() => httpClient.post(`${BASE_URL}/announcements/${id}/cancel`, { expected_version: expectedVersion, reason })),
  deleteAnnouncement: (id: string, expectedVersion: number) => request<{ deleted: boolean }>(() => httpClient.delete(`${BASE_URL}/announcements/${id}`, { params: { expected_version: expectedVersion } })),
  deliveries: (id: string) => request<DeliveryRecord[]>(() => httpClient.get(`${BASE_URL}/announcements/${id}/deliveries`)),
  inbox: (params?: { page?: number; per_page?: number; unread_only?: boolean }) => request<InboxResponse>(() => httpClient.get(`${BASE_URL}/inbox`, { params })),
  inboxMessage: (id: string) => request<InboxItem>(() => httpClient.get(`${BASE_URL}/inbox/${id}`)),
  markRead: (id: string) => request<InboxItem>(() => httpClient.post(`${BASE_URL}/inbox/${id}/read`)),
};

export function responseMessage(response: Pick<ApiEnvelope<unknown>, "issues" | "message">, fallback: string) { const issue = response.issues?.[0]; if (typeof issue === "string") return issue; return issue?.detail || response.message || fallback; }

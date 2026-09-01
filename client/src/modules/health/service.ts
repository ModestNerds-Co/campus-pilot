import { AxiosError } from "axios";
import { httpClient } from "@/lib/http-client";
import type {
  ApiEnvelope, CareItem, CareItemKind, CareSeverity, FollowUp, FollowUpStatus,
  FollowUpsResponse, HealthReferences, MedicationAdministration,
  MedicationAdministrationsResponse, MedicationPlan, MedicationPlansResponse,
  MedicationPlanStatus, PatientKind, PatientRecord, PatientsResponse,
  PatientStatus, Visit, VisitCategory, VisitDisposition, VisitsResponse,
} from "./types";

const BASE = "/api/1.0/health";
async function request<T>(work: () => Promise<{ data: ApiEnvelope<T> }>): Promise<ApiEnvelope<T>> {
  try { return (await work()).data; }
  catch (error) {
    if (error instanceof AxiosError && error.response) return error.response.data as ApiEnvelope<T>;
    throw error;
  }
}
export type ListParams = { page?: number; per_page?: number; search?: string; status?: string; patient_id?: string };

export const healthService = {
  references: (search?: string) => request<HealthReferences>(() => httpClient.get(`${BASE}/references`, { params: { search: search || undefined } })),
  patients: (params?: ListParams) => request<PatientsResponse>(() => httpClient.get(`${BASE}/patients`, { params })),
  patient: (id: string) => request<PatientRecord>(() => httpClient.get(`${BASE}/patients/${id}`)),
  createPatient: (person_kind: PatientKind, person_id: string) => request<PatientRecord>(() => httpClient.post(`${BASE}/patients`, { person_kind, person_id })),
  updatePatient: (patient: PatientSummaryLike, status: PatientStatus) => request<PatientRecord>(() => httpClient.put(`${BASE}/patients/${patient.id}`, { expected_version: patient.version, status })),
  createCareItem: (patientId: string, payload: CareItemPayload) => request<CareItem>(() => httpClient.post(`${BASE}/patients/${patientId}/care-items`, payload)),
  updateCareItem: (item: CareItem, payload: CareItemPayload & { status: "active" | "resolved" }) => request<CareItem>(() => httpClient.put(`${BASE}/care-items/${item.id}`, { ...payload, expected_version: item.version })),
  visits: (params?: ListParams) => request<VisitsResponse>(() => httpClient.get(`${BASE}/visits`, { params })),
  visit: (id: string) => request<Visit>(() => httpClient.get(`${BASE}/visits/${id}`)),
  createVisit: (payload: VisitPayload) => request<Visit>(() => httpClient.post(`${BASE}/visits`, payload)),
  closeVisit: (visit: Visit, disposition: VisitDisposition, assessment: string | null, care_given: string | null) => request<Visit>(() => httpClient.post(`${BASE}/visits/${visit.id}/close`, { expected_version: visit.version, disposition, assessment, care_given })),
  medicationPlans: (params?: ListParams) => request<MedicationPlansResponse>(() => httpClient.get(`${BASE}/medication-plans`, { params })),
  createMedicationPlan: (payload: MedicationPlanPayload) => request<MedicationPlan>(() => httpClient.post(`${BASE}/medication-plans`, payload)),
  updateMedicationPlan: (plan: MedicationPlan, payload: MedicationPlanPayload & { status: MedicationPlanStatus }) => request<MedicationPlan>(() => httpClient.put(`${BASE}/medication-plans/${plan.id}`, { ...payload, expected_version: plan.version })),
  administrations: (params?: Pick<ListParams, "page" | "per_page" | "patient_id">) => request<MedicationAdministrationsResponse>(() => httpClient.get(`${BASE}/medication-administrations`, { params })),
  recordAdministration: (planId: string, payload: { administered_at: string; dose: string; outcome: "given" | "refused" | "missed" | "held"; note: string | null }) => request<MedicationAdministration>(() => httpClient.post(`${BASE}/medication-plans/${planId}/administrations`, payload)),
  followUps: (params?: ListParams) => request<FollowUpsResponse>(() => httpClient.get(`${BASE}/follow-ups`, { params })),
  createFollowUp: (payload: FollowUpPayload) => request<FollowUp>(() => httpClient.post(`${BASE}/follow-ups`, payload)),
  updateFollowUp: (followUp: FollowUp, payload: Omit<FollowUpPayload, "patient_id" | "visit_id"> & { status: FollowUpStatus; outcome: string | null }) => request<FollowUp>(() => httpClient.put(`${BASE}/follow-ups/${followUp.id}`, { ...payload, expected_version: followUp.version })),
};

type PatientSummaryLike = { id: string; version: number };
export type CareItemPayload = { kind: CareItemKind; title: string; details: string | null; severity: CareSeverity; reviewed_on: string | null };
export type VisitPayload = { patient_id: string; checked_in_at: string; category: VisitCategory; presenting_concern: string; assessment: string | null; care_given: string | null };
export type MedicationPlanPayload = { patient_id: string; medication_name: string; dosage: string; route: string; schedule: string; instructions: string | null; authorization_reference: string; starts_on: string; ends_on: string | null };
export type FollowUpPayload = { patient_id: string; visit_id: string | null; assigned_employee_id: string | null; due_on: string; purpose: string };

export function responseMessage(response: ApiEnvelope<unknown>, fallback: string) {
  const first = response.issues?.[0];
  if (typeof first === "string" && first.trim()) return first;
  if (first && typeof first === "object" && first.detail) return first.detail;
  return response.message || fallback;
}

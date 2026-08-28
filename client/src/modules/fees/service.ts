import { AxiosError } from "axios";

import { httpClient } from "@/lib/http-client";

import type {
  ApiEnvelope, BillingAccount, BillingAccountsResponse, BillingAccountStatus,
  FeeStructure, FeeStructureInput, FeeStructuresResponse, FeesReferenceData,
  LearnerCandidatesResponse, ListParams,
} from "./types";

const BASE_URL = "/api/1.0/fees";

async function request<T>(work: () => Promise<{ data: ApiEnvelope<T> }>): Promise<ApiEnvelope<T>> {
  try {
    return (await work()).data;
  } catch (error) {
    if (error instanceof AxiosError && error.response) return error.response.data as ApiEnvelope<T>;
    throw error;
  }
}

export const feesService = {
  referenceData: () => request<FeesReferenceData>(() => httpClient.get(`${BASE_URL}/reference-data`)),
  learnerCandidates: (search?: string) => request<LearnerCandidatesResponse>(() => httpClient.get(`${BASE_URL}/learner-candidates`, { params: { search } })),
  listBillingAccounts: (params?: ListParams) => request<BillingAccountsResponse>(() => httpClient.get(`${BASE_URL}/billing-accounts`, { params })),
  readBillingAccount: (id: string) => request<BillingAccount>(() => httpClient.get(`${BASE_URL}/billing-accounts/${id}`)),
  createBillingAccount: (data: { learner_id: string; opened_on: string; idempotency_key: string }) => request<BillingAccount>(() => httpClient.post(`${BASE_URL}/billing-accounts`, data)),
  updateBillingAccount: (id: string, status: BillingAccountStatus, expectedVersion: number) => request<BillingAccount>(() => httpClient.put(`${BASE_URL}/billing-accounts/${id}`, { status, expected_version: expectedVersion })),
  listFeeStructures: (params?: ListParams) => request<FeeStructuresResponse>(() => httpClient.get(`${BASE_URL}/fee-structures`, { params })),
  readFeeStructure: (id: string) => request<FeeStructure>(() => httpClient.get(`${BASE_URL}/fee-structures/${id}`)),
  createFeeStructure: (data: FeeStructureInput & { idempotency_key: string }) => request<FeeStructure>(() => httpClient.post(`${BASE_URL}/fee-structures`, data)),
  updateFeeStructure: (id: string, data: FeeStructureInput & { expected_version: number }) => request<FeeStructure>(() => httpClient.put(`${BASE_URL}/fee-structures/${id}`, data)),
  deleteFeeStructure: (id: string, expectedVersion: number) => request<{ deleted: boolean }>(() => httpClient.delete(`${BASE_URL}/fee-structures/${id}`, { params: { expected_version: expectedVersion } })),
  activateFeeStructure: (id: string, expectedVersion: number) => request<FeeStructure>(() => httpClient.post(`${BASE_URL}/fee-structures/${id}/activate`, { expected_version: expectedVersion })),
  retireFeeStructure: (id: string, expectedVersion: number) => request<FeeStructure>(() => httpClient.post(`${BASE_URL}/fee-structures/${id}/retire`, { expected_version: expectedVersion })),
};

export function responseMessage(response: Pick<ApiEnvelope<unknown>, "issues" | "message">, fallback: string) {
  const issue = response.issues?.[0];
  if (typeof issue === "string") return issue;
  return issue?.detail || response.message || fallback;
}

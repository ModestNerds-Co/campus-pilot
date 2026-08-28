import { AxiosError } from "axios";

import { httpClient } from "@/lib/http-client";

import type {
  AccountInput, AccountsResponse, ApiEnvelope, CurrenciesResponse,
  CurrencyInput, FinanceAccount, FinanceAccountingPeriod, FinanceCurrency, FinanceFiscalYear,
  FiscalYearInput, FiscalYearsResponse, AccountingPeriodsResponse, ListParams,
  FinanceJournal, JournalInput, JournalsResponse, JournalValidation,
  FinancePostingRequest, PostingRequestConversionLine, PostingRequestsResponse,
} from "./types";

const BASE_URL = "/api/1.0/finance";

async function request<T>(work: () => Promise<{ data: ApiEnvelope<T> }>): Promise<ApiEnvelope<T>> {
  try {
    return (await work()).data;
  } catch (error) {
    if (error instanceof AxiosError && error.response) return error.response.data as ApiEnvelope<T>;
    throw error;
  }
}

export const financeService = {
  listCurrencies: (params?: ListParams) => request<CurrenciesResponse>(() => httpClient.get(`${BASE_URL}/currencies`, { params })),
  createCurrency: (data: CurrencyInput) => request<FinanceCurrency>(() => httpClient.post(`${BASE_URL}/currencies`, data)),
  updateCurrency: (id: string, data: CurrencyInput) => request<FinanceCurrency>(() => httpClient.put(`${BASE_URL}/currencies/${id}`, data)),
  deleteCurrency: (id: string) => request<{ deleted: boolean }>(() => httpClient.delete(`${BASE_URL}/currencies/${id}`)),
  listAccounts: (params?: ListParams) => request<AccountsResponse>(() => httpClient.get(`${BASE_URL}/accounts`, { params })),
  createAccount: (data: AccountInput) => request<FinanceAccount>(() => httpClient.post(`${BASE_URL}/accounts`, data)),
  updateAccount: (id: string, data: AccountInput) => request<FinanceAccount>(() => httpClient.put(`${BASE_URL}/accounts/${id}`, data)),
  deleteAccount: (id: string) => request<{ deleted: boolean }>(() => httpClient.delete(`${BASE_URL}/accounts/${id}`)),
  listFiscalYears: (params?: ListParams) => request<FiscalYearsResponse>(() => httpClient.get(`${BASE_URL}/fiscal-years`, { params })),
  createFiscalYear: (data: FiscalYearInput) => request<FinanceFiscalYear>(() => httpClient.post(`${BASE_URL}/fiscal-years`, data)),
  updateFiscalYear: (id: string, data: { name: string }) => request<FinanceFiscalYear>(() => httpClient.put(`${BASE_URL}/fiscal-years/${id}`, data)),
  deleteFiscalYear: (id: string) => request<{ deleted: boolean }>(() => httpClient.delete(`${BASE_URL}/fiscal-years/${id}`)),
  openFiscalYear: (id: string) => request<FinanceFiscalYear>(() => httpClient.post(`${BASE_URL}/fiscal-years/${id}/open`)),
  closeFiscalYear: (id: string) => request<FinanceFiscalYear>(() => httpClient.post(`${BASE_URL}/fiscal-years/${id}/close`)),
  listAccountingPeriods: (fiscalYearId: string) => request<AccountingPeriodsResponse>(() => httpClient.get(`${BASE_URL}/fiscal-years/${fiscalYearId}/periods`)),
  closeAccountingPeriod: (id: string) => request<FinanceAccountingPeriod>(() => httpClient.post(`${BASE_URL}/periods/${id}/close`)),
  reopenAccountingPeriod: (id: string) => request<FinanceAccountingPeriod>(() => httpClient.post(`${BASE_URL}/periods/${id}/reopen`)),
  listJournals: (params?: ListParams) => request<JournalsResponse>(() => httpClient.get(`${BASE_URL}/journals`, { params })),
  getJournal: (id: string) => request<FinanceJournal>(() => httpClient.get(`${BASE_URL}/journals/${id}`)),
  validateJournal: (id: string) => request<JournalValidation>(() => httpClient.get(`${BASE_URL}/journals/${id}/validation`)),
  createJournal: (data: JournalInput & { idempotency_key: string }) => request<FinanceJournal>(() => httpClient.post(`${BASE_URL}/journals`, data)),
  updateJournal: (id: string, data: JournalInput & { expected_version: number }) => request<FinanceJournal>(() => httpClient.put(`${BASE_URL}/journals/${id}`, data)),
  deleteJournal: (id: string, expectedVersion: number) => request<{ deleted: boolean }>(() => httpClient.delete(`${BASE_URL}/journals/${id}`, { params: { expected_version: expectedVersion } })),
  submitJournal: (id: string, expectedVersion: number) => request<FinanceJournal>(() => httpClient.post(`${BASE_URL}/journals/${id}/submit`, { expected_version: expectedVersion })),
  approveJournal: (id: string, expectedVersion: number) => request<FinanceJournal>(() => httpClient.post(`${BASE_URL}/journals/${id}/approve`, { expected_version: expectedVersion })),
  rejectJournal: (id: string, expectedVersion: number, reason: string) => request<FinanceJournal>(() => httpClient.post(`${BASE_URL}/journals/${id}/reject`, { expected_version: expectedVersion, reason })),
  postJournal: (id: string, expectedVersion: number) => request<FinanceJournal>(() => httpClient.post(`${BASE_URL}/journals/${id}/post`, { expected_version: expectedVersion })),
  reverseJournal: (id: string, expectedVersion: number, journalDate: string, reason: string) => request<FinanceJournal>(() => httpClient.post(`${BASE_URL}/journals/${id}/reverse`, { expected_version: expectedVersion, journal_date: journalDate, reason, idempotency_key: crypto.randomUUID() })),
  listPostingRequests: (params?: ListParams) => request<PostingRequestsResponse>(() => httpClient.get(`${BASE_URL}/posting-requests`, { params })),
  getPostingRequest: (id: string) => request<FinancePostingRequest>(() => httpClient.get(`${BASE_URL}/posting-requests/${id}`)),
  convertPostingRequest: (id: string, expectedVersion: number, lines: PostingRequestConversionLine[]) => request<FinancePostingRequest>(() => httpClient.post(`${BASE_URL}/posting-requests/${id}/convert`, { expected_version: expectedVersion, idempotency_key: crypto.randomUUID(), lines })),
  rejectPostingRequest: (id: string, expectedVersion: number, reason: string) => request<FinancePostingRequest>(() => httpClient.post(`${BASE_URL}/posting-requests/${id}/reject`, { expected_version: expectedVersion, reason })),
};

export function responseMessage(response: Pick<ApiEnvelope<unknown>, "issues" | "message">, fallback: string) {
  const issue = response.issues?.[0];
  if (typeof issue === "string") return issue;
  return issue?.detail || response.message || fallback;
}

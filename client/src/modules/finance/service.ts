import { AxiosError } from "axios";

import { httpClient } from "@/lib/http-client";

import type {
  AccountInput, AccountsResponse, ApiEnvelope, CurrenciesResponse,
  CurrencyInput, FinanceAccount, FinanceAccountingPeriod, FinanceCurrency, FinanceFiscalYear,
  FiscalYearInput, FiscalYearsResponse, AccountingPeriodsResponse, ListParams,
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
};

export function responseMessage(response: Pick<ApiEnvelope<unknown>, "issues" | "message">, fallback: string) {
  const issue = response.issues?.[0];
  if (typeof issue === "string") return issue;
  return issue?.detail || response.message || fallback;
}

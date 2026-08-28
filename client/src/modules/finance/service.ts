import { AxiosError } from "axios";

import { httpClient } from "@/lib/http-client";

import type {
  AccountInput, AccountsResponse, ApiEnvelope, CurrenciesResponse,
  CurrencyInput, FinanceAccount, FinanceCurrency, ListParams,
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
};

export function responseMessage(response: Pick<ApiEnvelope<unknown>, "issues" | "message">, fallback: string) {
  const issue = response.issues?.[0];
  if (typeof issue === "string") return issue;
  return issue?.detail || response.message || fallback;
}

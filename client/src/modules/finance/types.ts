export type RecordStatus = "active" | "inactive";
export type AccountType = "asset" | "liability" | "equity" | "income" | "expense";
export type CurrencyMode = "reporting" | "single" | "multi";

export interface FinanceCurrency {
  id: string;
  code: string;
  name: string;
  symbol: string | null;
  minor_units: number;
  is_reporting: boolean;
  status: RecordStatus;
  created_at: string;
  updated_at: string;
}

export interface FinanceAccount {
  id: string;
  parent_account_id: string | null;
  parent_account_code: string | null;
  currency_id: string | null;
  currency_code: string | null;
  code: string;
  name: string;
  description: string | null;
  account_type: AccountType;
  normal_balance: "debit" | "credit";
  currency_mode: CurrencyMode;
  accepts_postings: boolean;
  status: RecordStatus;
  child_count: number;
  created_at: string;
  updated_at: string;
}

export interface CurrencyInput {
  code: string;
  name: string;
  symbol: string | null;
  minor_units: number;
  is_reporting: boolean;
  status: RecordStatus;
}

export interface AccountInput {
  code: string;
  name: string;
  description: string | null;
  account_type: AccountType;
  parent_account_id: string | null;
  currency_mode: CurrencyMode;
  currency_id: string | null;
  accepts_postings: boolean;
  status: RecordStatus;
}

export interface ListParams {
  page?: number;
  per_page?: number;
  search?: string;
  status?: string;
  account_type?: string;
  currency_mode?: string;
}

export interface PaginationMeta {
  current_page: number;
  per_page: number;
  total: number;
  total_pages: number;
  has_next: boolean;
  has_prev: boolean;
}

export interface ApiEnvelope<T> {
  success: boolean;
  message: string | null;
  data: T | null;
  pagination: PaginationMeta | null;
  issues: Array<string | { detail?: string }> | null;
}

export interface CurrenciesResponse { currencies: FinanceCurrency[] }
export interface AccountsResponse { accounts: FinanceAccount[] }

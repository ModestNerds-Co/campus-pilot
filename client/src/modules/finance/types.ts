export type RecordStatus = "active" | "inactive";
export type AccountType = "asset" | "liability" | "equity" | "income" | "expense";
export type CurrencyMode = "reporting" | "single" | "multi";
export type FiscalYearStatus = "draft" | "open" | "closed";
export type AccountingPeriodStatus = "planned" | "open" | "closed";
export type PeriodCadence = "monthly" | "quarterly";

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

export interface FinanceFiscalYear {
  id: string;
  name: string;
  starts_on: string;
  ends_on: string;
  period_cadence: PeriodCadence;
  status: FiscalYearStatus;
  opened_at: string | null;
  closed_at: string | null;
  period_count: number;
  open_period_count: number;
  created_at: string;
  updated_at: string;
}

export interface FinanceAccountingPeriod {
  id: string;
  fiscal_year_id: string;
  period_number: number;
  name: string;
  starts_on: string;
  ends_on: string;
  status: AccountingPeriodStatus;
  closed_at: string | null;
  created_at: string;
  updated_at: string;
}

export interface FiscalYearInput {
  name: string;
  starts_on: string;
  ends_on: string;
  period_cadence: PeriodCadence;
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
export interface FiscalYearsResponse { fiscal_years: FinanceFiscalYear[] }
export interface AccountingPeriodsResponse { periods: FinanceAccountingPeriod[] }

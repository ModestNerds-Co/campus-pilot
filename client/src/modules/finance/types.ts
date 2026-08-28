export type RecordStatus = "active" | "inactive";
export type AccountType = "asset" | "liability" | "equity" | "income" | "expense";
export type CurrencyMode = "reporting" | "single" | "multi";
export type FiscalYearStatus = "draft" | "open" | "closed";
export type AccountingPeriodStatus = "planned" | "open" | "closed";
export type PeriodCadence = "monthly" | "quarterly";
export type JournalStatus = "draft" | "submitted" | "approved" | "rejected" | "posted" | "reversed";
export type PostingRequestStatus = "pending" | "converted" | "rejected" | "cancelled";

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

export interface JournalSource {
  module_key: string;
  record_type: string;
  record_id: string;
}

export interface FinanceJournalLine {
  id: string;
  line_number: number;
  account_id: string;
  account_code: string;
  account_name: string;
  transaction_currency_id: string;
  transaction_currency_code: string;
  transaction_currency_minor_units: number;
  description: string | null;
  debit_minor: number;
  credit_minor: number;
  reporting_debit_minor: number;
  reporting_credit_minor: number;
  exchange_rate: string | null;
}

export interface FinanceJournalSummary {
  id: string;
  fiscal_year_id: string;
  fiscal_year_name: string;
  accounting_period_id: string;
  accounting_period_name: string;
  reporting_currency_id: string;
  reporting_currency_code: string;
  reporting_currency_minor_units: number;
  reversal_of_journal_id: string | null;
  reversal_journal_id: string | null;
  journal_number: string;
  journal_date: string;
  description: string;
  reference: string | null;
  source_module_key: string | null;
  source_record_type: string | null;
  source_record_id: string | null;
  status: JournalStatus;
  version: number;
  line_count: number;
  reporting_debit_minor: number;
  reporting_credit_minor: number;
  created_by: string;
  submitted_by: string | null;
  submitted_at: string | null;
  approved_by: string | null;
  approved_at: string | null;
  rejected_by: string | null;
  rejected_at: string | null;
  rejection_reason: string | null;
  posted_by: string | null;
  posted_at: string | null;
  created_at: string;
  updated_at: string;
}

export interface FinanceJournal extends FinanceJournalSummary {
  lines: FinanceJournalLine[];
}

export interface JournalLineInput {
  account_id: string;
  transaction_currency_id: string;
  description: string | null;
  debit_minor: number;
  credit_minor: number;
  reporting_debit_minor: number;
  reporting_credit_minor: number;
  exchange_rate: string | null;
}

export interface JournalInput {
  journal_date: string;
  description: string;
  reference: string | null;
  source: JournalSource | null;
  lines: JournalLineInput[];
}

export interface JournalValidation {
  valid: boolean;
  issues: string[];
  line_count: number;
  reporting_debit_minor: number;
  reporting_credit_minor: number;
}

export interface FinancePostingRequestSummary {
  id: string;
  source_module_key: string;
  source_record_type: string;
  source_record_id: string;
  source_event_key: string;
  posting_date: string;
  transaction_currency_id: string;
  transaction_currency_code: string;
  transaction_currency_minor_units: number;
  description: string;
  reference: string | null;
  status: PostingRequestStatus;
  version: number;
  journal_id: string | null;
  line_count: number;
  debit_minor: number;
  credit_minor: number;
  created_by: string;
  resolved_by: string | null;
  resolved_at: string | null;
  resolution_reason: string | null;
  created_at: string;
  updated_at: string;
}

export interface FinancePostingRequestLine {
  id: string;
  line_number: number;
  account_id: string;
  account_code: string;
  account_name: string;
  description: string | null;
  debit_minor: number;
  credit_minor: number;
}

export interface FinancePostingRequest extends FinancePostingRequestSummary {
  lines: FinancePostingRequestLine[];
}

export interface PostingRequestConversionLine {
  line_id: string;
  reporting_debit_minor: number;
  reporting_credit_minor: number;
  exchange_rate: string | null;
}

export interface ListParams {
  page?: number;
  per_page?: number;
  search?: string;
  status?: string;
  account_type?: string;
  currency_mode?: string;
  starts_on?: string;
  ends_on?: string;
  source_module?: string;
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
export interface JournalsResponse { journals: FinanceJournalSummary[] }
export interface PostingRequestsResponse { posting_requests: FinancePostingRequestSummary[] }

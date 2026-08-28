export type BillingAccountStatus = "active" | "on_hold" | "closed";
export type FeeStructureStatus = "draft" | "active" | "retired";
export type InvoiceStatus = "draft" | "issued";

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

export interface BillingAccount {
  id: string;
  learner_id: string;
  learner_number: string;
  learner_name: string;
  learner_status: string;
  account_number: string;
  opened_on: string;
  status: BillingAccountStatus;
  version: number;
  created_by: string;
  closed_by: string | null;
  closed_at: string | null;
  created_at: string;
  updated_at: string;
}

export interface LearnerCandidate {
  id: string;
  learner_number: string;
  display_name: string;
  status: string;
}

export interface FeeStructure {
  id: string;
  academic_year_id: string;
  academic_term_id: string | null;
  grade_level_id: string | null;
  currency_id: string;
  receivable_account_id: string;
  revenue_account_id: string;
  code: string;
  name: string;
  description: string | null;
  amount_minor: number;
  status: FeeStructureStatus;
  version: number;
  created_by: string;
  activated_by: string | null;
  activated_at: string | null;
  retired_by: string | null;
  retired_at: string | null;
  created_at: string;
  updated_at: string;
}

export interface FeeCurrencyReference {
  id: string;
  code: string;
  minor_units: number;
  is_reporting: boolean;
}

export interface FeeAccountReference {
  id: string;
  code: string;
  name: string;
  currency_mode: string;
  currency_id: string | null;
}

export interface FeeAcademicYearReference {
  id: string;
  name: string;
  status: string;
}

export interface FeeAcademicTermReference {
  id: string;
  academic_year_id: string;
  code: string;
  name: string;
  status: string;
}

export interface FeeGradeLevelReference {
  id: string;
  code: string;
  name: string;
}

export interface FeesReferenceData {
  currencies: FeeCurrencyReference[];
  receivable_accounts: FeeAccountReference[];
  revenue_accounts: FeeAccountReference[];
  academic_years: FeeAcademicYearReference[];
  academic_terms: FeeAcademicTermReference[];
  grade_levels: FeeGradeLevelReference[];
}

export interface FeeStructureInput {
  academic_year_id: string;
  academic_term_id: string | null;
  grade_level_id: string | null;
  currency_id: string;
  receivable_account_id: string;
  revenue_account_id: string;
  code: string;
  name: string;
  description: string | null;
  amount_minor: number;
}

export interface InvoiceSummary {
  id: string;
  billing_account_id: string;
  billing_account_number: string;
  learner_id: string;
  learner_number: string;
  learner_name: string;
  academic_year_id: string;
  academic_year_name: string;
  academic_term_id: string | null;
  academic_term_name: string | null;
  currency_id: string;
  currency_code: string;
  currency_minor_units: number;
  posting_request_id: string | null;
  posting_request_status: string | null;
  invoice_number: string;
  invoice_date: string;
  due_date: string;
  description: string | null;
  reference: string | null;
  total_minor: number;
  status: InvoiceStatus;
  version: number;
  line_count: number;
  created_by: string;
  issued_by: string | null;
  issued_at: string | null;
  created_at: string;
  updated_at: string;
}

export interface InvoiceLine {
  id: string;
  line_number: number;
  fee_structure_id: string;
  receivable_account_id: string;
  revenue_account_id: string;
  fee_code: string;
  description: string;
  amount_minor: number;
}

export interface Invoice extends InvoiceSummary { lines: InvoiceLine[] }

export interface InvoiceInput {
  billing_account_id: string;
  academic_year_id: string;
  academic_term_id: string | null;
  invoice_date: string;
  due_date: string;
  description: string | null;
  reference: string | null;
  fee_structure_ids: string[];
}

export interface ListParams {
  page?: number;
  per_page?: number;
  search?: string;
  status?: string;
}

export interface BillingAccountsResponse { billing_accounts: BillingAccount[] }
export interface FeeStructuresResponse { fee_structures: FeeStructure[] }
export interface LearnerCandidatesResponse { learners: LearnerCandidate[] }
export interface InvoicesResponse { invoices: InvoiceSummary[] }

export type BorrowerKind = "learner" | "employee";
export type CopyCondition = "new" | "good" | "worn" | "damaged";
export type CopyStatus =
  | "available"
  | "on_loan"
  | "reserved"
  | "lost"
  | "repair"
  | "retired";
export type MembershipStatus = "active" | "suspended" | "closed";

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

export interface CurrencyReference {
  id: string;
  code: string;
  minor_units: number;
  is_reporting: boolean;
}
export interface BorrowerCandidate {
  kind: BorrowerKind;
  id: string;
  number: string;
  display_name: string;
  source_status: string;
  account_linked: boolean;
  already_member: boolean;
}
export interface BillingAccountReference {
  id: string;
  learner_id: string;
  learner_number: string;
  learner_name: string;
  account_number: string;
}
export interface LibraryReferenceData {
  learners: BorrowerCandidate[];
  employees: BorrowerCandidate[];
  currencies: CurrencyReference[];
  billing_accounts: BillingAccountReference[];
}

export interface LibrarySettings {
  accession_prefix: string;
  accession_next_sequence: number;
  accession_padding: number;
  next_accession_preview: string;
  learner_loan_days: number;
  employee_loan_days: number;
  default_loan_limit: number;
  maximum_renewals: number;
  fine_currency_id: string | null;
  fine_currency_code: string | null;
  fine_currency_minor_units: number | null;
  overdue_fine_minor: number;
  updated_at: string;
}

export interface TitleSummary {
  id: string;
  isbn: string | null;
  title: string;
  subtitle: string | null;
  authors: string[];
  subject: string | null;
  status: string;
  version: number;
  copy_count: number;
  available_copy_count: number;
  created_at: string;
  updated_at: string;
}
export interface TitleDetail extends TitleSummary {
  publisher: string | null;
  publication_year: number | null;
  edition: string | null;
  language_code: string;
  replacement_cost_minor: number | null;
  replacement_currency_id: string | null;
  replacement_currency_code: string | null;
  replacement_currency_minor_units: number | null;
  created_by: string;
}
export interface TitlePayload {
  title: string;
  subtitle: string | null;
  authors: string[];
  isbn: string | null;
  publisher: string | null;
  publication_year: number | null;
  edition: string | null;
  language_code: string;
  subject: string | null;
  replacement_cost_minor: number | null;
  replacement_currency_id: string | null;
}
export interface CopyRecord {
  id: string;
  title_id: string;
  title: string;
  accession_number: string;
  barcode: string | null;
  location: string | null;
  condition: CopyCondition;
  status: CopyStatus;
  version: number;
  created_at: string;
  updated_at: string;
}

export interface Membership {
  id: string;
  borrower_kind: BorrowerKind;
  borrower_id: string;
  borrower_number: string;
  borrower_name: string;
  borrower_source_status: string;
  account_linked: boolean;
  card_number: string;
  status: MembershipStatus;
  loan_limit: number;
  active_loan_count: number;
  active_hold_count: number;
  version: number;
  created_at: string;
  updated_at: string;
}
export interface Loan {
  id: string;
  copy_id: string;
  accession_number: string;
  title_id: string;
  title: string;
  membership_id: string;
  borrower_kind: BorrowerKind;
  borrower_number: string;
  borrower_name: string;
  status: "active" | "returned" | "lost";
  checked_out_on: string;
  due_on: string;
  returned_on: string | null;
  overdue: boolean;
  overdue_days: number;
  renewal_count: number;
  version: number;
  notes: string | null;
  created_at: string;
  updated_at: string;
}
export interface Hold {
  id: string;
  title_id: string;
  title: string;
  membership_id: string;
  borrower_kind: BorrowerKind;
  borrower_number: string;
  borrower_name: string;
  copy_id: string | null;
  accession_number: string | null;
  queue_position: number;
  status: "waiting" | "ready" | "fulfilled" | "cancelled" | "expired";
  version: number;
  expires_at: string | null;
  resolution_reason: string | null;
  created_at: string;
  updated_at: string;
}
export interface Fine {
  id: string;
  loan_id: string;
  membership_id: string;
  borrower_kind: BorrowerKind;
  borrower_number: string;
  borrower_name: string;
  title: string;
  kind: "overdue" | "replacement";
  currency_id: string;
  currency_code: string;
  currency_minor_units: number;
  amount_minor: number;
  status: "assessed" | "submitted_to_fees" | "waived";
  assessed_days: number | null;
  fees_charge_request_id: string | null;
  fees_charge_status: string | null;
  version: number;
  waiver_reason: string | null;
  created_at: string;
  updated_at: string;
}

export interface TitlesResponse {
  titles: TitleSummary[];
}
export interface CopiesResponse {
  copies: CopyRecord[];
}
export interface MembershipsResponse {
  memberships: Membership[];
}
export interface LoansResponse {
  loans: Loan[];
}
export interface HoldsResponse {
  holds: Hold[];
}
export interface FinesResponse {
  fines: Fine[];
}

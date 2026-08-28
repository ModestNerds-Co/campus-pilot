/**
 * Procurement API contracts. Money is transported as integer minor units;
 * requester identity remains owned by HR and currency metadata by Finance.
 */

export type SupplierStatus = "active" | "inactive";
export type RequisitionStatus = "draft" | "submitted" | "approved" | "rejected" | "cancelled";

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

export interface ListParams {
  page?: number;
  per_page?: number;
  search?: string;
  status?: string;
}

export interface Supplier {
  id: string;
  supplier_number: string;
  legal_name: string;
  trading_name: string | null;
  registration_number: string | null;
  tax_number: string | null;
  email: string | null;
  phone: string | null;
  address: string | null;
  status: SupplierStatus;
  version: number;
  created_by: string;
  created_at: string;
  updated_at: string;
}

export interface SupplierInput {
  legal_name: string;
  trading_name: string | null;
  registration_number: string | null;
  tax_number: string | null;
  email: string | null;
  phone: string | null;
  address: string | null;
}

export interface SuppliersResponse {
  suppliers: Supplier[];
}

export interface ProcurementCurrency {
  id: string;
  code: string;
  name: string;
  symbol: string | null;
  minor_units: number;
  is_reporting: boolean;
}

export interface ProcurementReferenceData {
  currencies: ProcurementCurrency[];
}

export interface RequesterCandidate {
  id: string;
  account_id: string | null;
  employee_number: string;
  display_name: string;
  work_email: string | null;
  phone: string | null;
  employment_status: string;
}

export interface RequesterCandidatesResponse {
  employees: RequesterCandidate[];
}

export interface RequisitionLineInput {
  description: string;
  quantity: number;
  unit_label: string | null;
  estimated_unit_amount_minor: number;
  preferred_supplier_id: string | null;
}

export interface RequisitionInput {
  requester_employee_id: string;
  currency_id: string;
  title: string;
  purpose: string | null;
  needed_by: string | null;
  lines: RequisitionLineInput[];
}

export interface RequisitionSummary {
  id: string;
  requisition_number: string;
  requester_employee_id: string;
  requester_account_id: string | null;
  requester_employee_number: string;
  requester_name: string;
  currency_id: string;
  currency_code: string;
  currency_minor_units: number;
  title: string;
  purpose: string | null;
  needed_by: string | null;
  status: RequisitionStatus;
  version: number;
  total_minor: number;
  line_count: number;
  created_by: string;
  submitted_by: string | null;
  submitted_at: string | null;
  decided_by: string | null;
  decided_at: string | null;
  decision_note: string | null;
  cancelled_by: string | null;
  cancelled_at: string | null;
  cancellation_note: string | null;
  created_at: string;
  updated_at: string;
}

export interface RequisitionLine {
  id: string;
  line_number: number;
  description: string;
  quantity: number;
  unit_label: string | null;
  estimated_unit_amount_minor: number;
  estimated_line_amount_minor: number;
  preferred_supplier_id: string | null;
  preferred_supplier_number: string | null;
  preferred_supplier_name: string | null;
}

export interface Requisition extends RequisitionSummary {
  lines: RequisitionLine[];
}

export interface RequisitionsResponse {
  requisitions: RequisitionSummary[];
}

export interface RequisitionListParams extends ListParams {
  requester_employee_id?: string;
}

/** Department stock-request API contracts. Quantities stay as exact scaled integers. */

export type StockRequestStatus =
  | "draft"
  | "submitted"
  | "approved"
  | "rejected"
  | "cancelled"
  | "partially_fulfilled"
  | "fulfilled"
  | "closed";

export interface StockRequesterCandidate {
  id: string;
  employee_number: string;
  display_name: string;
  department_id: string;
  department_code: string;
  department_name: string;
}

export interface StockRequestDepartment {
  id: string;
  code: string;
  name: string;
}

export interface StockRequesterCandidatesResponse { employees: StockRequesterCandidate[] }
export interface StockRequestDepartmentsResponse { departments: StockRequestDepartment[] }

export interface StockRequestSummary {
  id: string;
  request_number: string;
  requester_employee_id: string;
  requester_employee_number: string | null;
  requester_name: string | null;
  department_id: string;
  department_code: string | null;
  department_name: string | null;
  needed_by: string | null;
  status: StockRequestStatus;
  version: number;
  line_count: number;
  requested_quantity_minor: number;
  approved_quantity_minor: number;
  issued_quantity_minor: number;
  created_at: string;
  updated_at: string;
}

export interface StockRequestLine {
  id: string;
  line_number: number;
  item_id: string;
  item_number: string;
  item_name: string;
  unit_label: string;
  quantity_scale: number;
  requested_quantity_minor: number;
  approved_quantity_minor: number | null;
  issued_quantity_minor: number;
  remaining_quantity_minor: number;
}

export interface StockRequestEvent {
  event_type: string;
  from_status: StockRequestStatus | null;
  to_status: StockRequestStatus;
  request_version: number;
  note: string | null;
  created_at: string;
}

export interface StockRequestFulfilmentLine {
  request_line_id: string;
  item_id: string;
  item_number: string;
  item_name: string;
  store_id: string;
  store_number: string;
  store_name: string;
  quantity_minor: number;
  quantity_scale: number;
  unit_label: string;
}

export interface StockRequestFulfilment {
  id: string;
  movement_id: string;
  movement_number: string;
  effective_on: string;
  quantity_minor: number;
  created_at: string;
  lines: StockRequestFulfilmentLine[];
}

export interface StockRequest extends StockRequestSummary {
  purpose: string;
  submitted_at: string | null;
  decided_at: string | null;
  decision_note: string | null;
  cancelled_at: string | null;
  cancellation_note: string | null;
  closed_at: string | null;
  closure_note: string | null;
  lines: StockRequestLine[];
  events: StockRequestEvent[];
  fulfilments: StockRequestFulfilment[];
}

export interface StockRequestsResponse { requests: StockRequestSummary[] }

export interface StockRequestListParams {
  page?: number;
  per_page?: number;
  search?: string;
  status?: StockRequestStatus;
  requester_employee_id?: string;
  department_id?: string;
}

export interface StockRequestLineInput {
  item_id: string;
  requested_quantity_minor: number;
}

export interface CreateStockRequestInput {
  requester_employee_id: string;
  department_id: string;
  purpose: string;
  needed_by: string | null;
  idempotency_key: string;
  lines: StockRequestLineInput[];
}

export interface UpdateStockRequestInput extends CreateStockRequestInput { expected_version: number }

export interface StockRequestVersionCommand { expected_version: number; idempotency_key: string }

export interface ApproveStockRequestInput extends StockRequestVersionCommand {
  note: string | null;
  lines: Array<{ request_line_id: string; approved_quantity_minor: number }>;
}

export interface StockRequestReasonCommand extends StockRequestVersionCommand { reason: string }

export interface CloseStockRequestInput extends StockRequestVersionCommand { note: string | null }

export interface StockRequestBalancePreview {
  item_id: string;
  store_id: string;
  store_number: string;
  store_name: string;
  on_hand_minor: number;
  quantity_scale: number;
  unit_label: string;
  version: number;
}

export interface StockRequestFulfilmentPreview {
  request: StockRequest;
  balances: StockRequestBalancePreview[];
}

export interface FulfilStockRequestInput {
  expected_request_version: number;
  effective_on: string;
  reason: string | null;
  idempotency_key: string;
  lines: Array<{
    request_line_id: string;
    store_id: string;
    quantity_minor: number;
    expected_balance_version: number;
  }>;
}

export interface FulfilStockRequestResponse {
  request: StockRequest;
  movement_id: string;
  movement_number: string;
}

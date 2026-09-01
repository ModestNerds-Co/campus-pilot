/** Closed client contracts for Facilities operations. */

export type FacilityLocationKind = "site" | "building" | "floor" | "room" | "external_area";
export type FacilityPriority = "low" | "normal" | "high" | "urgent";
export type FacilityRequestStatus = "open" | "assigned" | "resolved" | "closed" | "cancelled";
export type FacilityWorkOrderStatus = "assigned" | "in_progress" | "ready_for_inspection" | "completed" | "cancelled";
export type FacilityInspectionOutcome = "pass" | "fail";

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

export interface FacilityLocation {
  id: string;
  parent_id: string | null;
  parent_name: string | null;
  kind: FacilityLocationKind;
  code: string;
  name: string;
  status: "active" | "archived";
  capacity: number | null;
  notes: string | null;
  version: number;
  child_count: number;
  created_at: string;
  updated_at: string;
}

export interface FacilityServiceRequestSummary {
  id: string;
  reference: string;
  location_id: string;
  location_name: string;
  reporter_user_id: string;
  reporter_name: string;
  priority: FacilityPriority;
  summary: string;
  status: FacilityRequestStatus;
  version: number;
  work_order_id: string | null;
  work_order_reference: string | null;
  created_at: string;
  updated_at: string;
}

export interface FacilityEvent {
  id: string;
  service_request_id: string | null;
  work_order_id: string | null;
  event_type: string;
  actor_id: string;
  actor_name: string;
  metadata: Record<string, unknown>;
  created_at: string;
}

export interface FacilityServiceRequestRecord {
  request: FacilityServiceRequestSummary;
  description: string;
  resolution_summary: string | null;
  resolved_at: string | null;
  closure_reason: string | null;
  closed_at: string | null;
  cancellation_reason: string | null;
  cancelled_at: string | null;
  history: FacilityEvent[];
}

export interface FacilityWorkOrderSummary {
  id: string;
  reference: string;
  service_request_id: string;
  service_request_reference: string;
  location_id: string;
  location_name: string;
  assigned_employee_id: string;
  assigned_employee_number: string;
  assigned_employee_name: string;
  title: string;
  target_date: string | null;
  status: FacilityWorkOrderStatus;
  version: number;
  inspection_count: number;
  created_at: string;
  updated_at: string;
}

export interface FacilityInspection {
  id: string;
  outcome: FacilityInspectionOutcome;
  notes: string;
  inspected_by: string;
  inspector_name: string;
  created_at: string;
}

export interface FacilityWorkOrderRecord {
  work_order: FacilityWorkOrderSummary;
  instructions: string | null;
  started_at: string | null;
  completion_summary: string | null;
  completion_submitted_at: string | null;
  completed_at: string | null;
  cancellation_reason: string | null;
  cancelled_at: string | null;
  inspections: FacilityInspection[];
  history: FacilityEvent[];
}

export interface EmployeeReference {
  id: string;
  account_id: string | null;
  employee_number: string;
  display_name: string;
  work_email: string | null;
  phone: string | null;
  employment_status: string;
}

export interface FacilityReferences {
  locations: FacilityLocation[];
  employees: EmployeeReference[];
}

export interface LocationPayload {
  parent_id: string | null;
  kind: FacilityLocationKind;
  code: string;
  name: string;
  capacity: number | null;
  notes: string | null;
}

export interface ServiceRequestPayload {
  location_id: string;
  priority: FacilityPriority;
  summary: string;
  description: string;
}

export interface WorkOrderPayload {
  service_request_id: string;
  assigned_employee_id: string;
  title: string;
  instructions: string | null;
  target_date: string | null;
}

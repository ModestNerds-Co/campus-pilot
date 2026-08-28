export type DirectoryStatus = "active" | "inactive";
export type EmploymentStatus = "active" | "inactive" | "suspended" | "terminated";
export type HrImportStatus = "uploaded" | "preview_ready" | "committed";
export type HrImportDateFormat = "yyyy_mm_dd" | "dd_mm_yyyy" | "mm_dd_yyyy";

export interface HrImportRecord {
  id: string;
  entity_key: "employees";
  file_name: string;
  content_type: string;
  source_format: "csv" | "xlsx";
  source_size_bytes: number;
  source_row_count: number;
  source_headers: string[];
  status: HrImportStatus;
  created_at: string;
  latest_preview_id: string | null;
  mapping_version: number | null;
  ready_rows: number | null;
  invalid_rows: number | null;
  duplicate_rows: number | null;
  created_rows: number | null;
  skipped_rows: number | null;
  failed_rows: number | null;
  committed_at: string | null;
}

export interface HrImportMapping {
  columns: Record<string, string>;
  date_format: HrImportDateFormat | null;
}

export interface HrImportPreviewRow {
  id: string;
  row_number: number;
  canonical_data: Record<string, unknown>;
  outcome: "ready" | "invalid" | "duplicate";
  issues: string[];
  duplicate_record_id: string | null;
}

export interface HrImportPreview {
  id: string;
  import_id: string;
  mapping_version: number;
  mapping: HrImportMapping;
  ready_rows: number;
  invalid_rows: number;
  duplicate_rows: number;
  created_at: string;
  rows: HrImportPreviewRow[];
  total_rows: number;
}

export interface HrImportCommit {
  id: string;
  import_id: string;
  preview_id: string;
  created_rows: number;
  skipped_rows: number;
  failed_rows: number;
  committed_at: string;
}

export interface HrImportsResponse { imports: HrImportRecord[] }

export interface Department {
  id: string;
  code: string;
  name: string;
  status: DirectoryStatus;
  notes: string | null;
}

export interface Position {
  id: string;
  department_id: string | null;
  code: string;
  title: string;
  status: DirectoryStatus;
  notes: string | null;
}

export interface EmployeeReference {
  id: string;
  account_id: string | null;
  employee_number: string;
  display_name: string;
  work_email: string | null;
  phone: string | null;
  employment_status: EmploymentStatus;
}

export interface Employee extends EmployeeReference {
  account_email: string | null;
  first_names: string | null;
  surname: string | null;
  department_id: string | null;
  department_name: string | null;
  position_id: string | null;
  position_title: string | null;
  hire_date: string | null;
  end_date: string | null;
}

export interface DirectoryListParams {
  page?: number;
  per_page?: number;
  search?: string;
  status?: DirectoryStatus;
}

export interface EmployeeListParams extends Omit<DirectoryListParams, "status"> {
  status?: EmploymentStatus;
  department_id?: string;
  position_id?: string;
  account_linked?: boolean;
}

export interface DepartmentInput {
  code: string;
  name: string;
  status?: DirectoryStatus;
  notes?: string | null;
}

export interface PositionInput {
  department_id?: string | null;
  code: string;
  title: string;
  status?: DirectoryStatus;
  notes?: string | null;
}

export interface EmployeeInput {
  employee_number: string;
  display_name: string;
  first_names?: string | null;
  surname?: string | null;
  work_email?: string | null;
  phone?: string | null;
  department_id?: string | null;
  position_id?: string | null;
  account_id?: string | null;
  employment_status?: EmploymentStatus;
  hire_date?: string | null;
  end_date?: string | null;
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

export interface DepartmentsResponse { departments: Department[] }
export interface PositionsResponse { positions: Position[] }
export interface EmployeesResponse { employees: Employee[] }

export type EmploymentType = "permanent" | "fixed_term" | "temporary" | "casual" | "contractor" | "intern";
export type EngagementStatus = "draft" | "active" | "ended" | "cancelled";

export interface EmploymentEngagement {
  id: string;
  employee_id: string;
  employee_number: string;
  employee_name: string;
  reference: string | null;
  employment_type: EmploymentType;
  department_id: string | null;
  department_name: string | null;
  position_id: string | null;
  position_title: string | null;
  status: EngagementStatus;
  start_date: string | null;
  end_date: string | null;
  workload_basis_points: number;
  notes: string | null;
}

export interface EmploymentEngagementInput {
  employee_id?: string;
  reference?: string | null;
  employment_type: EmploymentType;
  department_id?: string | null;
  position_id?: string | null;
  status: EngagementStatus;
  start_date: string;
  end_date?: string | null;
  workload_basis_points: number;
  notes?: string | null;
}

export interface EmploymentEngagementListParams {
  page?: number;
  per_page?: number;
  search?: string;
  employee_id?: string;
  status?: EngagementStatus;
  employment_type?: EmploymentType;
}

export interface EmploymentEngagementsResponse { employment_engagements: EmploymentEngagement[] }

export type AvailabilityKind = "leave" | "training" | "medical" | "personal" | "other";
export type AvailabilityStatus = "draft" | "submitted" | "approved" | "rejected" | "cancelled";

export interface EmployeeAvailability {
  id: string;
  employee_id: string;
  employee_number: string;
  employee_name: string;
  kind: AvailabilityKind;
  starts_at: string;
  ends_at: string;
  status: AvailabilityStatus;
  notes: string | null;
  decided_by: string | null;
  decided_by_name: string | null;
  decided_at: string | null;
}

export interface EmployeeAvailabilityInput {
  employee_id?: string;
  kind: AvailabilityKind;
  starts_at: string;
  ends_at: string;
  status: AvailabilityStatus;
  notes?: string | null;
}

export interface EmployeeAvailabilityListParams {
  page?: number;
  per_page?: number;
  search?: string;
  employee_id?: string;
  status?: AvailabilityStatus;
  kind?: AvailabilityKind;
  from?: string;
  to?: string;
}

export interface EmployeeAvailabilityResponse { availability_periods: EmployeeAvailability[] }

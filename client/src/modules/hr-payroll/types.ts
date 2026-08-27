export type DirectoryStatus = "active" | "inactive";
export type EmploymentStatus = "active" | "inactive" | "suspended" | "terminated";

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

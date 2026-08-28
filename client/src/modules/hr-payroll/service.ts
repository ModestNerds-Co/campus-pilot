import { AxiosError } from "axios";

import { httpClient } from "@/lib/http-client";

import type {
  ApiEnvelope,
  Department,
  DepartmentInput,
  DepartmentsResponse,
  DirectoryListParams,
  EmployeeAvailability,
  EmployeeAvailabilityInput,
  EmployeeAvailabilityListParams,
  EmployeeAvailabilityResponse,
  Employee,
  EmployeeInput,
  EmployeeListParams,
  EmployeesResponse,
  EmploymentEngagement,
  EmploymentEngagementInput,
  EmploymentEngagementListParams,
  EmploymentEngagementsResponse,
  HrImportCommit,
  HrImportMapping,
  HrImportPreview,
  HrImportRecord,
  HrImportsResponse,
  Position,
  PositionInput,
  PositionsResponse,
} from "./types";

const BASE_URL = "/api/1.0/hr-payroll";

async function request<T>(work: () => Promise<{ data: ApiEnvelope<T> }>): Promise<ApiEnvelope<T>> {
  try {
    return (await work()).data;
  } catch (error) {
    if (error instanceof AxiosError && error.response) return error.response.data as ApiEnvelope<T>;
    throw error;
  }
}

export function hrResponseMessage<T>(response: ApiEnvelope<T>, fallback: string): string {
  if (response.message) return response.message;
  const issue = response.issues?.[0];
  if (typeof issue === "string") return issue;
  return issue?.detail || fallback;
}

export const hrPayrollService = {
  listImports: (params?: { page?: number; per_page?: number }) =>
    request<HrImportsResponse>(() => httpClient.get(`${BASE_URL}/imports`, { params })),
  uploadImport: (file: File) => {
    const form = new FormData();
    form.append("file", file);
    return request<HrImportRecord>(() => httpClient.post(`${BASE_URL}/imports`, form));
  },
  getImport: (id: string) =>
    request<HrImportRecord>(() => httpClient.get(`${BASE_URL}/imports/${id}`)),
  createImportPreview: (id: string, mapping: HrImportMapping) =>
    request<HrImportPreview>(() => httpClient.put(`${BASE_URL}/imports/${id}/mapping`, mapping)),
  getImportPreview: (id: string, params?: { page?: number; per_page?: number }) =>
    request<HrImportPreview>(() => httpClient.get(`${BASE_URL}/imports/${id}/preview`, { params })),
  commitImport: (id: string, previewId: string) =>
    request<HrImportCommit>(() => httpClient.post(`${BASE_URL}/imports/${id}/commit`, { preview_id: previewId })),

  listDepartments: (params?: DirectoryListParams) =>
    request<DepartmentsResponse>(() => httpClient.get(`${BASE_URL}/departments`, { params })),
  createDepartment: (data: DepartmentInput) =>
    request<Department>(() => httpClient.post(`${BASE_URL}/departments`, data)),
  updateDepartment: (id: string, data: Partial<DepartmentInput>) =>
    request<Department>(() => httpClient.put(`${BASE_URL}/departments/${id}`, data)),
  deleteDepartment: (id: string) =>
    request<{ success: boolean }>(() => httpClient.delete(`${BASE_URL}/departments/${id}`)),

  listPositions: (params?: DirectoryListParams) =>
    request<PositionsResponse>(() => httpClient.get(`${BASE_URL}/positions`, { params })),
  createPosition: (data: PositionInput) =>
    request<Position>(() => httpClient.post(`${BASE_URL}/positions`, data)),
  updatePosition: (id: string, data: Partial<PositionInput>) =>
    request<Position>(() => httpClient.put(`${BASE_URL}/positions/${id}`, data)),
  deletePosition: (id: string) =>
    request<{ success: boolean }>(() => httpClient.delete(`${BASE_URL}/positions/${id}`)),

  listEmployees: (params?: EmployeeListParams) =>
    request<EmployeesResponse>(() => httpClient.get(`${BASE_URL}/employees`, { params })),
  createEmployee: (data: EmployeeInput) =>
    request<Employee>(() => httpClient.post(`${BASE_URL}/employees`, data)),
  updateEmployee: (id: string, data: Partial<EmployeeInput>) =>
    request<Employee>(() => httpClient.put(`${BASE_URL}/employees/${id}`, data)),
  linkEmployeeAccount: (id: string, accountId: string | null) =>
    request<Employee>(() => httpClient.put(`${BASE_URL}/employees/${id}/account`, { account_id: accountId })),
  deleteEmployee: (id: string) =>
    request<{ success: boolean }>(() => httpClient.delete(`${BASE_URL}/employees/${id}`)),

  listEmploymentEngagements: (params?: EmploymentEngagementListParams) =>
    request<EmploymentEngagementsResponse>(() => httpClient.get(`${BASE_URL}/employment-engagements`, { params })),
  createEmploymentEngagement: (data: EmploymentEngagementInput & { employee_id: string }) =>
    request<EmploymentEngagement>(() => httpClient.post(`${BASE_URL}/employment-engagements`, data)),
  updateEmploymentEngagement: (id: string, data: EmploymentEngagementInput) =>
    request<EmploymentEngagement>(() => httpClient.put(`${BASE_URL}/employment-engagements/${id}`, data)),
  deleteEmploymentEngagement: (id: string) =>
    request<{ success: boolean }>(() => httpClient.delete(`${BASE_URL}/employment-engagements/${id}`)),

  listEmployeeAvailability: (params?: EmployeeAvailabilityListParams) =>
    request<EmployeeAvailabilityResponse>(() => httpClient.get(`${BASE_URL}/availability`, { params })),
  createEmployeeAvailability: (data: EmployeeAvailabilityInput & { employee_id: string }) =>
    request<EmployeeAvailability>(() => httpClient.post(`${BASE_URL}/availability`, data)),
  updateEmployeeAvailability: (id: string, data: EmployeeAvailabilityInput) =>
    request<EmployeeAvailability>(() => httpClient.put(`${BASE_URL}/availability/${id}`, data)),
  deleteEmployeeAvailability: (id: string) =>
    request<{ success: boolean }>(() => httpClient.delete(`${BASE_URL}/availability/${id}`)),
};

import { AxiosError } from "axios";

import { httpClient } from "@/lib/http-client";

import type {
  ApiEnvelope,
  Department,
  DepartmentInput,
  DepartmentsResponse,
  DirectoryListParams,
  Employee,
  EmployeeInput,
  EmployeeListParams,
  EmployeesResponse,
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

export const hrPayrollService = {
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
};

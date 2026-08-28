// Campus Pilot Academics HTTP client.

import { AxiosError } from "axios";

import { httpClient } from "@/lib/http-client";

import type {
  AcademicYear,
  AcademicYearInput,
  AcademicYearsResponse,
  AcademicTerm,
  AcademicTermInput,
  AcademicTermsResponse,
  ApiEnvelope,
  ClassesResponse,
  ClassGroup,
  ClassGroupInput,
  ListParams,
  Subject,
  SubjectInput,
  SubjectsResponse,
  TeacherCandidatesResponse,
  TeacherProfile,
  TeachersResponse,
  TeachingAssignment,
  TeachingAssignmentInput,
  TeachingAssignmentsResponse,
} from "./types";

const BASE_URL = "/api/1.0/academics";

async function request<T>(work: () => Promise<{ data: ApiEnvelope<T> }>): Promise<ApiEnvelope<T>> {
  try {
    return (await work()).data;
  } catch (error) {
    if (error instanceof AxiosError && error.response) return error.response.data as ApiEnvelope<T>;
    throw error;
  }
}

export const academicsService = {
  listAcademicYears: (params?: ListParams) => request<AcademicYearsResponse>(() => httpClient.get(`${BASE_URL}/academic-years`, { params })),
  createAcademicYear: (data: AcademicYearInput) => request<AcademicYear>(() => httpClient.post(`${BASE_URL}/academic-years`, data)),
  updateAcademicYear: (id: string, data: AcademicYearInput) => request<AcademicYear>(() => httpClient.put(`${BASE_URL}/academic-years/${id}`, data)),
  deleteAcademicYear: (id: string) => request<{ deleted: boolean }>(() => httpClient.delete(`${BASE_URL}/academic-years/${id}`)),

  listAcademicTerms: (params?: ListParams) => request<AcademicTermsResponse>(() => httpClient.get(`${BASE_URL}/terms`, { params })),
  createAcademicTerm: (data: AcademicTermInput) => request<AcademicTerm>(() => httpClient.post(`${BASE_URL}/terms`, data)),
  updateAcademicTerm: (id: string, data: AcademicTermInput) => request<AcademicTerm>(() => httpClient.put(`${BASE_URL}/terms/${id}`, data)),
  deleteAcademicTerm: (id: string) => request<{ deleted: boolean }>(() => httpClient.delete(`${BASE_URL}/terms/${id}`)),

  listSubjects: (params?: ListParams) => request<SubjectsResponse>(() => httpClient.get(`${BASE_URL}/subjects`, { params })),
  createSubject: (data: SubjectInput) => request<Subject>(() => httpClient.post(`${BASE_URL}/subjects`, data)),
  updateSubject: (id: string, data: SubjectInput) => request<Subject>(() => httpClient.put(`${BASE_URL}/subjects/${id}`, data)),
  deleteSubject: (id: string) => request<{ deleted: boolean }>(() => httpClient.delete(`${BASE_URL}/subjects/${id}`)),

  listTeacherCandidates: (search?: string) => request<TeacherCandidatesResponse>(() => httpClient.get(`${BASE_URL}/teacher-candidates`, { params: { search: search || undefined } })),
  listTeachers: (params?: ListParams) => request<TeachersResponse>(() => httpClient.get(`${BASE_URL}/teachers`, { params })),
  createTeacher: (employeeId: string) => request<TeacherProfile>(() => httpClient.post(`${BASE_URL}/teachers`, { employee_id: employeeId, status: "active" })),
  updateTeacher: (id: string, status: "active" | "inactive") => request<TeacherProfile>(() => httpClient.put(`${BASE_URL}/teachers/${id}`, { status })),
  deleteTeacher: (id: string) => request<{ deleted: boolean }>(() => httpClient.delete(`${BASE_URL}/teachers/${id}`)),

  listClasses: (params?: ListParams) => request<ClassesResponse>(() => httpClient.get(`${BASE_URL}/classes`, { params })),
  createClass: (data: ClassGroupInput) => request<ClassGroup>(() => httpClient.post(`${BASE_URL}/classes`, data)),
  updateClass: (id: string, data: ClassGroupInput) => request<ClassGroup>(() => httpClient.put(`${BASE_URL}/classes/${id}`, data)),
  deleteClass: (id: string) => request<{ deleted: boolean }>(() => httpClient.delete(`${BASE_URL}/classes/${id}`)),

  listTeachingAssignments: (params?: ListParams) => request<TeachingAssignmentsResponse>(() => httpClient.get(`${BASE_URL}/teaching-assignments`, { params })),
  createTeachingAssignment: (data: TeachingAssignmentInput) => request<TeachingAssignment>(() => httpClient.post(`${BASE_URL}/teaching-assignments`, data)),
  updateTeachingAssignment: (id: string, data: TeachingAssignmentInput) => request<TeachingAssignment>(() => httpClient.put(`${BASE_URL}/teaching-assignments/${id}`, data)),
  deleteTeachingAssignment: (id: string) => request<{ deleted: boolean }>(() => httpClient.delete(`${BASE_URL}/teaching-assignments/${id}`)),
};

export function responseMessage(response: Pick<ApiEnvelope<unknown>, "issues" | "message">, fallback: string) {
  const issue = response.issues?.[0];
  if (typeof issue === "string") return issue;
  return issue?.detail || response.message || fallback;
}

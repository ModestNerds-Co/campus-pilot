// Campus Pilot Academics transport types.

export type DirectoryStatus = "active" | "inactive";
export type AcademicYearStatus = "planned" | "active" | "closed";
export type AssessmentCycleStatus = "draft" | "open" | "closed";
export type AssessmentKind = "assignment" | "quiz" | "test" | "project" | "exam" | "practical" | "other";

export interface AcademicYear {
  id: string;
  name: string;
  starts_on: string;
  ends_on: string;
  status: AcademicYearStatus;
}

export interface AcademicTerm {
  id: string;
  academic_year_id: string;
  academic_year_name: string;
  code: string;
  name: string;
  starts_on: string;
  ends_on: string;
  status: AcademicYearStatus;
}

export interface AcademicGradeLevel {
  id: string;
  code: string;
  name: string;
  sequence_number: number;
  status: DirectoryStatus;
}

export interface Subject {
  id: string;
  code: string;
  name: string;
  status: DirectoryStatus;
}

export interface TeacherProfile {
  id: string;
  employee_id: string;
  employee_number: string;
  display_name: string;
  work_email: string | null;
  phone: string | null;
  employment_status: string;
  status: DirectoryStatus;
}

export interface EmployeeCandidate {
  id: string;
  account_id: string | null;
  employee_number: string;
  display_name: string;
  work_email: string | null;
  phone: string | null;
  employment_status: string;
}

export interface ClassGroup {
  id: string;
  academic_year_id: string;
  academic_year_name: string;
  code: string;
  name: string;
  grade_level_id: string | null;
  grade_level: string | null;
  status: DirectoryStatus;
}

export interface TeachingAssignment {
  id: string;
  academic_year_id: string;
  academic_year_name: string;
  class_group_id: string;
  class_group_name: string;
  subject_id: string;
  subject_name: string;
  teacher_profile_id: string;
  employee_id: string;
  teacher_name: string;
  periods_per_cycle: number;
  status: DirectoryStatus;
}

export interface AssessmentCycle {
  id: string;
  academic_term_id: string;
  academic_term_code: string;
  academic_term_name: string;
  academic_year_id: string;
  academic_year_name: string;
  code: string;
  name: string;
  status: AssessmentCycleStatus;
  component_count: number;
  created_at: string;
  updated_at: string;
}

export interface AssessmentComponent {
  id: string;
  assessment_cycle_id: string;
  assessment_cycle_name: string;
  teaching_assignment_id: string;
  class_group_id: string;
  class_group_name: string;
  subject_id: string;
  subject_name: string;
  teacher_profile_id: string;
  teacher_name: string;
  code: string;
  name: string;
  assessment_kind: AssessmentKind;
  maximum_marks: number;
  weight_basis_points: number;
  occurs_on: string | null;
  status: DirectoryStatus;
  created_at: string;
  updated_at: string;
}

export interface AcademicYearInput {
  name: string;
  starts_on: string;
  ends_on: string;
  status?: AcademicYearStatus;
}

export interface AcademicTermInput {
  academic_year_id: string;
  code: string;
  name: string;
  starts_on: string;
  ends_on: string;
  status?: AcademicYearStatus;
}

export interface SubjectInput {
  code: string;
  name: string;
  status?: DirectoryStatus;
}

export interface AcademicGradeLevelInput {
  code: string;
  name: string;
  sequence_number: number;
  status?: DirectoryStatus;
}

export interface ClassGroupInput {
  academic_year_id: string;
  code: string;
  name: string;
  grade_level_id?: string | null;
  status?: DirectoryStatus;
}

export interface TeachingAssignmentInput {
  academic_year_id: string;
  class_group_id: string;
  subject_id: string;
  teacher_profile_id: string;
  periods_per_cycle: number;
  status?: DirectoryStatus;
}

export interface AssessmentCycleInput {
  academic_term_id: string;
  code: string;
  name: string;
  status?: AssessmentCycleStatus;
}

export interface AssessmentComponentInput {
  teaching_assignment_id: string;
  code: string;
  name: string;
  assessment_kind: AssessmentKind;
  maximum_marks: number;
  weight_basis_points: number;
  occurs_on: string | null;
  status?: DirectoryStatus;
}

export interface ListParams {
  page?: number;
  per_page?: number;
  search?: string;
  status?: string;
  academic_year_id?: string;
  grade_level_id?: string;
  class_group_id?: string;
  teacher_profile_id?: string;
  academic_term_id?: string;
  teaching_assignment_id?: string;
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

export interface AcademicYearsResponse { academic_years: AcademicYear[] }
export interface AcademicTermsResponse { terms: AcademicTerm[] }
export interface AcademicGradeLevelsResponse { grade_levels: AcademicGradeLevel[] }
export interface SubjectsResponse { subjects: Subject[] }
export interface TeachersResponse { teachers: TeacherProfile[] }
export interface TeacherCandidatesResponse { employees: EmployeeCandidate[] }
export interface ClassesResponse { classes: ClassGroup[] }
export interface TeachingAssignmentsResponse { assignments: TeachingAssignment[] }
export interface AssessmentCyclesResponse { assessment_cycles: AssessmentCycle[] }
export interface AssessmentComponentsResponse { assessment_components: AssessmentComponent[] }

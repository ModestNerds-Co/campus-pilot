// Campus Pilot Attendance transport contracts.

export type AttendancePeriod = "full_day" | "morning" | "afternoon";
export type AttendanceMarkStatus = "unmarked" | "present" | "absent" | "late" | "excused";
export type AttendanceRegisterStatus = "draft" | "submitted";

export interface AttendanceTermReference {
  id: string;
  academic_year_id: string;
  academic_year_name: string;
  code: string;
  name: string;
  starts_on: string;
  ends_on: string;
}

export interface AttendanceClassReference {
  id: string;
  code: string;
  name: string;
  grade_level: string | null;
}

export interface AttendanceReferenceData {
  term: AttendanceTermReference;
  classes: AttendanceClassReference[];
}

export interface AttendanceRegisterSummary {
  id: string;
  academic_term_id: string;
  academic_term_name: string;
  class_group_id: string;
  class_group_name: string;
  attendance_date: string;
  period: AttendancePeriod;
  status: AttendanceRegisterStatus;
  version: number;
  learner_count: number;
  present_count: number;
  absent_count: number;
  late_count: number;
  excused_count: number;
  unmarked_count: number;
  created_at: string;
  submitted_at: string | null;
}

export interface AttendanceMark {
  id: string;
  enrolment_id: string;
  learner_id: string;
  learner_number: string;
  learner_name: string;
  mark: AttendanceMarkStatus;
  minutes_late: number | null;
  note: string | null;
  version: number;
  marked_at: string | null;
}

export interface AttendanceRegister extends AttendanceRegisterSummary {
  marks: AttendanceMark[];
  reopened_at: string | null;
  reopen_reason: string | null;
}

export interface AttendanceRegisterInput {
  academic_term_id: string;
  class_group_id: string;
  attendance_date: string;
  period: AttendancePeriod;
  idempotency_key: string;
}

export interface AttendanceMarkInput {
  learner_id: string;
  mark: AttendanceMarkStatus;
  minutes_late: number | null;
  note: string | null;
}

export interface AttendanceRegisterListParams {
  page?: number;
  per_page?: number;
  date_from?: string;
  date_to?: string;
  class_group_id?: string;
  period?: AttendancePeriod;
  status?: AttendanceRegisterStatus;
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

export interface AttendanceRegistersResponse {
  registers: AttendanceRegisterSummary[];
}

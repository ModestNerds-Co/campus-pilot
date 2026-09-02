// Campus Pilot Attendance transport contracts.

export type AttendancePeriod = "full_day" | "morning" | "afternoon" | `lesson:${string}`;
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

export interface LearnerAttendanceHistoryEntry {
  register_id: string;
  class_group_id: string;
  class_group_name: string;
  attendance_date: string;
  period: AttendancePeriod;
  mark: Exclude<AttendanceMarkStatus, "unmarked">;
  minutes_late: number | null;
  note: string | null;
  submitted_at: string;
}

export interface LearnerAttendanceHistory {
  learner_id: string;
  learner_number: string;
  learner_name: string;
  present_count: number;
  absent_count: number;
  late_count: number;
  excused_count: number;
  entries: LearnerAttendanceHistoryEntry[];
}

export interface LearnerAttendanceHistoryParams {
  page?: number;
  per_page?: number;
  date_from?: string;
  date_to?: string;
}

export type AttendanceLessonSessionStatus = "scheduled" | "open" | "completed" | "cancelled";

export interface AttendanceLessonSession {
  id: string;
  academic_term_id: string;
  academic_term_name: string;
  class_group_id: string;
  class_group_name: string;
  teaching_assignment_id: string;
  subject_id: string;
  subject_name: string;
  teacher_name: string;
  timetable_run_id: string;
  session_date: string;
  day_key: string;
  period_key: string;
  status: AttendanceLessonSessionStatus;
  version: number;
  register_id: string | null;
  cancellation_reason: string | null;
  opened_at: string | null;
  completed_at: string | null;
  cancelled_at: string | null;
  created_at: string;
}

export interface AttendanceLessonSessionListParams {
  page?: number;
  per_page?: number;
  date_from?: string;
  date_to?: string;
  class_group_id?: string;
  status?: AttendanceLessonSessionStatus;
}

export interface AttendanceLessonSessionsResponse {
  sessions: AttendanceLessonSession[];
}

export interface SyncAttendanceLessonSessionsResponse {
  timetable_run_id: string;
  date_from: string;
  date_to: string;
  created_count: number;
  existing_count: number;
}

export type AttendanceExceptionStatus = "open" | "acknowledged" | "resolved";
export type AttendanceExceptionMark = "absent" | "late" | "excused";

export interface AttendanceException {
  id: string;
  register_id: string;
  enrolment_id: string;
  learner_id: string;
  learner_number: string;
  learner_name: string;
  class_group_id: string;
  class_group_name: string;
  source_register_version: number;
  attendance_date: string;
  period: string;
  mark: AttendanceExceptionMark;
  minutes_late: number | null;
  attendance_note: string | null;
  source_submitted_at: string;
  status: AttendanceExceptionStatus;
  version: number;
  acknowledged_at: string | null;
  acknowledgement_note: string | null;
  resolved_at: string | null;
  resolution: string | null;
  reopened_at: string | null;
  reopen_reason: string | null;
  created_at: string;
  updated_at: string;
}

export interface AttendanceExceptionListParams {
  page?: number;
  per_page?: number;
  date_from?: string;
  date_to?: string;
  class_group_id?: string;
  status?: AttendanceExceptionStatus;
  mark?: AttendanceExceptionMark;
}

export interface AttendanceExceptionsResponse {
  exceptions: AttendanceException[];
}

export interface AttendanceLessonSessionsSearch {
  page: number;
  date_from: string;
  date_to: string;
  class_group_id: string;
  status: "all" | AttendanceLessonSessionStatus;
}

export interface AttendanceExceptionsSearch {
  page: number;
  date_from: string;
  date_to: string;
  class_group_id: string;
  status: "all" | AttendanceExceptionStatus;
  mark: "all" | AttendanceExceptionMark;
}

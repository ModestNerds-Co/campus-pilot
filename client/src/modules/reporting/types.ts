export type AcademicReportBatchStatus = "draft" | "reviewed" | "published";
export type ProgressionOutcome = "not_applicable" | "pending" | "promoted" | "retained" | "completed";

export interface ReportingSource {
  assessment_cycle_id: string;
  assessment_cycle_name: string;
  academic_term_id: string;
  academic_term_name: string;
  academic_term_starts_on: string;
  academic_term_ends_on: string;
  academic_year_id: string;
  academic_year_name: string;
  class_group_id: string;
  class_group_name: string;
  component_count: number;
  published_sheet_count: number;
}

export interface GradeLevelReference {
  id: string;
  code: string;
  name: string;
  sort_order: number;
}

export interface GradingBand {
  id: string;
  code: string;
  label: string;
  minimum_basis_points: number;
  is_pass: boolean;
}

export interface GradingBandInput {
  code: string;
  label: string;
  minimum_basis_points: number;
  is_pass: boolean;
}

export interface GradingScheme {
  id: string;
  name: string;
  description: string | null;
  is_default: boolean;
  status: "active" | "retired";
  version: number;
  bands: GradingBand[];
  created_at: string;
  updated_at: string;
}

export interface ReportingReferenceData {
  sources: ReportingSource[];
  grading_schemes: GradingScheme[];
  grade_levels: GradeLevelReference[];
}

export interface ReportBatchSummary {
  id: string;
  assessment_cycle_id: string;
  assessment_cycle_name: string;
  academic_term_id: string;
  academic_term_name: string;
  academic_year_id: string;
  academic_year_name: string;
  class_group_id: string;
  class_group_name: string;
  grading_scheme_id: string;
  grading_scheme_name: string;
  grading_scheme_version: number;
  status: AcademicReportBatchStatus;
  version: number;
  learner_count: number;
  graded_subject_count: number;
  incomplete_subject_count: number;
  created_at: string;
  reviewed_at: string | null;
  published_at: string | null;
}

export interface SubjectResult {
  id: string;
  teaching_assignment_id: string;
  subject_id: string;
  subject_name: string;
  result_status: "graded" | "exempt" | "incomplete";
  percentage_basis_points: number | null;
  grade_code: string | null;
  grade_label: string | null;
  is_pass: boolean | null;
  scored_component_count: number;
  absent_component_count: number;
  exempt_component_count: number;
}

export interface ReportAttendance {
  present_count: number;
  absent_count: number;
  late_count: number;
  excused_count: number;
  attendance_percentage_basis_points: number | null;
}

export interface ReportCard {
  id: string;
  enrolment_id: string;
  learner_id: string;
  learner_number: string;
  learner_name: string;
  overall_percentage_basis_points: number | null;
  overall_grade_code: string | null;
  overall_grade_label: string | null;
  teacher_comment: string | null;
  reviewer_comment: string | null;
  progression_outcome: ProgressionOutcome;
  target_grade_level_id: string | null;
  target_grade_level_name: string | null;
  version: number;
  subjects: SubjectResult[];
  attendance: ReportAttendance;
}

export interface ReportBatch extends ReportBatchSummary {
  cards: ReportCard[];
  reopened_at: string | null;
  reopen_reason: string | null;
}

export interface ReportBatchesResponse {
  report_batches: ReportBatchSummary[];
}

export interface TranscriptEntry {
  report_batch_id: string;
  assessment_cycle_name: string;
  academic_term_name: string;
  academic_year_name: string;
  class_group_name: string;
  published_at: string;
  overall_percentage_basis_points: number | null;
  overall_grade_code: string | null;
  overall_grade_label: string | null;
  progression_outcome: ProgressionOutcome;
  subjects: SubjectResult[];
}

export interface LearnerTranscript {
  learner_id: string;
  learner_number: string;
  learner_name: string;
  entries: TranscriptEntry[];
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

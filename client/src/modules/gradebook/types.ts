export type GradebookMarkStatus = "unmarked" | "scored" | "absent" | "exempt";
export type GradebookSheetStatus = "draft" | "submitted" | "published";

export interface GradebookComponentReference {
  assessment_component_id: string;
  assessment_component_code: string;
  assessment_component_name: string;
  assessment_kind: string;
  maximum_marks: number;
  weight_basis_points: number;
  occurs_on: string | null;
  assessment_cycle_id: string;
  assessment_cycle_name: string;
  assessment_cycle_status: string;
  academic_term_id: string;
  academic_term_name: string;
  academic_term_starts_on: string;
  academic_term_ends_on: string;
  academic_year_id: string;
  academic_year_name: string;
  teaching_assignment_id: string;
  class_group_id: string;
  class_group_name: string;
  subject_id: string;
  subject_name: string;
  teacher_profile_id: string;
  teacher_name: string;
  mark_sheet_id: string | null;
  mark_sheet_status: GradebookSheetStatus | null;
  mark_sheet_version: number | null;
}

export interface GradebookReferenceData {
  components: GradebookComponentReference[];
}

export interface GradebookSheetSummary {
  id: string;
  assessment_component_id: string;
  assessment_component_code: string;
  assessment_component_name: string;
  assessment_kind: string;
  maximum_marks: number;
  weight_basis_points: number;
  assessment_cycle_id: string;
  assessment_cycle_name: string;
  academic_term_id: string;
  academic_term_name: string;
  academic_year_id: string;
  academic_year_name: string;
  class_group_id: string;
  class_group_name: string;
  subject_id: string;
  subject_name: string;
  teacher_profile_id: string;
  teacher_name: string;
  roster_on: string;
  status: GradebookSheetStatus;
  version: number;
  learner_count: number;
  scored_count: number;
  absent_count: number;
  exempt_count: number;
  unmarked_count: number;
  average_percentage_basis_points: number | null;
  created_at: string;
  submitted_at: string | null;
  published_at: string | null;
}

export interface GradebookMark {
  id: string;
  enrolment_id: string;
  learner_id: string;
  learner_number: string;
  learner_name: string;
  mark_status: GradebookMarkStatus;
  marks_awarded_hundredths: number | null;
  percentage_basis_points: number | null;
  weighted_score_basis_points: number | null;
  note: string | null;
  version: number;
  marked_at: string | null;
}

export interface GradebookSheet extends GradebookSheetSummary {
  marks: GradebookMark[];
  reopened_at: string | null;
  reopen_reason: string | null;
}

export interface GradebookMarkInput {
  learner_id: string;
  mark_status: GradebookMarkStatus;
  marks_awarded_hundredths: number | null;
  note: string | null;
}

export interface GradebookSheetsResponse {
  mark_sheets: GradebookSheetSummary[];
}

export type GradebookMarkImportStatus = "uploaded" | "preview_ready" | "committed";
export type GradebookMarkImportDecimalSeparator = "dot" | "comma";

export interface GradebookMarkImportRecord {
  id: string;
  mark_sheet_id: string;
  file_name: string;
  content_type: string;
  source_format: "csv" | "xlsx";
  source_size_bytes: number;
  source_row_count: number;
  source_headers: string[];
  status: GradebookMarkImportStatus;
  created_at: string;
  latest_preview_id: string | null;
  mapping_version: number | null;
  ready_rows: number | null;
  invalid_rows: number | null;
  duplicate_rows: number | null;
  updated_rows: number | null;
  skipped_rows: number | null;
  failed_rows: number | null;
  committed_at: string | null;
}

export interface GradebookMarkImportMapping {
  columns: Record<string, string>;
  decimal_separator: GradebookMarkImportDecimalSeparator;
  expected_sheet_version: number;
}

export interface GradebookMarkImportPreviewRow {
  id: string;
  row_number: number;
  canonical_data: {
    learner_number?: string | null;
    learner_name?: string;
    mark_status?: GradebookMarkStatus;
    marks_awarded_hundredths?: number | null;
    note?: string | null;
  };
  outcome: "ready" | "invalid" | "duplicate";
  issues: string[];
  duplicate_record_id: string | null;
}

export interface GradebookMarkImportPreview {
  id: string;
  import_id: string;
  mapping_version: number;
  mapping: GradebookMarkImportMapping;
  ready_rows: number;
  invalid_rows: number;
  duplicate_rows: number;
  created_at: string;
  rows: GradebookMarkImportPreviewRow[];
  total_rows: number;
}

export interface GradebookMarkImportCommit {
  id: string;
  import_id: string;
  preview_id: string;
  updated_rows: number;
  skipped_rows: number;
  failed_rows: number;
  committed_at: string;
}

export interface GradebookMarkImportsResponse {
  imports: GradebookMarkImportRecord[];
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

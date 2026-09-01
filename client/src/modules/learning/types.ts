export type LearningSpaceStatus = "draft" | "published" | "archived";
export type LearningUnitStatus = "draft" | "published" | "withdrawn";
export type LearningResourceStatus = "draft" | "published" | "withdrawn";
export type LearningAssignmentStatus = "draft" | "published" | "closed";
export type LearningSubmissionStatus =
  | "draft"
  | "submitted"
  | "revision_requested"
  | "graded";
export type LearningReviewOutcome = "graded" | "revision_requested";
export type LearningAssignmentTab =
  | "brief"
  | "work"
  | "submissions"
  | "rubric";

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

export interface LearningSettings {
  document_series_id: string | null;
  document_series_name: string | null;
  version: number;
  updated_at: string;
}

export interface LearningTermReference {
  id: string;
  academic_year_id: string;
  academic_year_name: string;
  code: string;
  name: string;
  starts_on: string;
  ends_on: string;
}

export interface TeachingAssignmentReference {
  id: string;
  academic_year_id: string;
  academic_year_name: string;
  class_group_id: string;
  class_group_name: string;
  subject_id: string;
  subject_name: string;
  teacher_name: string;
}

export interface LearningReferenceData {
  active_term: LearningTermReference | null;
  assignments: TeachingAssignmentReference[];
}

export interface GovernedFileReference {
  id: string;
  reference: string;
  title: string;
  sensitivity: string;
  status: string;
}

export interface LearningSpaceSummary {
  id: string;
  teaching_assignment_id: string;
  academic_year_id: string;
  academic_year_name: string;
  academic_term_id: string;
  academic_term_name: string;
  class_group_id: string;
  class_group_name: string;
  subject_name: string;
  teacher_name: string;
  title: string;
  summary: string | null;
  status: LearningSpaceStatus;
  version: number;
  unit_count: number;
  published_unit_count: number;
  published_at: string | null;
  archived_at: string | null;
  archive_reason: string | null;
  created_at: string;
  updated_at: string;
}

export interface LearningResource {
  id: string;
  learning_unit_id: string;
  document_file_id: string;
  document: GovernedFileReference | null;
  display_title: string;
  sensitivity_snapshot: string;
  position: number;
  status: LearningResourceStatus;
  version: number;
  published_at: string | null;
  withdrawn_at: string | null;
  withdrawal_reason: string | null;
  created_at: string;
  updated_at: string;
}

export interface LearningUnit {
  id: string;
  learning_space_id: string;
  position: number;
  title: string;
  summary: string | null;
  status: LearningUnitStatus;
  version: number;
  published_at: string | null;
  withdrawn_at: string | null;
  withdrawal_reason: string | null;
  resources: LearningResource[];
  created_at: string;
  updated_at: string;
}

export interface LearningSpace extends LearningSpaceSummary {
  units: LearningUnit[];
}

export interface LearningRubricCriterion {
  id: string;
  learning_assignment_id: string;
  position: number;
  title: string;
  description: string | null;
  max_score_hundredths: number;
  version: number;
}

export interface LearningAssignment {
  id: string;
  learning_unit_id: string;
  learning_space_id: string;
  position: number;
  title: string;
  instructions: string;
  due_at: string;
  max_score_hundredths: number;
  status: LearningAssignmentStatus;
  version: number;
  recipient_count: number;
  submission_count: number;
  published_at: string | null;
  closed_at: string | null;
  close_reason: string | null;
  rubric: LearningRubricCriterion[];
  created_at: string;
  updated_at: string;
}

export interface LearningSubmissionVersion {
  id: string;
  revision_number: number;
  body: string;
  late: boolean;
  submitted_at: string;
}

export interface LearningReviewScore {
  rubric_criterion_id: string;
  earned_score_hundredths: number;
  feedback: string | null;
}

export interface LearningFeedback {
  id: string;
  submission_version_id: string;
  status: string;
  outcome: LearningReviewOutcome | null;
  overall_feedback: string | null;
  total_score_hundredths: number | null;
  version: number;
  scores: LearningReviewScore[];
  released_at: string | null;
}

export interface LearningSubmission {
  id: string;
  learning_assignment_id: string;
  assignment_recipient_id: string;
  learner_id: string;
  enrolment_id: string;
  learner_name: string;
  learner_number: string;
  draft_body: string | null;
  status: LearningSubmissionStatus;
  version: number;
  current_submission_version_id: string | null;
  versions: LearningSubmissionVersion[];
  feedback: LearningFeedback | null;
  created_at: string;
  updated_at: string;
}

export interface LearningProgressEntry {
  learner_id: string;
  enrolment_id: string;
  learner_name: string;
  learner_number: string;
  total_assignments: number;
  not_started: number;
  drafts: number;
  awaiting_feedback: number;
  revision_requested: number;
  graded: number;
  overdue: number;
  completion_percent: number;
  earned_score_hundredths: number;
  possible_score_hundredths: number;
}

export interface LearningSpacesResponse {
  spaces: LearningSpaceSummary[];
}

export interface LearningAssignmentsResponse {
  assignments: LearningAssignment[];
}

export interface LearningSubmissionsResponse {
  submissions: LearningSubmission[];
}

export interface LearningProgressResponse {
  progress: LearningProgressEntry[];
}

export interface LearningFilesResponse {
  files: GovernedFileReference[];
}

export interface LearningDownload {
  url: string;
  expires_in_seconds: number;
}

export interface SpacesSearch {
  q: string;
  status: "all" | LearningSpaceStatus;
  page: number;
}

export interface AssignmentsSearch {
  status: "all" | LearningAssignmentStatus;
  page: number;
}

export interface ProgressSearch {
  q: string;
  page: number;
}

export interface AssignmentDetailSearch {
  tab: LearningAssignmentTab;
  submission_status: "all" | LearningSubmissionStatus;
  submission_page: number;
}

export interface SubmissionSearch {
  version: string;
}

export interface LearningSpaceListParams {
  page?: number;
  per_page?: number;
  search?: string;
  status?: LearningSpaceStatus;
}

export interface LearningAssignmentListParams {
  page?: number;
  per_page?: number;
  status?: LearningAssignmentStatus;
}

export interface LearningSubmissionListParams {
  page?: number;
  per_page?: number;
  status?: LearningSubmissionStatus;
}

export interface LearningProgressListParams {
  page?: number;
  per_page?: number;
  search?: string;
}

export interface CreateLearningSpace {
  teaching_assignment_id: string;
  academic_term_id: string;
  title: string;
  summary: string | null;
}

export interface CreateLearningUnit {
  position: number;
  title: string;
  summary: string | null;
}

export interface CreateLearningResource {
  document_file_id: string;
  display_title: string;
  position: number;
}

export interface CreateLearningAssignment {
  position: number;
  title: string;
  instructions: string;
  due_at: string;
  max_score_hundredths: number;
}

export interface CreateLearningRubricCriterion {
  position: number;
  title: string;
  description: string | null;
  max_score_hundredths: number;
}

export interface LearningRubricScoreInput {
  rubric_criterion_id: string;
  earned_score_hundredths: number;
  feedback: string | null;
}

export interface UpdateLearningFeedbackPayload {
  submission_version_id: string;
  overall_feedback: string | null;
  scores: LearningRubricScoreInput[];
  expected_review_version: number | null;
}

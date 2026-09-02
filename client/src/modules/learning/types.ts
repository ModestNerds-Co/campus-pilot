/** Shared E-learning API contracts for authoring, participation, and review. */

export type LearningSpaceStatus = "draft" | "published" | "archived";
export type LearningUnitStatus = "draft" | "published" | "withdrawn";
export type LearningResourceStatus = "draft" | "published" | "withdrawn";
export type LearningAssignmentStatus = "draft" | "published" | "closed";
export type LearningSubmissionMethod = "text" | "file" | "text_or_file";
export type LearningSubmissionStatus =
  | "draft"
  | "submitted"
  | "revision_requested"
  | "graded";
export type LearningReviewOutcome = "graded" | "revision_requested";
export type LearningQuizStatus = "draft" | "published" | "closed";
export type LearningQuizAttemptStatus = "in_progress" | "submitted";
export type LearningCompletionPolicyStatus = "draft" | "published" | "superseded";
export type LearningCompletionRequirementType = "assignment" | "quiz";
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
  learner_submission_series_id: string | null;
  learner_submission_series_name: string | null;
  version: number;
  updated_at: string;
}

export interface LearningUploadClassificationOption {
  id: string;
  code: string;
  name: string;
  default_sensitivity: string;
}

export interface LearningUploadClassificationOptions {
  resource_series: LearningUploadClassificationOption[];
  learner_submission_series: LearningUploadClassificationOption[];
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
  submission_method: LearningSubmissionMethod;
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
  body: string | null;
  files: LearningSubmissionFile[];
  late: boolean;
  submitted_at: string;
}

export interface LearningSubmissionFile {
  id: string;
  document_file_id: string;
  document_reference: string;
  original_file_name: string;
  media_type: string;
  byte_size: number;
  position: number;
  version: number | null;
  attached_at: string;
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
  draft_files: LearningSubmissionFile[];
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

export interface LearningQuizChoice {
  id: string;
  position: number;
  label: string;
  is_correct?: boolean;
}

export interface LearningQuizQuestion {
  id: string;
  position: number;
  prompt: string;
  points: number;
  version: number;
  choices: LearningQuizChoice[];
}

export interface LearningQuiz {
  id: string;
  learning_unit_id: string;
  learning_space_id: string;
  position: number;
  title: string;
  instructions: string | null;
  opens_at: string | null;
  closes_at: string | null;
  attempt_limit: number;
  pass_score_basis_points: number;
  status: LearningQuizStatus;
  version: number;
  recipient_count: number;
  submitted_attempt_count: number;
  my_attempt_count: number;
  questions: LearningQuizQuestion[];
  published_at: string | null;
  closed_at: string | null;
  close_reason: string | null;
  created_at: string;
  updated_at: string;
}

export interface LearningQuizAttemptAnswer {
  question_id: string;
  selected_choice_id: string;
}

export interface LearningQuizAttempt {
  id: string;
  learning_quiz_id: string;
  learner_id: string;
  enrolment_id: string;
  learner_name: string;
  learner_number: string;
  attempt_number: number;
  status: LearningQuizAttemptStatus;
  version: number;
  answers: LearningQuizAttemptAnswer[];
  started_at: string;
  submitted_at: string | null;
  total_points: number | null;
  earned_points: number | null;
  score_basis_points: number | null;
  passed: boolean | null;
}

export interface LearningCompletionRequirement {
  id: string;
  position: number;
  requirement_type: LearningCompletionRequirementType;
  source_id: string;
  source_title: string;
  minimum_score_basis_points: number;
}

export interface LearningCompletionPolicy {
  id: string;
  learning_space_id: string;
  status: LearningCompletionPolicyStatus;
  version: number;
  requirements: LearningCompletionRequirement[];
  recipient_count: number;
  published_at: string | null;
  created_at: string;
  updated_at: string;
}

export interface LearningCompletionEntry {
  learner_id: string;
  enrolment_id: string;
  learner_name: string;
  learner_number: string;
  required_count: number;
  completed_count: number;
  completion_percent: number;
  complete: boolean;
}

export interface LearningCompletionPage {
  policy: LearningCompletionPolicy | null;
  progress: LearningCompletionEntry[];
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

export interface LearningQuizzesResponse {
  quizzes: LearningQuiz[];
}

export interface LearningQuizAttemptsResponse {
  attempts: LearningQuizAttempt[];
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

export interface QuizzesSearch {
  status: "all" | LearningQuizStatus;
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

export interface LearningQuizListParams {
  page?: number;
  per_page?: number;
  status?: LearningQuizStatus;
}

export interface LearningQuizAttemptListParams {
  page?: number;
  per_page?: number;
  status?: LearningQuizAttemptStatus;
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
  submission_method: LearningSubmissionMethod;
}

export interface CreateLearningQuiz {
  position: number;
  title: string;
  instructions: string | null;
  opens_at: string | null;
  closes_at: string | null;
  attempt_limit: number;
  pass_score_basis_points: number;
}

export interface LearningQuizChoiceInput {
  label: string;
  is_correct: boolean;
}

export interface CreateLearningQuizQuestion {
  position: number;
  prompt: string;
  points: number;
  choices: LearningQuizChoiceInput[];
}

export interface LearningCompletionRequirementInput {
  requirement_type: LearningCompletionRequirementType;
  source_id: string;
  minimum_score_basis_points: number;
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

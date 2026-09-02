/** Typed HTTP client for the E-learning module's governed operations. */

import { AxiosError } from "axios";
import { httpClient } from "@/lib/http-client";
import type {
  ApiEnvelope,
  CreateLearningResource,
  CreateLearningSpace,
  CreateLearningUnit,
  GovernedFileReference,
  LearningAssignment,
  LearningAssignmentListParams,
  CreateLearningAssignment,
  CreateLearningRubricCriterion,
  CreateLearningQuiz,
  CreateLearningQuizQuestion,
  LearningAssignmentsResponse,
  LearningCompletionPage,
  LearningCompletionPolicy,
  LearningCompletionRequirementInput,
  LearningDownload,
  LearningFeedback,
  LearningFilesResponse,
  LearningProgressEntry,
  LearningProgressResponse,
  LearningProgressListParams,
  LearningQuiz,
  LearningQuizAttempt,
  LearningQuizAttemptListParams,
  LearningQuizAttemptsResponse,
  LearningQuizListParams,
  LearningQuizQuestion,
  LearningQuizzesResponse,
  LearningReferenceData,
  LearningResource,
  LearningRubricCriterion,
  LearningSettings,
  LearningSpace,
  LearningSpaceListParams,
  LearningSpacesResponse,
  LearningSubmission,
  LearningSubmissionListParams,
  LearningSubmissionsResponse,
  LearningUnit,
  LearningUploadClassificationOptions,
  LearningReviewOutcome,
  UpdateLearningFeedbackPayload,
} from "./types";

const BASE = "/api/1.0/learning";

async function request<T>(
  work: () => Promise<{ data: ApiEnvelope<T> }>,
): Promise<ApiEnvelope<T>> {
  try {
    return (await work()).data;
  } catch (error) {
    if (error instanceof AxiosError && error.response) {
      return error.response.data as ApiEnvelope<T>;
    }
    throw error;
  }
}

export const learningService = {
  settings: () => request<LearningSettings>(() => httpClient.get(`${BASE}/settings`)),
  uploadClassificationOptions: () =>
    request<LearningUploadClassificationOptions>(() =>
      httpClient.get(`${BASE}/settings/classifications`),
    ),
  updateSettings: (
    settings: LearningSettings,
    documentSeriesId: string | null,
    learnerSubmissionSeriesId: string | null,
  ) =>
    request<LearningSettings>(() =>
      httpClient.put(`${BASE}/settings`, {
        document_series_id: documentSeriesId,
        learner_submission_series_id: learnerSubmissionSeriesId,
        expected_version: settings.version,
      }),
    ),
  references: () =>
    request<LearningReferenceData>(() => httpClient.get(`${BASE}/references`)),
  resourceFiles: (search?: string) =>
    request<LearningFilesResponse>(() =>
      httpClient.get(`${BASE}/resource-files`, {
        params: { search: search || undefined, limit: 100 },
      }),
    ),
  spaces: (params?: LearningSpaceListParams) =>
    request<LearningSpacesResponse>(() => httpClient.get(`${BASE}/spaces`, { params })),
  createSpace: (payload: CreateLearningSpace) =>
    request<LearningSpace>(() => httpClient.post(`${BASE}/spaces`, payload)),
  space: (id: string) =>
    request<LearningSpace>(() => httpClient.get(`${BASE}/spaces/${id}`)),
  updateSpace: (space: LearningSpace, payload: { title: string; summary: string | null }) =>
    request<LearningSpace>(() =>
      httpClient.put(`${BASE}/spaces/${space.id}`, {
        ...payload,
        expected_version: space.version,
      }),
    ),
  publishSpace: (space: LearningSpace) =>
    request<LearningSpace>(() =>
      httpClient.post(`${BASE}/spaces/${space.id}/publish`, {
        expected_version: space.version,
      }),
    ),
  archiveSpace: (space: LearningSpace, reason: string) =>
    request<LearningSpace>(() =>
      httpClient.post(`${BASE}/spaces/${space.id}/archive`, {
        expected_version: space.version,
        reason,
      }),
    ),
  createUnit: (spaceId: string, payload: CreateLearningUnit) =>
    request<LearningUnit>(() =>
      httpClient.post(`${BASE}/spaces/${spaceId}/units`, payload),
    ),
  updateUnit: (unit: LearningUnit, payload: CreateLearningUnit) =>
    request<LearningUnit>(() =>
      httpClient.put(`${BASE}/units/${unit.id}`, {
        ...payload,
        expected_version: unit.version,
      }),
    ),
  publishUnit: (unit: LearningUnit) =>
    request<LearningUnit>(() =>
      httpClient.post(`${BASE}/units/${unit.id}/publish`, {
        expected_version: unit.version,
      }),
    ),
  withdrawUnit: (unit: LearningUnit, reason: string) =>
    request<LearningUnit>(() =>
      httpClient.post(`${BASE}/units/${unit.id}/withdraw`, {
        expected_version: unit.version,
        reason,
      }),
    ),
  createResource: (unitId: string, payload: CreateLearningResource) =>
    request<LearningResource>(() =>
      httpClient.post(`${BASE}/units/${unitId}/resources`, payload),
    ),
  uploadResource: (unitId: string, form: FormData) =>
    request<LearningResource>(() =>
      httpClient.post(`${BASE}/units/${unitId}/resources/upload`, form),
    ),
  updateResource: (
    resource: LearningResource,
    payload: { display_title: string; position: number },
  ) =>
    request<LearningResource>(() =>
      httpClient.put(`${BASE}/resources/${resource.id}`, {
        ...payload,
        expected_version: resource.version,
      }),
    ),
  publishResource: (resource: LearningResource) =>
    request<LearningResource>(() =>
      httpClient.post(`${BASE}/resources/${resource.id}/publish`, {
        expected_version: resource.version,
      }),
    ),
  withdrawResource: (resource: LearningResource, reason: string) =>
    request<LearningResource>(() =>
      httpClient.post(`${BASE}/resources/${resource.id}/withdraw`, {
        expected_version: resource.version,
        reason,
      }),
    ),
  downloadResource: (resourceId: string) =>
    request<LearningDownload>(() =>
      httpClient.get(`${BASE}/resources/${resourceId}/download`),
    ),
  assignments: (spaceId: string, params?: LearningAssignmentListParams) =>
    request<LearningAssignmentsResponse>(() =>
      httpClient.get(`${BASE}/spaces/${spaceId}/assignments`, { params }),
    ),
  assignment: (assignmentId: string) =>
    request<LearningAssignment>(() =>
      httpClient.get(`${BASE}/assignments/${assignmentId}`),
    ),
  createAssignment: (unitId: string, payload: CreateLearningAssignment) =>
    request<LearningAssignment>(() =>
      httpClient.post(`${BASE}/units/${unitId}/assignments`, payload),
    ),
  updateAssignment: (assignment: LearningAssignment, payload: CreateLearningAssignment) =>
    request<LearningAssignment>(() =>
      httpClient.put(`${BASE}/assignments/${assignment.id}`, {
        ...payload,
        expected_version: assignment.version,
      }),
    ),
  publishAssignment: (assignment: LearningAssignment) =>
    request<LearningAssignment>(() =>
      httpClient.post(`${BASE}/assignments/${assignment.id}/publish`, {
        expected_version: assignment.version,
      }),
    ),
  closeAssignment: (assignment: LearningAssignment, reason: string) =>
    request<LearningAssignment>(() =>
      httpClient.post(`${BASE}/assignments/${assignment.id}/close`, {
        expected_version: assignment.version,
        reason,
      }),
    ),
  createRubricCriterion: (
    assignmentId: string,
    payload: CreateLearningRubricCriterion,
  ) =>
    request<LearningRubricCriterion>(() =>
      httpClient.post(`${BASE}/assignments/${assignmentId}/rubric-criteria`, payload),
    ),
  updateRubricCriterion: (
    criterion: LearningRubricCriterion,
    payload: CreateLearningRubricCriterion,
  ) =>
    request<LearningRubricCriterion>(() =>
      httpClient.put(`${BASE}/rubric-criteria/${criterion.id}`, {
        ...payload,
        expected_version: criterion.version,
      }),
    ),
  deleteRubricCriterion: (criterion: LearningRubricCriterion) =>
    request<{ deleted: boolean }>(() =>
      httpClient.delete(`${BASE}/rubric-criteria/${criterion.id}`, {
        data: { expected_version: criterion.version },
      }),
    ),
  mySubmission: (assignmentId: string) =>
    request<LearningSubmission>(() =>
      httpClient.get(`${BASE}/assignments/${assignmentId}/submission`),
    ),
  saveSubmission: (assignmentId: string, body: string, expectedVersion: number | null) =>
    request<LearningSubmission>(() =>
      httpClient.put(`${BASE}/assignments/${assignmentId}/submission`, {
        body,
        expected_version: expectedVersion,
      }),
    ),
  uploadSubmissionFile: (
    assignmentId: string,
    file: File,
    expectedVersion: number | null,
  ) => {
    const form = new FormData();
    form.append("expected_submission_version", expectedVersion?.toString() ?? "");
    form.append("file", file);
    return request<LearningSubmission>(() =>
      httpClient.post(`${BASE}/assignments/${assignmentId}/submission/files`, form),
    );
  },
  removeSubmissionFile: (
    assignmentId: string,
    attachmentId: string,
    expectedSubmissionVersion: number,
    expectedAttachmentVersion: number,
  ) =>
    request<LearningSubmission>(() =>
      httpClient.delete(
        `${BASE}/assignments/${assignmentId}/submission/files/${attachmentId}`,
        {
          data: {
            expected_submission_version: expectedSubmissionVersion,
            expected_attachment_version: expectedAttachmentVersion,
          },
        },
      ),
    ),
  downloadSubmissionFile: (attachmentId: string) =>
    request<LearningDownload>(() =>
      httpClient.get(`${BASE}/submission-files/${attachmentId}/download`),
    ),
  submitSubmission: (
    assignmentId: string,
    expectedVersion: number,
    idempotencyKey: string,
  ) =>
    request<LearningSubmission>(() =>
      httpClient.post(`${BASE}/assignments/${assignmentId}/submission/submit`, {
        expected_version: expectedVersion,
        idempotency_key: idempotencyKey,
      }),
    ),
  submissions: (assignmentId: string, params?: LearningSubmissionListParams) =>
    request<LearningSubmissionsResponse>(() =>
      httpClient.get(`${BASE}/assignments/${assignmentId}/submissions`, { params }),
    ),
  submission: (submissionId: string) =>
    request<LearningSubmission>(() =>
      httpClient.get(`${BASE}/submissions/${submissionId}`),
    ),
  updateFeedback: (submissionId: string, payload: UpdateLearningFeedbackPayload) =>
    request<LearningFeedback>(() =>
      httpClient.put(`${BASE}/submissions/${submissionId}/feedback`, payload),
    ),
  releaseFeedback: (
    submissionId: string,
    outcome: LearningReviewOutcome,
    expectedReviewVersion: number,
    idempotencyKey: string,
  ) =>
    request<LearningFeedback>(() =>
      httpClient.post(`${BASE}/submissions/${submissionId}/feedback/release`, {
        outcome,
        expected_review_version: expectedReviewVersion,
        idempotency_key: idempotencyKey,
      }),
    ),
  myProgress: (spaceId: string) =>
    request<LearningProgressEntry>(() =>
      httpClient.get(`${BASE}/spaces/${spaceId}/progress/me`),
    ),
  progress: (spaceId: string, params?: LearningProgressListParams) =>
    request<LearningProgressResponse>(() =>
      httpClient.get(`${BASE}/spaces/${spaceId}/progress`, { params }),
    ),
  quizzes: (spaceId: string, params?: LearningQuizListParams) =>
    request<LearningQuizzesResponse>(() =>
      httpClient.get(`${BASE}/spaces/${spaceId}/quizzes`, { params }),
    ),
  quiz: (quizId: string) =>
    request<LearningQuiz>(() => httpClient.get(`${BASE}/quizzes/${quizId}`)),
  createQuiz: (unitId: string, payload: CreateLearningQuiz) =>
    request<LearningQuiz>(() => httpClient.post(`${BASE}/units/${unitId}/quizzes`, payload)),
  updateQuiz: (quiz: LearningQuiz, payload: CreateLearningQuiz) =>
    request<LearningQuiz>(() =>
      httpClient.put(`${BASE}/quizzes/${quiz.id}`, { ...payload, expected_version: quiz.version }),
    ),
  createQuizQuestion: (quizId: string, payload: CreateLearningQuizQuestion) =>
    request<LearningQuizQuestion>(() =>
      httpClient.post(`${BASE}/quizzes/${quizId}/questions`, payload),
    ),
  updateQuizQuestion: (question: LearningQuizQuestion, payload: CreateLearningQuizQuestion) =>
    request<LearningQuizQuestion>(() =>
      httpClient.put(`${BASE}/quiz-questions/${question.id}`, {
        ...payload,
        expected_version: question.version,
      }),
    ),
  deleteQuizQuestion: (question: LearningQuizQuestion) =>
    request<{ deleted: boolean }>(() =>
      httpClient.delete(`${BASE}/quiz-questions/${question.id}`, {
        data: { expected_version: question.version },
      }),
    ),
  publishQuiz: (quiz: LearningQuiz) =>
    request<LearningQuiz>(() =>
      httpClient.post(`${BASE}/quizzes/${quiz.id}/publish`, { expected_version: quiz.version }),
    ),
  closeQuiz: (quiz: LearningQuiz, reason: string) =>
    request<LearningQuiz>(() =>
      httpClient.post(`${BASE}/quizzes/${quiz.id}/close`, {
        expected_version: quiz.version,
        reason,
      }),
    ),
  startQuizAttempt: (quizId: string) =>
    request<LearningQuizAttempt>(() => httpClient.post(`${BASE}/quizzes/${quizId}/attempts`)),
  quizAttempts: (quizId: string, params?: LearningQuizAttemptListParams) =>
    request<LearningQuizAttemptsResponse>(() =>
      httpClient.get(`${BASE}/quizzes/${quizId}/attempts`, { params }),
    ),
  quizAttempt: (attemptId: string) =>
    request<LearningQuizAttempt>(() => httpClient.get(`${BASE}/quiz-attempts/${attemptId}`)),
  saveQuizAttempt: (
    attempt: LearningQuizAttempt,
    answers: Array<{ question_id: string; selected_choice_id: string }>,
  ) =>
    request<LearningQuizAttempt>(() =>
      httpClient.put(`${BASE}/quiz-attempts/${attempt.id}`, {
        answers,
        expected_version: attempt.version,
      }),
    ),
  submitQuizAttempt: (attempt: LearningQuizAttempt, idempotencyKey: string) =>
    request<LearningQuizAttempt>(() =>
      httpClient.post(`${BASE}/quiz-attempts/${attempt.id}/submit`, {
        expected_version: attempt.version,
        idempotency_key: idempotencyKey,
      }),
    ),
  completionPolicy: (spaceId: string) =>
    request<LearningCompletionPolicy>(() =>
      httpClient.get(`${BASE}/spaces/${spaceId}/completion-policy`),
    ),
  saveCompletionPolicy: (
    spaceId: string,
    policy: LearningCompletionPolicy | null,
    requirements: LearningCompletionRequirementInput[],
  ) =>
    request<LearningCompletionPolicy>(() =>
      httpClient.put(`${BASE}/spaces/${spaceId}/completion-policy`, {
        requirements,
        expected_version: policy?.status === "draft" ? policy.version : null,
      }),
    ),
  publishCompletionPolicy: (spaceId: string, policy: LearningCompletionPolicy) =>
    request<LearningCompletionPolicy>(() =>
      httpClient.post(`${BASE}/spaces/${spaceId}/completion-policy/publish`, {
        expected_version: policy.version,
      }),
    ),
  myCompletion: (spaceId: string) =>
    request<LearningCompletionPage>(() =>
      httpClient.get(`${BASE}/spaces/${spaceId}/completion/me`),
    ),
  completion: (spaceId: string) =>
    request<LearningCompletionPage>(() =>
      httpClient.get(`${BASE}/spaces/${spaceId}/completion`),
    ),
};

export function responseMessage(
  response: Pick<ApiEnvelope<unknown>, "issues" | "message">,
  fallback: string,
) {
  const issue = response.issues?.[0];
  if (typeof issue === "string") return issue;
  if (issue && typeof issue === "object" && issue.detail) return issue.detail;
  return response.message || fallback;
}

export function fileLabel(file: GovernedFileReference) {
  return `${file.reference} · ${file.title}`;
}

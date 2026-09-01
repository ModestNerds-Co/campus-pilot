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
  LearningAssignmentsResponse,
  LearningDownload,
  LearningFeedback,
  LearningFilesResponse,
  LearningProgressEntry,
  LearningProgressResponse,
  LearningProgressListParams,
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
  updateSettings: (settings: LearningSettings, documentSeriesId: string | null) =>
    request<LearningSettings>(() =>
      httpClient.put(`${BASE}/settings`, {
        document_series_id: documentSeriesId,
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

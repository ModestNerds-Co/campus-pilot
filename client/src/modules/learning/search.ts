import { z } from "zod";

import type {
  AssignmentDetailSearch,
  AssignmentsSearch,
  ProgressSearch,
  QuizzesSearch,
  SpacesSearch,
  SubmissionSearch,
} from "./types";

const page = z.coerce.number().int().min(1).max(1_000_000).catch(1);

const spacesSearchSchema = z.object({
  q: z.string().max(180).catch(""),
  status: z.enum(["all", "draft", "published", "archived"]).catch("all"),
  page,
});

const assignmentsSearchSchema = z.object({
  status: z.enum(["all", "draft", "published", "closed"]).catch("all"),
  page,
});

const assignmentDetailSearchSchema = z.object({
  tab: z.enum(["brief", "work", "submissions", "rubric"]).catch("brief"),
  submission_status: z
    .enum(["all", "draft", "submitted", "revision_requested", "graded"])
    .catch("all"),
  submission_page: page,
});

const submissionSearchSchema = z.object({
  version: z.union([z.literal(""), z.string().uuid()]).catch(""),
});

const progressSearchSchema = z.object({
  q: z.string().max(180).catch(""),
  page,
});

const quizzesSearchSchema = z.object({
  status: z.enum(["all", "draft", "published", "closed"]).catch("all"),
  page,
});

export function parseSpacesSearch(search: Record<string, unknown>): SpacesSearch {
  return spacesSearchSchema.parse(search);
}

export function parseAssignmentsSearch(search: Record<string, unknown>): AssignmentsSearch {
  return assignmentsSearchSchema.parse(search);
}

export function parseAssignmentDetailSearch(
  search: Record<string, unknown>,
): AssignmentDetailSearch {
  return assignmentDetailSearchSchema.parse(search);
}

export function parseSubmissionSearch(search: Record<string, unknown>): SubmissionSearch {
  return submissionSearchSchema.parse(search);
}

export function parseProgressSearch(search: Record<string, unknown>): ProgressSearch {
  return progressSearchSchema.parse(search);
}

export function parseQuizzesSearch(search: Record<string, unknown>): QuizzesSearch {
  return quizzesSearchSchema.parse(search);
}

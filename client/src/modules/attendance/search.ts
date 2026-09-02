import { z } from "zod";

import type {
  AttendanceExceptionsSearch,
  AttendanceLessonSessionsSearch,
} from "./types";

const page = z.coerce.number().int().min(1).max(1_000_000).catch(1);
const date = z.union([z.literal(""), z.string().date()]).catch("");
const resource = z.union([z.literal("all"), z.string().uuid()]).catch("all");

const lessonSessionsSearch = z.object({
  page,
  date_from: date,
  date_to: date,
  class_group_id: resource,
  status: z.enum(["all", "scheduled", "open", "completed", "cancelled"]).catch("all"),
});

const exceptionsSearch = z.object({
  page,
  date_from: date,
  date_to: date,
  class_group_id: resource,
  status: z.enum(["all", "open", "acknowledged", "resolved"]).catch("all"),
  mark: z.enum(["all", "absent", "late", "excused"]).catch("all"),
});

export function parseLessonSessionsSearch(
  search: Record<string, unknown>,
): AttendanceLessonSessionsSearch {
  return lessonSessionsSearch.parse(search);
}

export function parseExceptionsSearch(
  search: Record<string, unknown>,
): AttendanceExceptionsSearch {
  return exceptionsSearch.parse(search);
}

import { z } from "zod";

import type { MessagingListSearch } from "./types";

const messagingSearchSchema = z.object({
  q: z.string().max(180).catch(""),
  status: z.enum(["all", "draft", "submitted", "published", "cancelled"]).catch("all"),
  page: z.coerce.number().int().min(1).max(1_000_000).catch(1),
  filter: z.enum(["all", "unread"]).catch("all"),
});

export function parseMessagingSearch(search: Record<string, unknown>): MessagingListSearch {
  return messagingSearchSchema.parse(search);
}

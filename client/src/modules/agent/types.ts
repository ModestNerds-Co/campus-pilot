import type { ApiEnvelope } from "@/modules/users/types";

export type AgentApiResponse<T> = ApiEnvelope<T> & { http_status?: number };

export type SessionStatus = "active" | "archived";
export type MessageRole = "user" | "assistant";
export type RunStatus =
  | "queued"
  | "running"
  | "awaiting_approval"
  | "completed"
  | "failed"
  | "cancelled"
  | "interrupted";

export interface AgentSession {
  id: string;
  title: string;
  status: SessionStatus;
  version: number;
  last_activity_at: string;
  created_at: string;
  updated_at: string;
}

export interface AgentMessage {
  id: string;
  session_id: string;
  sequence: number;
  role: MessageRole;
  content: string;
  created_at: string;
}

export interface AgentRun {
  id: string;
  session_id: string;
  request_message_id: string;
  response_message_id?: string | null;
  task_class: string;
  origin_module_key: string;
  origin_route: string;
  status: RunStatus;
  safe_failure_code?: string | null;
  safe_failure_message?: string | null;
  started_at?: string | null;
  finished_at?: string | null;
  version: number;
  created_at: string;
  updated_at: string;
}

export interface AgentRunEvent {
  cursor: string;
  run_id: string;
  event_type: string;
  created_at: string;
}

export interface SessionCursor {
  last_activity_at: string;
  session_id: string;
}

export interface MessageCursor {
  sequence: number;
  message_id: string;
}

export interface RunCursor {
  created_at: string;
  run_id: string;
}

export interface CursorPage<T, C> {
  items: T[];
  next_cursor?: C | null;
}

export interface AgentUsageReportRow {
  event_id: string;
  event_kind: string;
  outcome: string;
  run_id: string;
  actor_user_id: string;
  origin_module_key: string;
  capability_module_key?: string | null;
  capability_key?: string | null;
  provider_key?: string | null;
  provider_model_id?: string | null;
  meter: string;
  amount?: number | null;
  enforcement_amount?: number | null;
  enforcement_basis?: string | null;
  currency_code?: string | null;
  currency_exponent?: number | null;
  pricing_version?: string | null;
  occurred_at: string;
}

export interface AgentUsageCursor {
  occurred_at: string;
  event_id: string;
  meter: string;
}

export interface AgentUsageReportPage {
  items: AgentUsageReportRow[];
  next_cursor?: AgentUsageCursor | null;
}

export interface AgentPageContext {
  moduleKey: string;
  label: string;
  route: string;
}

export interface ListSessionsInput {
  limit?: number;
  cursor?: SessionCursor;
  titleSearch?: string;
  includeArchived?: boolean;
}

export interface ListMessagesInput {
  limit?: number;
  cursor?: MessageCursor;
}

export interface ListRunsInput {
  limit?: number;
  cursor?: RunCursor;
}

export function isActiveRun(status: RunStatus) {
  return status === "queued" || status === "running" || status === "awaiting_approval";
}

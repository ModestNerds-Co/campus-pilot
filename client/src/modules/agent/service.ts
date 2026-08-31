import { AxiosError } from "axios";

import { httpClient } from "@/lib/http-client";
import type { ApiEnvelope } from "@/modules/users/types";

import type {
  AgentApiResponse,
  AgentMessage,
  AgentRun,
  AgentRunEvent,
  AgentSession,
  AgentUsageReportPage,
  AgentUsageReportRow,
  AgentUsageCursor,
  CursorPage,
  ListMessagesInput,
  ListRunsInput,
  ListSessionsInput,
  MessageCursor,
  RunCursor,
  SessionCursor,
} from "./types";

const BASE_URL = "/api/1.0/agent";

async function request<T>(work: () => Promise<{ data: ApiEnvelope<T>; status: number }>): Promise<AgentApiResponse<T>> {
  try {
    const response = await work();
    return { ...response.data, http_status: response.status };
  } catch (error) {
    if (error instanceof AxiosError && error.response) {
      return { ...(error.response.data as ApiEnvelope<T>), http_status: error.response.status };
    }
    throw error;
  }
}

export const agentService = {
  listSessions: (input: ListSessionsInput = {}) =>
    request<CursorPage<AgentSession, SessionCursor>>(() =>
      httpClient.get(`${BASE_URL}/sessions`, {
        params: {
          limit: input.limit,
          cursor_last_activity_at: input.cursor?.last_activity_at,
          cursor_session_id: input.cursor?.session_id,
          title_search: input.titleSearch || undefined,
          include_archived: input.includeArchived || undefined,
        },
      }),
    ),

  listAllSessions: (input: Omit<ListSessionsInput, "cursor"> = {}) =>
    collectCursorPages<AgentSession, SessionCursor>((cursor) =>
      agentService.listSessions({ ...input, cursor, limit: 100 }),
    ),

  createSession: (title: string, idempotencyKey: string) =>
    request<AgentSession>(() =>
      httpClient.post(
        `${BASE_URL}/sessions`,
        { title, idempotency_key: idempotencyKey },
        { headers: { "Idempotency-Key": idempotencyKey } },
      ),
    ),

  getSession: (sessionId: string) =>
    request<AgentSession>(() => httpClient.get(`${BASE_URL}/sessions/${sessionId}`)),

  renameSession: (sessionId: string, title: string, expectedVersion: number) =>
    request<AgentSession>(() =>
      httpClient.patch(`${BASE_URL}/sessions/${sessionId}`, {
        title,
        expected_version: expectedVersion,
      }),
    ),

  archiveSession: (sessionId: string, expectedVersion: number) =>
    request<AgentSession>(() =>
      httpClient.post(`${BASE_URL}/sessions/${sessionId}/archive`, {
        expected_version: expectedVersion,
      }),
    ),

  listMessages: (sessionId: string, input: ListMessagesInput = {}) =>
    request<CursorPage<AgentMessage, MessageCursor>>(() =>
      httpClient.get(`${BASE_URL}/sessions/${sessionId}/messages`, {
        params: {
          limit: input.limit,
          cursor_sequence: input.cursor?.sequence,
          cursor_message_id: input.cursor?.message_id,
        },
      }),
    ),

  listAllMessages: (sessionId: string) =>
    collectCursorPages<AgentMessage, MessageCursor>((cursor) =>
      agentService.listMessages(sessionId, { cursor, limit: 100 }),
    ),

  submitMessage: (
    sessionId: string,
    input: {
      content: string;
      originModuleKey: string;
      originRoute: string;
      idempotencyKey: string;
    },
  ) =>
    request<AgentRun>(() =>
      httpClient.post(
        `${BASE_URL}/sessions/${sessionId}/messages`,
        {
          content: input.content,
          task_class: "campus_conversation",
          origin_module_key: input.originModuleKey,
          origin_route: input.originRoute,
          idempotency_key: input.idempotencyKey,
        },
        { headers: { "Idempotency-Key": input.idempotencyKey } },
      ),
    ),

  listRuns: (sessionId: string, input: ListRunsInput = {}) =>
    request<CursorPage<AgentRun, RunCursor>>(() =>
      httpClient.get(`${BASE_URL}/sessions/${sessionId}/runs`, {
        params: {
          limit: input.limit,
          cursor_created_at: input.cursor?.created_at,
          cursor_run_id: input.cursor?.run_id,
        },
      }),
    ),

  listAllRuns: (sessionId: string) =>
    collectCursorPages<AgentRun, RunCursor>((cursor) =>
      agentService.listRuns(sessionId, { cursor, limit: 100 }),
    ),

  getRun: (runId: string) => request<AgentRun>(() => httpClient.get(`${BASE_URL}/runs/${runId}`)),

  cancelRun: (runId: string) =>
    request<AgentRun>(() => httpClient.post(`${BASE_URL}/runs/${runId}/cancel`)),

  listRunEvents: (runId: string, after = "0") =>
    request<CursorPage<AgentRunEvent, string>>(() =>
      httpClient.get(`${BASE_URL}/runs/${runId}/events`, { params: { after, limit: 100 } }),
    ),

  getPersonalUsage: (cursor?: AgentUsageCursor) =>
    request<AgentUsageReportPage>(() => httpClient.get(`${BASE_URL}/usage/personal`, {
      params: {
        limit: 100,
        cursor_occurred_at: cursor?.occurred_at,
        cursor_event_id: cursor?.event_id,
        cursor_meter: cursor?.meter,
      },
    })),

  getAllPersonalUsage: () =>
    collectCursorPages<AgentUsageReportRow, AgentUsageCursor>((cursor) => agentService.getPersonalUsage(cursor)),
};

export function agentErrorMessage(response: Pick<AgentApiResponse<unknown>, "issues" | "message">, fallback: string) {
  const issue = response.issues?.[0];
  if (typeof issue === "string") return issue;
  return issue?.detail || response.message || fallback;
}

export function isForbidden(response: Pick<AgentApiResponse<unknown>, "http_status">) {
  return response.http_status === 403;
}

export function newIdempotencyKey(prefix: string) {
  return `${prefix}-${crypto.randomUUID()}`;
}

async function collectCursorPages<T, C>(
  read: (cursor?: C) => Promise<AgentApiResponse<CursorPage<T, C>>>,
): Promise<AgentApiResponse<CursorPage<T, C>>> {
  const items: T[] = [];
  const seen = new Set<string>();
  let cursor: C | undefined;
  let lastResponse: AgentApiResponse<CursorPage<T, C>> | null = null;
  for (let page = 0; page < 100; page += 1) {
    const response = await read(cursor);
    lastResponse = response;
    if (!response.success || !response.data) return response;
    items.push(...response.data.items);
    const next = response.data.next_cursor ?? undefined;
    if (!next) return { ...response, data: { items, next_cursor: null } };
    const identity = JSON.stringify(next);
    if (seen.has(identity)) {
      return {
        ...response,
        data: null,
        issues: ["Agent returned a repeated pagination cursor."],
        message: "Agent pagination could not continue safely.",
        success: false,
      };
    }
    seen.add(identity);
    cursor = next;
  }
  return {
    ...(lastResponse as AgentApiResponse<CursorPage<T, C>>),
    data: null,
    issues: ["Agent history exceeded the supported pagination boundary."],
    message: "Agent history is too large to load safely.",
    success: false,
  };
}

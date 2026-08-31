import React, { useCallback, useEffect, useRef, useState } from "react";
import { Link } from "@tanstack/react-router";
import { Bot, Clock3, History, Loader2, MessageSquarePlus, RefreshCw, Send } from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { DialogBody, DialogFooter, DialogHeader, DialogShell } from "@/components/ui/dialog";
import { Textarea } from "@/components/ui/input";
import { useAuthStore } from "@/stores/auth-store";

import { formatAgentDate, runStatusLabel } from "./format";
import { agentErrorMessage, agentService, isForbidden, newIdempotencyKey } from "./service";
import { ContextChip, AgentSessionPanel } from "./session-panel";
import type { AgentPageContext, AgentRun, AgentSession } from "./types";
import { isActiveRun } from "./types";

type RecentState = "idle" | "loading" | "ready" | "error" | "forbidden";

export function AgentWidget({ context }: { context: AgentPageContext }) {
  const user = useAuthStore((state) => state.user);
  const permissions = user?.permissions ?? [];
  const enabled = Boolean(user?.modules?.includes("agent") && hasPermission(permissions, "agent:view"));
  const canRun = hasPermission(permissions, "agent:run");
  const canHistory = hasPermission(permissions, "agent:history");
  const [open, setOpen] = useState(false);
  const [contextSnapshot, setContextSnapshot] = useState(context);
  const [selectedSessionId, setSelectedSessionId] = useState<string | null>(null);
  const [pendingSessionId, setPendingSessionId] = useState<string | null>(null);
  const [recent, setRecent] = useState<AgentSession[]>([]);
  const [recentRuns, setRecentRuns] = useState<Record<string, AgentRun>>({});
  const [recentState, setRecentState] = useState<RecentState>("idle");
  const [recentError, setRecentError] = useState("");
  const [draft, setDraft] = useState("");
  const [sending, setSending] = useState(false);
  const [sendError, setSendError] = useState("");
  const messageKeyRef = useRef<string | null>(null);
  const sessionKeyRef = useRef<string | null>(null);

  const loadRecent = useCallback(async () => {
    if (!canHistory) {
      setRecentState("forbidden");
      return;
    }
    setRecentState("loading");
    try {
      const response = await agentService.listSessions({ limit: 5 });
      if (!response.success || !response.data) {
        if (isForbidden(response)) setRecentState("forbidden");
        else {
          setRecentError(agentErrorMessage(response, "Recent Sessions could not be loaded."));
          setRecentState("error");
        }
        return;
      }
      const sessions = response.data.items;
      setRecent(sessions);
      const runResponses = await Promise.all(
        sessions.map(async (session) => ({ sessionId: session.id, response: await agentService.listRuns(session.id, { limit: 1 }) })),
      );
      setRecentRuns(Object.fromEntries(runResponses.flatMap(({ response, sessionId }) => response.success && response.data?.items[0] ? [[sessionId, response.data.items[0]]] : [])));
      setRecentError("");
      setRecentState("ready");
    } catch {
      setRecentError("Agent could not be reached. Check your connection and try again.");
      setRecentState("error");
    }
  }, [canHistory]);

  useEffect(() => {
    if (open && !selectedSessionId) void loadRecent();
  }, [loadRecent, open, selectedSessionId]);

  useEffect(() => {
    if (!open) return;
    const closeOnlyAgentDrawer = (event: KeyboardEvent) => {
      if (event.key !== "Escape" || document.querySelectorAll('[role="dialog"]').length !== 1) return;
      event.preventDefault();
      event.stopImmediatePropagation();
      setOpen(false);
    };
    window.addEventListener("keydown", closeOnlyAgentDrawer, true);
    return () => window.removeEventListener("keydown", closeOnlyAgentDrawer, true);
  }, [open]);

  useEffect(() => {
    if (enabled) return;
    setOpen(false);
    setSelectedSessionId(null);
    setPendingSessionId(null);
    setDraft("");
    messageKeyRef.current = null;
    sessionKeyRef.current = null;
  }, [enabled, user?.id]);

  if (!enabled) return null;

  const show = () => {
    if (!selectedSessionId && !pendingSessionId) setContextSnapshot(context);
    setOpen(true);
  };

  const startNew = () => {
    setSelectedSessionId(null);
    setPendingSessionId(null);
    setDraft("");
    setSendError("");
    setContextSnapshot(context);
    messageKeyRef.current = null;
    sessionKeyRef.current = null;
  };

  const submitNew = async (event: React.FormEvent) => {
    event.preventDefault();
    const content = draft.trim();
    if (!content || !canRun || sending) return;
    setSending(true);
    setSendError("");
    try {
      let sessionId = pendingSessionId;
      if (!sessionId) {
        const title = content.length > 72 ? `${content.slice(0, 69).trimEnd()}…` : content;
        const sessionKey = sessionKeyRef.current || newIdempotencyKey("agent-session");
        sessionKeyRef.current = sessionKey;
        const createResponse = await agentService.createSession(title, sessionKey);
        if (!createResponse.success || !createResponse.data) {
          setSendError(agentErrorMessage(createResponse, "A new Session could not be created."));
          return;
        }
        sessionId = createResponse.data.id;
        sessionKeyRef.current = null;
        setPendingSessionId(sessionId);
      }
      const idempotencyKey = messageKeyRef.current || newIdempotencyKey("agent-message");
      messageKeyRef.current = idempotencyKey;
      const response = await agentService.submitMessage(sessionId, {
        content,
        idempotencyKey,
        originModuleKey: contextSnapshot.moduleKey,
        originRoute: contextSnapshot.route,
      });
      if (!response.success) {
        setSendError(agentErrorMessage(response, "Your message was not sent. Try again."));
        return;
      }
      setDraft("");
      setPendingSessionId(null);
      setSelectedSessionId(sessionId);
      messageKeyRef.current = null;
    } catch {
      setSendError("Agent could not be reached. Your message is still here so you can try again.");
    } finally {
      setSending(false);
    }
  };

  return (
    <>
      <button
        aria-haspopup="dialog"
        className="mb-2 flex min-h-10 w-full items-center gap-3 rounded-[8px] border border-[var(--sidebar-border)] bg-white/5 px-3 text-left text-[13px] font-medium text-[var(--sidebar-foreground)] hover:bg-[var(--sidebar-hover)] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--brand-highlight)]"
        onClick={show}
        type="button"
      >
        <Bot className="size-[17px] text-[var(--brand-highlight)]" />
        <span className="min-w-0 flex-1 truncate">Agent</span>
        <span className="text-[10px] uppercase tracking-[0.12em] text-[var(--sidebar-muted)]">Open</span>
      </button>

      <DialogShell onClose={() => setOpen(false)} open={open} panelClassName="sm:max-w-[560px]">
        <DialogHeader onClose={() => setOpen(false)} title="Agent" />
        <div className="flex shrink-0 items-center justify-between gap-3 border-b border-[var(--border)] bg-[var(--surface-muted)] px-4 py-2 sm:px-5">
          <Button onClick={startNew} size="sm" variant="ghost"><MessageSquarePlus className="size-3.5" />New Session</Button>
          <Link className="inline-flex min-h-9 items-center gap-1.5 rounded-[var(--radius-md)] px-3 text-xs font-medium text-[var(--text-link)] hover:bg-[var(--surface)] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--focus-ring)]" onClick={() => setOpen(false)} to="/modules/agent"><History className="size-3.5" />Full workspace</Link>
        </div>

        {selectedSessionId ? (
          <div className="min-h-0 min-w-0 flex-1">
            <AgentSessionPanel compact context={contextSnapshot} onContextResolved={setContextSnapshot} sessionId={selectedSessionId} />
          </div>
        ) : (
          <>
            <DialogBody className="space-y-5">
              <section aria-labelledby="agent-context-heading">
                <p className="text-[10px] font-semibold uppercase tracking-[0.16em] text-[var(--text-subtle)]" id="agent-context-heading">Attached context</p>
                <div className="mt-2"><ContextChip context={contextSnapshot} /></div>
              </section>

              {pendingSessionId ? (
                <div className="rounded-[var(--radius-md)] border border-[var(--tone-warn-bd)] bg-[var(--tone-warn-bg)] p-3 text-sm text-[var(--tone-warn-strong)]">The Session was created. Send again to retry this message.</div>
              ) : null}

              <section aria-labelledby="recent-sessions-heading">
                <div className="flex items-center justify-between gap-3"><h2 className="text-sm font-semibold text-[var(--text-strong)]" id="recent-sessions-heading">Recent Sessions</h2>{recentState === "loading" ? <Loader2 aria-label="Loading recent Sessions" className="size-4 animate-spin text-[var(--brand)]" /> : null}</div>
                {recentState === "loading" || recentState === "idle" ? <RecentSkeleton /> : null}
                {recentState === "forbidden" ? <WidgetState title="History is not available" description="You can start a Session if your role allows Agent runs." /> : null}
                {recentState === "error" ? <WidgetState action={<Button onClick={() => void loadRecent()} size="sm" variant="secondary"><RefreshCw className="size-3.5" />Try again</Button>} title="Recent Sessions could not be loaded" description={recentError} /> : null}
                {recentState === "ready" && recent.length === 0 ? <WidgetState title="No Sessions yet" description="Start with the message below." /> : null}
                {recentState === "ready" && recent.length > 0 ? (
                  <div className="mt-3 space-y-2">
                    {recent.map((session) => (
                      <button className="flex min-h-14 w-full min-w-0 items-center gap-3 rounded-[var(--radius-md)] border border-[var(--border)] bg-[var(--surface)] px-3 py-2 text-left hover:bg-[var(--surface-muted)] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--focus-ring)]" key={session.id} onClick={() => { setPendingSessionId(null); setSelectedSessionId(session.id); }} type="button">
                        <span className="flex size-8 shrink-0 items-center justify-center rounded-[var(--radius-md)] bg-[var(--brand-soft)] text-[var(--brand-strong)]"><Bot className="size-4" /></span>
                        <span className="min-w-0 flex-1"><span className="block truncate text-sm font-medium text-[var(--text-strong)]">{session.title}</span><span className="mt-0.5 flex items-center gap-1 text-[11px] text-[var(--text-subtle)]"><Clock3 className="size-3" />{formatAgentDate(session.last_activity_at)}</span></span>
                        {recentRuns[session.id] && isActiveRun(recentRuns[session.id].status) ? <Badge dot tone="info">{runStatusLabel(recentRuns[session.id].status)}</Badge> : session.status === "archived" ? <Badge tone="neutral">Archived</Badge> : null}
                      </button>
                    ))}
                  </div>
                ) : null}
              </section>
            </DialogBody>
            <div className="shrink-0">
              <form onSubmit={submitNew}>
                <DialogFooter className="block">
                {sendError ? <p className="mb-2 text-xs leading-5 text-[var(--tone-danger-strong)]" role="alert">{sendError}</p> : null}
                {!canRun ? <p className="mb-2 text-xs text-[var(--text-muted)]">Your current access is view-only.</p> : null}
                <div className="flex min-w-0 items-end gap-2">
                  <Textarea
                    aria-label="Start a Session"
                    className="max-h-40 min-h-11 resize-y bg-[var(--surface)]"
                    data-autofocus="true"
                    disabled={!canRun}
                    maxLength={20000}
                    onChange={(event) => { setDraft(event.target.value); setSendError(""); messageKeyRef.current = null; }}
                    onKeyDown={(event) => { if (event.key === "Enter" && !event.shiftKey) { event.preventDefault(); event.currentTarget.form?.requestSubmit(); } }}
                    placeholder="Message Agent"
                    rows={1}
                    value={draft}
                  />
                  <Button aria-label="Send message" disabled={!draft.trim() || !canRun || sending} size="icon-lg" type="submit">{sending ? <Loader2 className="size-4 animate-spin" /> : <Send className="size-4" />}</Button>
                </div>
                </DialogFooter>
              </form>
            </div>
          </>
        )}
      </DialogShell>
    </>
  );
}

function RecentSkeleton() {
  return <div className="mt-3 space-y-2" role="status">{Array.from({ length: 3 }).map((_, index) => <div className="flex items-center gap-3 rounded-[var(--radius-md)] border border-[var(--border)] p-3" key={index}><div className="size-8 animate-pulse rounded-[var(--radius-md)] bg-[var(--surface-sunken)]" /><div className="flex-1"><div className="h-3 animate-pulse rounded-full bg-[var(--surface-sunken)]" /><div className="mt-2 h-2 w-1/2 animate-pulse rounded-full bg-[var(--surface-muted)]" /></div></div>)}</div>;
}

function WidgetState({ action, description, title }: { action?: React.ReactNode; description: string; title: string }) {
  return <div className="mt-3 rounded-[var(--radius-md)] border border-[var(--border)] bg-[var(--surface-muted)] p-4"><p className="text-sm font-semibold text-[var(--text-strong)]">{title}</p><p className="mt-1 text-xs leading-5 text-[var(--text-muted)]">{description}</p>{action ? <div className="mt-3">{action}</div> : null}</div>;
}

function hasPermission(permissions: string[], permission: string) {
  return permissions.includes("*") || permissions.includes(permission);
}

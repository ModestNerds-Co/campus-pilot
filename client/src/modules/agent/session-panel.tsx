import React, { useCallback, useEffect, useRef, useState } from "react";
import {
  AlertTriangle,
  Archive,
  Bot,
  Check,
  CircleStop,
  Loader2,
  Pencil,
  RefreshCw,
  Send,
  UserRound,
  X,
} from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { DialogBody, DialogFooter, DialogHeader, DialogShell } from "@/components/ui/dialog";
import { Input, Textarea } from "@/components/ui/input";
import { useAuthStore } from "@/stores/auth-store";

import { formatAgentDate, moduleContextLabel, runStatusLabel } from "./format";
import { agentErrorMessage, agentService, isForbidden, newIdempotencyKey } from "./service";
import type { AgentMessage, AgentPageContext, AgentRun, AgentSession } from "./types";
import { isActiveRun } from "./types";

type LoadState = "loading" | "ready" | "error" | "forbidden" | "not_found";

export function AgentSessionPanel({
  compact = false,
  context,
  onArchived,
  onContextResolved,
  sessionId,
}: {
  compact?: boolean;
  context: AgentPageContext;
  onArchived?: () => void;
  onContextResolved?: (context: AgentPageContext) => void;
  sessionId: string;
}) {
  const permissions = useAuthStore((state) => state.user?.permissions ?? []);
  const canRun = hasPermission(permissions, "agent:run");
  const canManageHistory = hasPermission(permissions, "agent:history");
  const [session, setSession] = useState<AgentSession | null>(null);
  const [messages, setMessages] = useState<AgentMessage[]>([]);
  const [runs, setRuns] = useState<AgentRun[]>([]);
  const [state, setState] = useState<LoadState>("loading");
  const [error, setError] = useState("");
  const [refreshError, setRefreshError] = useState("");
  const [draft, setDraft] = useState("");
  const [sending, setSending] = useState(false);
  const [sendError, setSendError] = useState("");
  const [renaming, setRenaming] = useState(false);
  const [renameValue, setRenameValue] = useState("");
  const [renamePending, setRenamePending] = useState(false);
  const [renameError, setRenameError] = useState("");
  const [archiveOpen, setArchiveOpen] = useState(false);
  const [archivePending, setArchivePending] = useState(false);
  const [cancelRun, setCancelRun] = useState<AgentRun | null>(null);
  const [cancelPending, setCancelPending] = useState(false);
  const [actionError, setActionError] = useState("");
  const idempotencyKeyRef = useRef<string | null>(null);
  const requestVersionRef = useRef(0);
  const transcriptEndRef = useRef<HTMLDivElement>(null);
  const renamingRef = useRef(false);

  useEffect(() => {
    renamingRef.current = renaming;
  }, [renaming]);

  const load = useCallback(async (quiet = false) => {
    const requestVersion = ++requestVersionRef.current;
    if (!quiet) setState("loading");
    try {
      const [sessionResponse, messageResponse, runResponse] = await Promise.all([
        agentService.getSession(sessionId),
        agentService.listAllMessages(sessionId),
        agentService.listAllRuns(sessionId),
      ]);
      if (requestVersion !== requestVersionRef.current) return;
      const failedResponse = [sessionResponse, messageResponse, runResponse].find((response) => !response.success);
      if (failedResponse) {
        if (quiet) {
          setRefreshError(agentErrorMessage(failedResponse, "Session updates could not be loaded."));
          return;
        }
        if (isForbidden(failedResponse)) setState("forbidden");
        else if (failedResponse.http_status === 404) setState("not_found");
        else {
          setError(agentErrorMessage(failedResponse, "This Session could not be loaded."));
          setState("error");
        }
        return;
      }
      if (!sessionResponse.data || !messageResponse.data || !runResponse.data) {
        setError("The Agent service returned an incomplete Session.");
        setState("error");
        return;
      }
      const nextRuns = [...runResponse.data.items].sort(
        (left, right) => new Date(right.created_at).getTime() - new Date(left.created_at).getTime(),
      );
      setSession(sessionResponse.data);
      setMessages([...messageResponse.data.items].sort((left, right) => left.sequence - right.sequence));
      setRuns(nextRuns);
      if (!renamingRef.current) setRenameValue(sessionResponse.data.title);
      setError("");
      setRefreshError("");
      setState("ready");
      const runContext = nextRuns[0];
      if (runContext) {
        onContextResolved?.({
          moduleKey: runContext.origin_module_key,
          label: moduleContextLabel(runContext.origin_module_key),
          route: runContext.origin_route,
        });
      }
    } catch {
      if (requestVersion !== requestVersionRef.current) return;
      if (quiet) {
        setRefreshError("Session updates could not be loaded. Agent will try again.");
        return;
      }
      setError("Campus Pilot could not reach Agent. Check your connection and try again.");
      setState("error");
    }
  }, [onContextResolved, sessionId]);

  useEffect(() => {
    setDraft("");
    setSendError("");
    setRenaming(false);
    idempotencyKeyRef.current = null;
    void load();
    return () => {
      requestVersionRef.current += 1;
    };
  }, [load]);

  const latestRun = runs[0] ?? null;
  const activeRun = runs.find((run) => isActiveRun(run.status)) ?? null;

  useEffect(() => {
    if (!activeRun) return;
    const poll = window.setInterval(() => void load(true), 3000);
    return () => window.clearInterval(poll);
  }, [activeRun?.id, load]);

  useEffect(() => {
    if (state !== "ready") return;
    transcriptEndRef.current?.scrollIntoView({ block: "end" });
  }, [messages.length, state]);

  const submit = async (event: React.FormEvent) => {
    event.preventDefault();
    const content = draft.trim();
    if (!content || !session || sending || !canRun || activeRun) return;
    const idempotencyKey = idempotencyKeyRef.current || newIdempotencyKey("agent-message");
    idempotencyKeyRef.current = idempotencyKey;
    setSending(true);
    setSendError("");
    try {
      const response = await agentService.submitMessage(session.id, {
        content,
        originModuleKey: context.moduleKey,
        originRoute: context.route,
        idempotencyKey,
      });
      if (!response.success) {
        setSendError(agentErrorMessage(response, "Your message was not sent. Try again."));
        return;
      }
      setDraft("");
      idempotencyKeyRef.current = null;
      await load(true);
    } catch {
      setSendError("Agent could not be reached. Your message is still here so you can try again.");
    } finally {
      setSending(false);
    }
  };

  const rename = async (event: React.FormEvent) => {
    event.preventDefault();
    const title = renameValue.trim();
    if (!session || !title || renamePending) return;
    setRenamePending(true);
    setRenameError("");
    try {
      const response = await agentService.renameSession(session.id, title, session.version);
      if (!response.success) {
        setRenameError(agentErrorMessage(response, "The Session was not renamed."));
        return;
      }
      const truth = await agentService.getSession(session.id);
      if (truth.success && truth.data) setSession(truth.data);
      else if (response.data) setSession(response.data);
      setRenaming(false);
    } catch {
      setRenameError("Agent could not be reached. Try renaming the Session again.");
    } finally {
      setRenamePending(false);
    }
  };

  const archive = async () => {
    if (!session || archivePending) return;
    setArchivePending(true);
    setActionError("");
    try {
      const response = await agentService.archiveSession(session.id, session.version);
      if (!response.success) {
        const truth = await agentService.getSession(session.id);
        if (truth.success && truth.data?.status === "archived") {
          setSession(truth.data);
          setArchiveOpen(false);
          onArchived?.();
          return;
        }
        setActionError(agentErrorMessage(response, "The Session was not archived."));
        return;
      }
      const truth = await agentService.getSession(session.id);
      setSession(truth.success && truth.data ? truth.data : response.data ?? session);
      setArchiveOpen(false);
      onArchived?.();
    } catch {
      const truth = await agentService.getSession(session.id).catch(() => null);
      if (truth?.success && truth.data?.status === "archived") {
        setSession(truth.data);
        setArchiveOpen(false);
        onArchived?.();
        return;
      }
      setActionError("Agent could not be reached. Try archiving the Session again.");
    } finally {
      setArchivePending(false);
    }
  };

  const cancel = async () => {
    if (!cancelRun || cancelPending) return;
    setCancelPending(true);
    setActionError("");
    try {
      const response = await agentService.cancelRun(cancelRun.id);
      if (!response.success) {
        const truth = await agentService.getRun(cancelRun.id);
        if (truth.success && truth.data && !isActiveRun(truth.data.status)) {
          setCancelRun(null);
          await load(true);
          return;
        }
        setActionError(agentErrorMessage(response, "The run was not cancelled."));
        return;
      }
      await agentService.getRun(cancelRun.id);
      setCancelRun(null);
      await load(true);
    } catch {
      const truth = await agentService.getRun(cancelRun.id).catch(() => null);
      if (truth?.success && truth.data && !isActiveRun(truth.data.status)) {
        setCancelRun(null);
        await load(true);
        return;
      }
      setActionError("Agent could not be reached. Try cancelling the run again.");
    } finally {
      setCancelPending(false);
    }
  };

  if (state === "loading") return <AgentSessionLoading compact={compact} />;
  if (state === "forbidden") {
    return <AgentSessionState icon={<AlertTriangle />} title="Session access is not available" description="Your current access does not allow this Session." />;
  }
  if (state === "not_found") {
    return <AgentSessionState icon={<AlertTriangle />} title="Session not found" description="It may have been removed or is not available to your account." />;
  }
  if (state === "error") {
    return <AgentSessionState action={<Button onClick={() => void load()} variant="secondary"><RefreshCw className="size-4" />Try again</Button>} icon={<AlertTriangle />} title="Session could not be loaded" description={error} />;
  }
  if (!session) return null;

  return (
    <section className={`flex min-h-0 min-w-0 flex-col overflow-hidden bg-[var(--surface)] ${compact ? "h-full" : "min-h-[640px] rounded-[var(--radius-xl)] border border-[var(--border)] shadow-[var(--shadow-card)]"}`}>
      <header className="shrink-0 border-b border-[var(--border)] px-4 py-4 sm:px-6">
        <div className="flex min-w-0 items-start justify-between gap-3">
          <div className="min-w-0 flex-1">
            {renaming && !compact ? (
              <form className="flex min-w-0 items-center gap-2" onSubmit={rename}>
                <Input aria-label="Session title" autoFocus maxLength={120} onChange={(event) => setRenameValue(event.target.value)} value={renameValue} />
                <Button aria-label="Save Session title" disabled={renamePending || !renameValue.trim()} size="icon" type="submit"><Check className="size-4" /></Button>
                <Button aria-label="Cancel rename" disabled={renamePending} onClick={() => { setRenaming(false); setRenameValue(session.title); setRenameError(""); }} size="icon" type="button" variant="ghost"><X className="size-4" /></Button>
              </form>
            ) : (
              <div className="flex min-w-0 items-center gap-2">
                <h2 className="truncate text-base font-semibold text-[var(--text-strong)]">{session.title}</h2>
                {!compact && canManageHistory && session.status === "active" ? (
                  <Button aria-label="Rename Session" onClick={() => setRenaming(true)} size="icon-sm" variant="ghost"><Pencil className="size-3.5" /></Button>
                ) : null}
              </div>
            )}
            <div className="mt-2 flex flex-wrap items-center gap-2">
              <ContextChip context={context} />
              {latestRun ? <RunStatusBadge run={latestRun} /> : null}
              {session.status === "archived" ? <Badge tone="neutral">Archived</Badge> : null}
            </div>
            {renameError ? <p className="mt-2 text-xs text-[var(--tone-danger-strong)]" role="alert">{renameError}</p> : null}
          </div>
          {!compact && canManageHistory && session.status === "active" ? (
            <Button aria-label="Archive Session" onClick={() => setArchiveOpen(true)} size="icon" variant="ghost"><Archive className="size-4" /></Button>
          ) : null}
        </div>
        {latestRun?.safe_failure_message ? (
          <p className="mt-3 rounded-[var(--radius-md)] border border-[var(--tone-danger-bd)] bg-[var(--tone-danger-bg)] px-3 py-2 text-sm text-[var(--tone-danger-strong)]" role="alert">{latestRun.safe_failure_message}</p>
        ) : null}
        {refreshError ? <p className="mt-3 rounded-[var(--radius-md)] border border-[var(--tone-warn-bd)] bg-[var(--tone-warn-bg)] px-3 py-2 text-xs text-[var(--tone-warn-strong)]" role="status">{refreshError}</p> : null}
        {activeRun && canRun ? (
          <div className="mt-3 flex items-center justify-between gap-3 rounded-[var(--radius-md)] bg-[var(--surface-muted)] px-3 py-2 text-xs text-[var(--text-muted)]">
            <span>{activeRun.status === "awaiting_approval" ? "This run is waiting for approval." : "Agent is working on this Session."}</span>
            {!compact ? <Button onClick={() => setCancelRun(activeRun)} size="sm" variant="secondary"><CircleStop className="size-3.5" />Cancel</Button> : null}
          </div>
        ) : null}
      </header>

      <div aria-label="Session messages" aria-live="polite" className="min-h-0 flex-1 overflow-y-auto overscroll-contain px-4 py-5 sm:px-6">
        {messages.length === 0 ? (
          <div className="flex min-h-64 flex-col items-center justify-center text-center">
            <span className="flex size-11 items-center justify-center rounded-[var(--radius-lg)] bg-[var(--brand-soft)] text-[var(--brand-strong)]"><Bot className="size-5" /></span>
            <h3 className="mt-4 text-sm font-semibold text-[var(--text-strong)]">Start this Session</h3>
            <p className="mt-1 max-w-sm text-sm leading-6 text-[var(--text-muted)]">Ask for campus information or help with an available task.</p>
          </div>
        ) : (
          <div className="space-y-5">
            {messages.map((message) => <AgentMessageItem key={message.id} message={message} />)}
          </div>
        )}
        <div ref={transcriptEndRef} />
      </div>

      <form className="shrink-0 border-t border-[var(--border)] bg-[var(--surface)] p-3 sm:p-4" onSubmit={submit}>
        {sendError ? <p className="mb-2 text-xs leading-5 text-[var(--tone-danger-strong)]" role="alert">{sendError}</p> : null}
        {!canRun ? <p className="mb-2 text-xs text-[var(--text-muted)]">Your current access is view-only.</p> : null}
        {session.status === "archived" ? <p className="mb-2 text-xs text-[var(--text-muted)]">Archived Sessions are read-only.</p> : null}
        <div className="flex min-w-0 items-end gap-2">
          <Textarea
            aria-label="Message Agent"
            className="max-h-40 min-h-11 resize-y"
            disabled={!canRun || session.status === "archived"}
            maxLength={20000}
            onChange={(event) => { setDraft(event.target.value); setSendError(""); idempotencyKeyRef.current = null; }}
            onKeyDown={(event) => {
              if (event.key === "Enter" && !event.shiftKey) {
                event.preventDefault();
                event.currentTarget.form?.requestSubmit();
              }
            }}
            placeholder={activeRun ? "Wait for the current run to finish" : "Message Agent"}
            rows={1}
            value={draft}
          />
          <Button aria-label="Send message" disabled={!draft.trim() || sending || !canRun || session.status === "archived" || Boolean(activeRun)} size="icon-lg" type="submit">
            {sending ? <Loader2 className="size-4 animate-spin" /> : <Send className="size-4" />}
          </Button>
        </div>
      </form>

      {!compact && runs.length > 0 ? <RunHistory runs={runs} /> : null}

      <AgentConfirmDrawer
        confirmLabel="Archive Session"
        description="This Session will leave your active history. Its messages and run history remain available when archived Sessions are included."
        dismissLabel="Keep Session"
        error={actionError}
        onClose={() => { setArchiveOpen(false); setActionError(""); }}
        onConfirm={() => void archive()}
        open={archiveOpen}
        pending={archivePending}
        title="Archive this Session?"
      />
      <AgentConfirmDrawer
        confirmLabel="Cancel run"
        description="Agent will stop this run at the next safe checkpoint. Messages already saved remain in this Session."
        dismissLabel="Keep running"
        error={actionError}
        onClose={() => { setCancelRun(null); setActionError(""); }}
        onConfirm={() => void cancel()}
        open={cancelRun !== null}
        pending={cancelPending}
        title="Cancel the current run?"
      />
    </section>
  );
}

export function ContextChip({ context }: { context: AgentPageContext }) {
  return (
    <span className="inline-flex max-w-full items-center gap-1.5 rounded-full border border-[var(--border)] bg-[var(--surface-muted)] px-2.5 py-1 text-[11px] font-medium text-[var(--text-muted)]" title={context.route}>
      <span aria-hidden="true" className="size-1.5 shrink-0 rounded-full bg-[var(--brand)]" />
      <span className="truncate">Context: {context.label}</span>
    </span>
  );
}

function AgentMessageItem({ message }: { message: AgentMessage }) {
  const isUser = message.role === "user";
  const Icon = isUser ? UserRound : Bot;
  return (
    <article className={`flex min-w-0 gap-3 ${isUser ? "sm:pl-12" : "sm:pr-12"}`}>
      <span className={`mt-0.5 flex size-8 shrink-0 items-center justify-center rounded-full ${isUser ? "bg-[var(--surface-sunken)] text-[var(--text-muted)]" : "bg-[var(--brand-soft)] text-[var(--brand-strong)]"}`}><Icon className="size-4" /></span>
      <div className="min-w-0 flex-1">
        <div className="flex items-center justify-between gap-3">
          <p className="text-xs font-semibold text-[var(--text-strong)]">{isUser ? "You" : "Agent"}</p>
          <time className="shrink-0 text-[10px] text-[var(--text-subtle)]" dateTime={message.created_at}>{formatAgentDate(message.created_at)}</time>
        </div>
        <p className="mt-1 whitespace-pre-wrap break-words text-sm leading-6 text-[var(--text-body)]">{message.content}</p>
      </div>
    </article>
  );
}

function RunStatusBadge({ run }: { run: AgentRun }) {
  const tone = run.status === "completed" ? "success" : run.status === "failed" || run.status === "interrupted" ? "danger" : run.status === "cancelled" ? "neutral" : "info";
  return <Badge dot tone={tone}>{runStatusLabel(run.status)}</Badge>;
}

function RunHistory({ runs }: { runs: AgentRun[] }) {
  return (
    <details className="shrink-0 border-t border-[var(--border)] bg-[var(--surface-muted)] px-4 py-3 sm:px-6">
      <summary className="cursor-pointer text-xs font-semibold text-[var(--text-strong)] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--focus-ring)]">Run history ({runs.length})</summary>
      <div className="mt-3 grid gap-2 sm:grid-cols-2">
        {runs.map((run) => (
          <div className="rounded-[var(--radius-md)] border border-[var(--border)] bg-[var(--surface)] p-3" key={run.id}>
            <div className="flex items-center justify-between gap-2"><RunStatusBadge run={run} /><time className="text-[10px] text-[var(--text-subtle)]" dateTime={run.created_at}>{formatAgentDate(run.created_at)}</time></div>
            <p className="mt-2 truncate text-xs text-[var(--text-muted)]">{moduleContextLabel(run.origin_module_key)} · {run.task_class.replace(/_/g, " ")}</p>
          </div>
        ))}
      </div>
    </details>
  );
}

function AgentConfirmDrawer({ confirmLabel, description, dismissLabel, error, onClose, onConfirm, open, pending, title }: { confirmLabel: string; description: string; dismissLabel: string; error: string; onClose: () => void; onConfirm: () => void; open: boolean; pending: boolean; title: string }) {
  return (
    <DialogShell onClose={pending ? () => undefined : onClose} open={open}>
      <DialogHeader onClose={pending ? undefined : onClose} title={title} />
      <DialogBody>
        <div className="flex gap-4">
          <span className="flex size-10 shrink-0 items-center justify-center rounded-[var(--radius-md)] bg-[var(--tone-warn-bg)] text-[var(--tone-warn-strong)]"><AlertTriangle className="size-5" /></span>
          <div><p className="max-w-lg text-sm leading-6 text-[var(--text-muted)]">{description}</p>{error ? <p className="mt-3 text-sm text-[var(--tone-danger-strong)]" role="alert">{error}</p> : null}</div>
        </div>
      </DialogBody>
      <DialogFooter>
        <Button data-autofocus="true" disabled={pending} onClick={onClose} type="button" variant="secondary">{dismissLabel}</Button>
        <Button disabled={pending} onClick={onConfirm} type="button" variant="destructive">{pending ? <Loader2 className="size-4 animate-spin" /> : null}{pending ? "Working…" : confirmLabel}</Button>
      </DialogFooter>
    </DialogShell>
  );
}

function AgentSessionLoading({ compact }: { compact: boolean }) {
  return <div aria-busy="true" className={`flex items-center justify-center bg-[var(--surface)] ${compact ? "h-full min-h-72" : "min-h-[640px] rounded-[var(--radius-xl)] border border-[var(--border)]"}`} role="status"><div className="text-center"><Loader2 className="mx-auto size-6 animate-spin text-[var(--brand)]" /><p className="mt-3 text-sm text-[var(--text-muted)]">Loading Session…</p></div></div>;
}

function AgentSessionState({ action, description, icon, title }: { action?: React.ReactNode; description: string; icon: React.ReactNode; title: string }) {
  return <div className="flex min-h-72 flex-col items-center justify-center rounded-[var(--radius-xl)] border border-[var(--border)] bg-[var(--surface)] p-6 text-center"><span className="flex size-11 items-center justify-center rounded-[var(--radius-lg)] bg-[var(--surface-muted)] text-[var(--text-muted)] [&_svg]:size-5">{icon}</span><h2 className="mt-4 text-base font-semibold text-[var(--text-strong)]">{title}</h2><p className="mt-2 max-w-md text-sm leading-6 text-[var(--text-muted)]">{description}</p>{action ? <div className="mt-4">{action}</div> : null}</div>;
}

function hasPermission(permissions: string[], permission: string) {
  return permissions.includes("*") || permissions.includes(permission);
}

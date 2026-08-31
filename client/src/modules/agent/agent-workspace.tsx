import React, { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Link, useNavigate } from "@tanstack/react-router";
import {
  Archive,
  Bot,
  BarChart3,
  Clock3,
  History,
  Loader2,
  MessageSquareText,
  Plus,
  RefreshCw,
  Search,
} from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Table,
  TableEmpty,
  TableError,
  TableLoading,
  TableScroll,
  TableWrap,
  TBody,
  TD,
  TH,
  THead,
  TR,
} from "@/components/ui/data-table";
import { Input } from "@/components/ui/input";
import { usePageChrome } from "@/modules/admin/layouts/page-chrome";
import { useAuthStore } from "@/stores/auth-store";

import { formatAgentDate, moduleContextLabel } from "./format";
import { agentErrorMessage, agentService, isForbidden, newIdempotencyKey } from "./service";
import { AgentSessionPanel } from "./session-panel";
import type { AgentSession, AgentUsageReportRow } from "./types";

type HistoryState = "loading" | "ready" | "error" | "forbidden";

export function AgentWorkspace({
  selectedSessionId,
  view = "sessions",
}: {
  selectedSessionId?: string;
  view?: "sessions" | "usage";
}) {
  const navigate = useNavigate();
  const permissions = useAuthStore((state) => state.user?.permissions ?? []);
  const canRun = hasPermission(permissions, "agent:run");
  const canHistory = hasPermission(permissions, "agent:history");
  const [sessions, setSessions] = useState<AgentSession[]>([]);
  const [historyState, setHistoryState] = useState<HistoryState>(canHistory ? "loading" : "forbidden");
  const [historyError, setHistoryError] = useState("");
  const [search, setSearch] = useState("");
  const [includeArchived, setIncludeArchived] = useState(false);
  const [creating, setCreating] = useState(false);
  const [createError, setCreateError] = useState("");
  const [sessionContext, setSessionContext] = useState({ label: "Agent", moduleKey: "agent", route: "/modules/agent" });
  const historyRequestRef = useRef(0);
  const createKeyRef = useRef<string | null>(null);

  const createSession = useCallback(async () => {
    if (!canRun || creating) return;
    setCreating(true);
    setCreateError("");
    try {
      const idempotencyKey = createKeyRef.current || newIdempotencyKey("agent-session");
      createKeyRef.current = idempotencyKey;
      const response = await agentService.createSession("New session", idempotencyKey);
      if (!response.success || !response.data) {
        setCreateError(agentErrorMessage(response, "A new Session could not be created."));
        return;
      }
      createKeyRef.current = null;
      await navigate({
        params: { sessionId: response.data.id },
        to: "/modules/agent/sessions/$sessionId",
      });
    } catch {
      setCreateError("Agent could not be reached. Try creating the Session again.");
    } finally {
      setCreating(false);
    }
  }, [canRun, creating, navigate]);

  const pageAction = useMemo(
    () => canRun && view === "sessions" ? <Button disabled={creating} onClick={() => void createSession()}><Plus className="size-4" />New Session</Button> : null,
    [canRun, createSession, creating, view],
  );
  usePageChrome(view === "usage" ? "Personal Agent usage" : "Agent Sessions", pageAction);

  const loadSessions = useCallback(async () => {
    if (!canHistory || view !== "sessions") {
      setHistoryState("forbidden");
      return;
    }
    const requestVersion = ++historyRequestRef.current;
    setHistoryState("loading");
    try {
      const response = await agentService.listAllSessions({
        includeArchived,
        titleSearch: search.trim() || undefined,
      });
      if (requestVersion !== historyRequestRef.current) return;
      if (!response.success || !response.data) {
        if (isForbidden(response)) setHistoryState("forbidden");
        else {
          setHistoryError(agentErrorMessage(response, "Session history could not be loaded."));
          setHistoryState("error");
        }
        return;
      }
      setSessions(response.data.items);
      setHistoryError("");
      setHistoryState("ready");
    } catch {
      if (requestVersion !== historyRequestRef.current) return;
      setHistoryError("Agent could not be reached. Check your connection and try again.");
      setHistoryState("error");
    }
  }, [canHistory, includeArchived, search, view]);

  useEffect(() => {
    const timeout = window.setTimeout(() => void loadSessions(), search ? 250 : 0);
    return () => {
      window.clearTimeout(timeout);
      historyRequestRef.current += 1;
    };
  }, [loadSessions, search]);

  useEffect(() => {
    setSessionContext({
      label: "Agent",
      moduleKey: "agent",
      route: selectedSessionId ? `/modules/agent/sessions/${selectedSessionId}` : "/modules/agent",
    });
  }, [selectedSessionId]);

  if (view === "usage") return <PersonalUsageWorkspace />;

  return (
    <div className="min-w-0 space-y-5">
      <section className="flex flex-col gap-3 border-b border-[var(--border)] pb-5 sm:flex-row sm:items-end sm:justify-between">
        <div>
          <div className="flex items-center gap-2 text-[11px] font-semibold uppercase tracking-[0.16em] text-[var(--brand-strong)]"><Bot className="size-3.5" />Agent</div>
          <h1 className="mt-2 text-2xl font-semibold tracking-[-0.035em] text-[var(--text-strong)]">Sessions</h1>
          <p className="mt-1 text-sm text-[var(--text-muted)]">Open a Session or start a new one.</p>
        </div>
        <Link className="inline-flex min-h-10 items-center gap-2 self-start rounded-[var(--button-radius)] border border-[var(--border)] bg-[var(--surface)] px-4 text-sm font-medium text-[var(--text-strong)] hover:bg-[var(--surface-muted)] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--focus-ring)] sm:self-auto" to="/modules/agent/usage"><BarChart3 className="size-4" />Personal usage</Link>
      </section>

      {createError ? <div className="rounded-[var(--radius-md)] border border-[var(--tone-danger-bd)] bg-[var(--tone-danger-bg)] px-4 py-3 text-sm text-[var(--tone-danger-strong)]" role="alert">{createError}</div> : null}

      <div className="grid min-w-0 gap-5 xl:grid-cols-[320px_minmax(0,1fr)]">
        <aside className={`min-w-0 overflow-hidden rounded-[var(--radius-xl)] border border-[var(--border)] bg-[var(--surface)] shadow-[var(--shadow-card)] xl:order-1 ${selectedSessionId ? "order-2" : "order-1"}`} aria-label="Session history">
          <div className="border-b border-[var(--border)] p-4">
            <div className="flex items-center justify-between gap-3"><h2 className="text-sm font-semibold text-[var(--text-strong)]">History</h2>{historyState === "loading" ? <Loader2 aria-label="Loading history" className="size-4 animate-spin text-[var(--brand)]" /> : null}</div>
            {canHistory ? (
              <>
                <Input className="mt-3" leadingIcon={<Search />} onChange={(event) => setSearch(event.target.value)} placeholder="Search Sessions" type="search" value={search} />
                <label className="mt-3 flex min-h-10 cursor-pointer items-center gap-2 text-xs text-[var(--text-muted)]"><input checked={includeArchived} className="size-4 accent-[var(--brand)]" onChange={(event) => setIncludeArchived(event.target.checked)} type="checkbox" /><Archive className="size-3.5" />Include archived</label>
              </>
            ) : null}
          </div>
          <div className="cp-sidebar-scroll max-h-[640px] overflow-y-auto p-2">
            {historyState === "loading" ? <HistorySkeleton /> : null}
            {historyState === "forbidden" ? <HistoryState icon={<History />} title="History is not available" description="Your current access does not include Session history." /> : null}
            {historyState === "error" ? <HistoryState action={<Button onClick={() => void loadSessions()} size="sm" variant="secondary"><RefreshCw className="size-3.5" />Try again</Button>} icon={<History />} title="History could not be loaded" description={historyError} /> : null}
            {historyState === "ready" && sessions.length === 0 ? <HistoryState icon={<MessageSquareText />} title={search ? "No Sessions match" : "No Sessions yet"} description={search ? "Change the search or include archived Sessions." : canRun ? "Start a new Session when you are ready." : "No Session history is available."} /> : null}
            {historyState === "ready" && sessions.length > 0 ? (
              <div className="space-y-1">
                {sessions.map((session) => <SessionHistoryLink active={session.id === selectedSessionId} key={session.id} session={session} />)}
              </div>
            ) : null}
          </div>
        </aside>

        <div className={`min-w-0 xl:order-2 ${selectedSessionId ? "order-1" : "order-2"}`}>
          {selectedSessionId ? (
            <AgentSessionPanel
              context={sessionContext}
              onArchived={() => { void loadSessions(); void navigate({ to: "/modules/agent" }); }}
              onContextResolved={setSessionContext}
              sessionId={selectedSessionId}
            />
          ) : (
            <section className="flex min-h-[480px] flex-col items-center justify-center rounded-[var(--radius-xl)] border border-[var(--border)] bg-[var(--surface)] p-6 text-center shadow-[var(--shadow-card)]">
              <span className="flex size-12 items-center justify-center rounded-[var(--radius-lg)] bg-[var(--brand-soft)] text-[var(--brand-strong)]"><Bot className="size-5" /></span>
              <h2 className="mt-4 text-lg font-semibold text-[var(--text-strong)]">Choose a Session</h2>
              <p className="mt-2 max-w-md text-sm leading-6 text-[var(--text-muted)]">Open one from your history or start a new Session.</p>
              {canRun ? <Button className="mt-5" disabled={creating} onClick={() => void createSession()}>{creating ? <Loader2 className="size-4 animate-spin" /> : <Plus className="size-4" />}New Session</Button> : null}
            </section>
          )}
        </div>
      </div>
    </div>
  );
}

function SessionHistoryLink({ active, session }: { active: boolean; session: AgentSession }) {
  return (
    <Link
      aria-current={active ? "page" : undefined}
      className={`block min-w-0 rounded-[var(--radius-md)] border px-3 py-3 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--focus-ring)] ${active ? "border-[var(--brand)] bg-[var(--brand-soft)]" : "border-transparent hover:bg-[var(--surface-muted)]"}`}
      params={{ sessionId: session.id }}
      to="/modules/agent/sessions/$sessionId"
    >
      <div className="flex min-w-0 items-start justify-between gap-2"><p className="truncate text-sm font-medium text-[var(--text-strong)]">{session.title}</p>{session.status === "archived" ? <Badge tone="neutral">Archived</Badge> : null}</div>
      <p className="mt-1 flex items-center gap-1 text-[11px] text-[var(--text-subtle)]"><Clock3 className="size-3" />{formatAgentDate(session.last_activity_at)}</p>
    </Link>
  );
}

function HistorySkeleton() {
  return <div aria-label="Loading Session history" className="space-y-2 p-2" role="status">{Array.from({ length: 5 }).map((_, index) => <div className="rounded-[var(--radius-md)] border border-[var(--border)] p-3" key={index}><div className="h-3 animate-pulse rounded-full bg-[var(--surface-sunken)]" /><div className="mt-3 h-2 w-2/3 animate-pulse rounded-full bg-[var(--surface-muted)]" /></div>)}</div>;
}

function HistoryState({ action, description, icon, title }: { action?: React.ReactNode; description: string; icon: React.ReactNode; title: string }) {
  return <div className="p-6 text-center"><span className="mx-auto flex size-10 items-center justify-center rounded-[var(--radius-md)] bg-[var(--surface-muted)] text-[var(--text-muted)] [&_svg]:size-4">{icon}</span><p className="mt-3 text-sm font-semibold text-[var(--text-strong)]">{title}</p><p className="mt-1 text-xs leading-5 text-[var(--text-muted)]">{description}</p>{action ? <div className="mt-3">{action}</div> : null}</div>;
}

function PersonalUsageWorkspace() {
  const [items, setItems] = useState<AgentUsageReportRow[]>([]);
  const [state, setState] = useState<"loading" | "ready" | "error" | "forbidden">("loading");
  const [error, setError] = useState("");

  const load = useCallback(async () => {
    setState("loading");
    try {
      const response = await agentService.getAllPersonalUsage();
      if (!response.success || !response.data) {
        if (isForbidden(response)) setState("forbidden");
        else {
          setError(agentErrorMessage(response, "Personal usage could not be loaded."));
          setState("error");
        }
        return;
      }
      setItems(response.data.items);
      setError("");
      setState("ready");
    } catch {
      setError("Agent could not be reached. Check your connection and try again.");
      setState("error");
    }
  }, []);

  useEffect(() => { void load(); }, [load]);

  return (
    <div className="min-w-0 space-y-5">
      <section className="flex flex-col gap-4 border-b border-[var(--border)] pb-5 sm:flex-row sm:items-end sm:justify-between">
        <div><div className="flex items-center gap-2 text-[11px] font-semibold uppercase tracking-[0.16em] text-[var(--brand-strong)]"><BarChart3 className="size-3.5" />Agent</div><h1 className="mt-2 text-2xl font-semibold tracking-[-0.035em] text-[var(--text-strong)]">Personal usage</h1><p className="mt-1 text-sm text-[var(--text-muted)]">Your recorded Agent activity.</p></div>
        <Link className="inline-flex min-h-10 items-center gap-2 self-start rounded-[var(--button-radius)] border border-[var(--border)] bg-[var(--surface)] px-4 text-sm font-medium text-[var(--text-strong)] hover:bg-[var(--surface-muted)] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--focus-ring)] sm:self-auto" to="/modules/agent"><History className="size-4" />Sessions</Link>
      </section>

      <TableWrap>
        {state === "loading" ? <TableLoading columns={6} label="Loading personal Agent usage…" /> : null}
        {state === "forbidden" ? <TableEmpty description="Your current access does not include personal usage history." icon={<BarChart3 />} title="Usage is not available" /> : null}
        {state === "error" ? <TableError description={error} onRetry={() => void load()} title="Usage could not be loaded" /> : null}
        {state === "ready" && items.length === 0 ? <TableEmpty description="Usage appears after an Agent run is recorded." icon={<BarChart3 />} title="No usage recorded" /> : null}
        {state === "ready" && items.length > 0 ? (
          <TableScroll>
            <Table>
              <THead><tr><TH>When</TH><TH>Activity</TH><TH>Module</TH><TH>Provider</TH><TH>Meter</TH><TH>Amount</TH><TH>Outcome</TH></tr></THead>
              <TBody>{items.map((item) => <TR key={item.event_id}><TD className="whitespace-nowrap text-[var(--text-muted)]">{formatAgentDate(item.occurred_at)}</TD><TD><p className="font-medium text-[var(--text-strong)]">{item.event_kind.replace(/_/g, " ")}</p>{item.capability_key ? <p className="mt-1 max-w-64 truncate text-xs text-[var(--text-muted)]">{item.capability_key}</p> : null}</TD><TD className="text-[var(--text-muted)]">{moduleContextLabel(item.capability_module_key || item.origin_module_key)}</TD><TD className="text-[var(--text-muted)]">{item.provider_key ? `${item.provider_key}${item.provider_model_id ? ` · ${item.provider_model_id}` : ""}` : "—"}</TD><TD className="text-[var(--text-muted)]">{item.meter.replace(/_/g, " ")}</TD><TD className="font-tabular text-[var(--text-strong)]">{formatUsageAmount(item)}</TD><TD><Badge tone={item.outcome === "succeeded" || item.outcome === "completed" ? "success" : item.outcome.includes("denied") || item.outcome === "failed" ? "danger" : "neutral"}>{item.outcome.replace(/_/g, " ")}</Badge></TD></TR>)}</TBody>
            </Table>
          </TableScroll>
        ) : null}
      </TableWrap>
    </div>
  );
}

function formatUsageAmount(item: AgentUsageReportRow) {
  if (item.amount == null) return "Unknown";
  if (item.currency_code && item.currency_exponent != null) {
    return new Intl.NumberFormat(undefined, { style: "currency", currency: item.currency_code }).format(item.amount / 10 ** item.currency_exponent);
  }
  return new Intl.NumberFormat().format(item.amount);
}

function hasPermission(permissions: string[], permission: string) {
  return permissions.includes("*") || permissions.includes(permission);
}

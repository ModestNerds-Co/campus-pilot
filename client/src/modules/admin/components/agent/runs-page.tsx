/**
 * Renders the reduced Agent run and audit trail. Filters and the selected run live in the URL so
 * refresh, Back, and direct links preserve context without persisting provider or message content.
 */

import React, { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Activity, Eye, RefreshCw, Search, SlidersHorizontal } from "lucide-react";

import { SearchableSelect } from "@/components/searchable-select";
import { Button } from "@/components/ui/button";
import {
  Table,
  TableControlsBar,
  TableControlsPagination,
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
import { Input, Label, Select } from "@/components/ui/input";
import { usePageChrome } from "@/modules/admin/layouts/page-chrome";

import { agentGovernanceService, governanceErrorMessage, isGovernanceForbidden } from "./service";
import { AgentStatus, ForbiddenPanel, formatCount, formatDuration, formatTimestamp, statusLabel } from "./shared";
import type { AgentRunAuditPage, AgentRunFilters } from "./types";
import { RunDetailDrawer } from "./run-detail-drawer";

export function AgentRunsPage({
  filters,
  selectedRunId,
  onFiltersChange,
  onSelectedRunChange,
}: {
  filters: AgentRunFilters;
  selectedRunId?: string;
  onFiltersChange: (filters: AgentRunFilters) => void;
  onSelectedRunChange: (runId?: string) => void;
}) {
  const [data, setData] = useState<AgentRunAuditPage | null>(null);
  const [draft, setDraft] = useState(filters);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [forbidden, setForbidden] = useState(false);
  const generation = useRef(0);

  useEffect(() => setDraft(filters), [filters]);

  const load = useCallback(async () => {
    const requestGeneration = ++generation.current;
    setLoading(true);
    setError(null);
    setForbidden(false);
    try {
      const response = await agentGovernanceService.runs(filters);
      if (requestGeneration !== generation.current) return;
      if (!response.success || !response.data) {
        setForbidden(isGovernanceForbidden(response));
        setError(governanceErrorMessage(response, "Agent runs could not be loaded."));
        return;
      }
      setData(response.data);
    } catch {
      if (requestGeneration === generation.current) {
        setError("Campus Pilot could not reach Agent runs. Check the connection and try again.");
      }
    } finally {
      if (requestGeneration === generation.current) setLoading(false);
    }
  }, [filters]);

  useEffect(() => {
    void load();
    return () => { generation.current += 1; };
  }, [load]);

  const action = useMemo(() => (
    <Button disabled={loading} onClick={() => void load()} variant="secondary">
      <RefreshCw className={`size-4 ${loading ? "animate-spin" : ""}`} />Refresh
    </Button>
  ), [load, loading]);
  usePageChrome("Runs and audit", action);

  if (forbidden) return <ForbiddenPanel area="Agent run history" />;

  const apply = () => onFiltersChange({
    ...draft,
    from: dateStart(draft.from),
    to: dateEndExclusive(draft.to),
    page: 1,
  });

  return (
    <div className="space-y-6">
      <form
        className="grid gap-4 rounded-[var(--radius-xl)] border border-[var(--border)] bg-[var(--surface)] p-4 shadow-[var(--shadow-card)] md:grid-cols-2 xl:grid-cols-4"
        onSubmit={(event) => { event.preventDefault(); apply(); }}
      >
        <div><Label htmlFor="agent-runs-from">From</Label><Input className="mt-1.5" id="agent-runs-from" onChange={(event) => setDraft((value) => ({ ...value, from: event.target.value || undefined }))} type="date" value={dateValue(draft.from)} /></div>
        <div><Label htmlFor="agent-runs-to">To</Label><Input className="mt-1.5" id="agent-runs-to" onChange={(event) => setDraft((value) => ({ ...value, to: event.target.value || undefined }))} type="date" value={dateValue(draft.to, true)} /></div>
        <div><Label htmlFor="agent-runs-status">Status</Label><Select className="mt-1.5" id="agent-runs-status" onChange={(event) => setDraft((value) => ({ ...value, status: event.target.value || undefined }))} value={draft.status || ""}><option value="">All statuses</option>{RUN_STATUSES.map((status) => <option key={status} value={status}>{statusLabel(status)}</option>)}</Select></div>
        <div><Label htmlFor="agent-runs-person">Person</Label><SearchableSelect className="mt-1.5" id="agent-runs-person" loading={!data && loading} onChange={(value) => setDraft((current) => ({ ...current, person_id: value || undefined }))} options={(data?.people || []).map((person) => ({ id: person.id, value: person.name, label: person.name }))} placeholder="All people" value={draft.person_id || null} /></div>
        <div><Label htmlFor="agent-runs-module">Opened from</Label><Select className="mt-1.5" id="agent-runs-module" onChange={(event) => setDraft((value) => ({ ...value, origin_module: event.target.value || undefined }))} value={draft.origin_module || ""}><option value="">All modules</option>{data?.modules.map((module) => <option key={module.key} value={module.key}>{module.label}</option>)}</Select></div>
        <div><Label htmlFor="agent-runs-correlation">Correlation ID</Label><Input className="mt-1.5" id="agent-runs-correlation" onChange={(event) => setDraft((value) => ({ ...value, correlation_id: event.target.value.trim() || undefined }))} placeholder="UUID" value={draft.correlation_id || ""} /></div>
        <div className="md:col-span-2"><Label htmlFor="agent-runs-search">Session</Label><Input className="mt-1.5" id="agent-runs-search" leadingIcon={<Search />} onChange={(event) => setDraft((value) => ({ ...value, search: event.target.value || undefined }))} placeholder="Search session title" value={draft.search || ""} /></div>
        <div className="flex items-end gap-2 md:col-span-2 xl:col-span-4">
          <Button type="submit"><SlidersHorizontal className="size-4" />Apply filters</Button>
          <Button onClick={() => { setDraft({}); onFiltersChange({}); }} type="button" variant="secondary">Clear</Button>
          {data && data.total_pages > 0 ? (
            <TableControlsPagination
              className="ml-auto"
              onNext={() => onFiltersChange({ ...filters, page: Math.min(data.total_pages, data.page + 1) })}
              onPrevious={() => onFiltersChange({ ...filters, page: Math.max(1, data.page - 1) })}
              page={data.page}
              totalPages={data.total_pages}
            />
          ) : null}
        </div>
      </form>

      {error && data ? <div className="border border-[var(--tone-danger-border)] bg-[var(--tone-danger-bg)] px-4 py-3 text-sm text-[var(--tone-danger-strong)]" role="alert">{error} Existing results remain visible.</div> : null}

      {!data && loading ? (
        <TableWrap><TableLoading columns={6} label="Loading Agent runs…" rows={8} /></TableWrap>
      ) : !data && error ? (
        <TableWrap><TableError description={error} onRetry={() => void load()} title="Agent runs could not be loaded" /></TableWrap>
      ) : data && data.items.length === 0 ? (
        <TableWrap><TableEmpty description="Change the date range or filters to inspect other runs." icon={<Activity />} title="No Agent runs match these filters" /></TableWrap>
      ) : data ? (
        <TableWrap aria-busy={loading}>
          <TableControlsBar className="border-0 border-b shadow-none">
            <p className="text-sm text-[var(--text-muted)]">{formatCount(data.total)} runs</p>
          </TableControlsBar>
          <TableScroll><Table><THead><TR><TH>Session</TH><TH>Person</TH><TH>Status</TH><TH>Started</TH><TH>Activity</TH><TH><span className="sr-only">Inspect</span></TH></TR></THead><TBody>
            {data.items.map((run) => (
              <TR key={run.id}>
                <TD className="min-w-64"><p className="font-medium text-[var(--text-strong)]">{run.session_title}</p><p className="mt-1 text-xs text-[var(--text-muted)]">{statusLabel(run.origin_module_key)} · {statusLabel(run.task_class)}</p></TD>
                <TD><p className="font-medium text-[var(--text-strong)]">{run.requested_by_name}</p><code className="mt-1 block text-xs text-[var(--text-muted)]">{run.correlation_id}</code></TD>
                <TD><AgentStatus value={run.status} />{run.safe_failure_code ? <p className="mt-1 text-xs text-[var(--tone-danger-strong)]">{statusLabel(run.safe_failure_code)}</p> : null}</TD>
                <TD><p>{formatTimestamp(run.started_at || run.created_at)}</p><p className="mt-1 text-xs text-[var(--text-muted)]">{formatDuration(run.started_at, run.finished_at)}</p></TD>
                <TD className="tabular-nums"><p>{formatCount(run.provider_attempts)} provider attempts</p><p className="mt-1 text-xs text-[var(--text-muted)]">{formatCount(run.capability_calls)} capability calls</p></TD>
                <TD className="text-right"><Button aria-label={`Inspect ${run.session_title}`} onClick={() => onSelectedRunChange(run.id)} size="sm" variant="secondary"><Eye className="size-4" />Inspect</Button></TD>
              </TR>
            ))}
          </TBody></Table></TableScroll>
        </TableWrap>
      ) : null}

      <RunDetailDrawer onClose={() => onSelectedRunChange(undefined)} runId={selectedRunId} />
    </div>
  );
}

const RUN_STATUSES = ["queued", "running", "awaiting_approval", "completed", "failed", "cancelled", "interrupted"];

function dateValue(value: string | undefined, exclusiveEnd = false) {
  if (!value) return "";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return "";
  if (exclusiveEnd) date.setUTCDate(date.getUTCDate() - 1);
  return date.toISOString().slice(0, 10);
}

function dateStart(value: string | undefined) {
  if (!value || value.includes("T")) return value;
  return new Date(`${value}T00:00:00.000Z`).toISOString();
}

function dateEndExclusive(value: string | undefined) {
  if (!value || value.includes("T")) return value;
  const date = new Date(`${value}T00:00:00.000Z`);
  date.setUTCDate(date.getUTCDate() + 1);
  return date.toISOString();
}

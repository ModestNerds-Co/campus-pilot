/** Reduced, secret-free detail drawer for one Agent run. */

import React, { useCallback, useEffect, useRef, useState } from "react";
import { AlertTriangle, RefreshCw } from "lucide-react";

import { Button } from "@/components/ui/button";
import { DialogBody, DialogFooter, DialogHeader, DialogShell } from "@/components/ui/dialog";

import { agentGovernanceService, governanceErrorMessage, isGovernanceForbidden } from "./service";
import { AgentStatus, formatCount, formatDuration, formatTimestamp, formatUsageAmount, statusLabel } from "./shared";
import type { AgentRunAuditDetail } from "./types";

export function RunDetailDrawer({ runId, onClose }: { runId?: string; onClose: () => void }) {
  const [detail, setDetail] = useState<AgentRunAuditDetail | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [forbidden, setForbidden] = useState(false);
  const [notFound, setNotFound] = useState(false);
  const generation = useRef(0);

  const load = useCallback(async () => {
    if (!runId) return;
    const requestGeneration = ++generation.current;
    setLoading(true);
    setDetail(null);
    setError(null);
    setForbidden(false);
    setNotFound(false);
    try {
      const response = await agentGovernanceService.run(runId);
      if (requestGeneration !== generation.current) return;
      if (!response.success || !response.data) {
        setForbidden(isGovernanceForbidden(response));
        setNotFound(response.http_status === 404);
        setError(governanceErrorMessage(response, "This Agent run could not be loaded."));
        return;
      }
      setDetail(response.data);
    } catch {
      if (requestGeneration === generation.current) {
        setError("Campus Pilot could not reach this Agent run. Check the connection and try again.");
      }
    } finally {
      if (requestGeneration === generation.current) setLoading(false);
    }
  }, [runId]);

  useEffect(() => {
    void load();
    return () => { generation.current += 1; };
  }, [load]);

  return (
    <DialogShell onClose={onClose} open={Boolean(runId)} panelClassName="sm:max-w-[760px]">
      <DialogHeader onClose={onClose} title="Agent run" />
      <DialogBody className="space-y-7">
        {loading ? <DrawerLoading /> : forbidden ? <DrawerMessage detail="Your account cannot inspect Agent run history." title="Access required" /> : notFound ? <DrawerMessage detail="The run may have been removed, or it does not belong to this campus." title="Agent run not found" /> : error ? <DrawerMessage action={<Button onClick={() => void load()} size="sm" variant="secondary"><RefreshCw className="size-4" />Try again</Button>} detail={error} title="Agent run could not be loaded" /> : detail ? <RunDetail detail={detail} /> : null}
      </DialogBody>
      <DialogFooter><Button onClick={onClose} variant="secondary">Close</Button></DialogFooter>
    </DialogShell>
  );
}

function RunDetail({ detail }: { detail: AgentRunAuditDetail }) {
  const { run } = detail;
  return (
    <>
      <section aria-labelledby="run-summary-title">
        <div className="flex flex-wrap items-start justify-between gap-3">
          <div><p className="text-xs font-semibold uppercase tracking-[0.14em] text-[var(--text-muted)]">Session</p><h3 className="mt-1 text-xl font-semibold text-[var(--text-strong)]" id="run-summary-title">{run.session_title}</h3><p className="mt-1 text-sm text-[var(--text-muted)]">{run.requested_by_name} · {statusLabel(run.origin_module_key)}</p></div>
          <AgentStatus value={run.status} />
        </div>
        <dl className="mt-5 grid gap-px overflow-hidden rounded-[var(--radius-lg)] border border-[var(--border)] bg-[var(--border)] sm:grid-cols-2">
          <Fact label="Started" value={formatTimestamp(run.started_at || run.created_at)} />
          <Fact label="Duration" value={formatDuration(run.started_at, run.finished_at)} />
          <Fact label="Task class" value={statusLabel(run.task_class)} />
          <Fact label="Correlation ID" mono value={run.correlation_id} />
          {run.safe_failure_code ? <Fact label="Failure code" value={statusLabel(run.safe_failure_code)} /> : null}
        </dl>
      </section>

      <AuditSection empty="No provider attempt was recorded." title={`Provider attempts (${formatCount(detail.provider_attempts.length)})`}>
        {detail.provider_attempts.map((attempt) => (
          <article className="rounded-[var(--radius-lg)] border border-[var(--border)] p-4" key={attempt.id}>
            <div className="flex flex-wrap items-start justify-between gap-3"><div><p className="font-medium text-[var(--text-strong)]">{statusLabel(attempt.provider_key)} · {attempt.provider_model_id}</p><p className="mt-1 text-xs text-[var(--text-muted)]">Turn {attempt.turn_index}, attempt {attempt.attempt_index} · {formatTimestamp(attempt.started_at)}</p></div><AgentStatus value={attempt.status} /></div>
            <div className="mt-3 grid grid-cols-2 gap-3 text-sm sm:grid-cols-4"><FactInline label="Input" value={optionalCount(attempt.input_tokens)} /><FactInline label="Output" value={optionalCount(attempt.output_tokens)} /><FactInline label="Cached" value={optionalCount(attempt.cached_tokens)} /><FactInline label="Reasoning" value={optionalCount(attempt.reasoning_tokens)} /></div>
            <p className="mt-3 text-xs text-[var(--text-muted)]">Provider cost: {optionalAmount(attempt.provider_reported_cost_amount, attempt.provider_reported_cost_currency, attempt.provider_reported_cost_exponent)} · Estimated cost: {optionalAmount(attempt.estimated_cost_amount, attempt.estimated_cost_currency, attempt.estimated_cost_exponent)}</p>
            {attempt.failure_category ? <p className="mt-2 text-xs text-[var(--tone-danger-strong)]">{statusLabel(attempt.failure_origin || "provider")} · {statusLabel(attempt.failure_category)}</p> : null}
          </article>
        ))}
      </AuditSection>

      <AuditSection empty="No capability call was recorded." title={`Capability calls (${formatCount(detail.capability_calls.length)})`}>
        {detail.capability_calls.map((call) => (
          <article className="rounded-[var(--radius-lg)] border border-[var(--border)] p-4" key={call.id}>
            <div className="flex flex-wrap items-start justify-between gap-3"><div><p className="font-medium text-[var(--text-strong)]">{call.capability_key}</p><p className="mt-1 text-xs text-[var(--text-muted)]">{statusLabel(call.owning_module_key)} · call {call.call_sequence} · version {call.capability_version}</p></div><AgentStatus value={call.status} /></div>
            <p className="mt-3 text-xs text-[var(--text-muted)]">{formatCount(call.resource_count)} scoped resources · {call.duration_ms == null ? "Duration not recorded" : `${formatCount(call.duration_ms)} ms`} · {call.required_permission}</p>
            {call.safe_failure_code ? <p className="mt-2 text-xs text-[var(--tone-danger-strong)]">{statusLabel(call.safe_failure_code)}</p> : null}
          </article>
        ))}
      </AuditSection>

      <AuditSection empty="No run event was recorded." title={`Run events (${formatCount(detail.events.length)})`}>
        {detail.events.map((event) => <div className="flex items-start justify-between gap-4 border-b border-[var(--border-subtle)] py-3 last:border-b-0" key={event.event_id}><p className="font-medium text-[var(--text-strong)]">{statusLabel(event.event_type)}</p><time className="text-right text-xs text-[var(--text-muted)]">{formatTimestamp(event.created_at)}</time></div>)}
      </AuditSection>

      <AuditSection empty="No actor audit event was recorded." title={`Actor audit (${formatCount(detail.audit_events.length)})`}>
        {detail.audit_events.map((event) => <div className="border-b border-[var(--border-subtle)] py-3 last:border-b-0" key={event.id}><div className="flex flex-wrap items-start justify-between gap-3"><p className="font-medium text-[var(--text-strong)]">{statusLabel(event.action_key)}</p><AgentStatus value={event.outcome} /></div><p className="mt-1 text-xs text-[var(--text-muted)]">{event.actor_name || statusLabel(event.actor_type)} · {event.target_type ? statusLabel(event.target_type) : "No target type"} · {formatTimestamp(event.occurred_at)}</p></div>)}
      </AuditSection>
    </>
  );
}

function AuditSection({ title, empty, children }: { title: string; empty: string; children: React.ReactNode }) {
  const items = React.Children.count(children);
  return <section><h3 className="text-base font-semibold text-[var(--text-strong)]">{title}</h3><div className="mt-3 space-y-3">{items ? children : <p className="rounded-[var(--radius-lg)] border border-dashed border-[var(--border)] p-4 text-sm text-[var(--text-muted)]">{empty}</p>}</div></section>;
}

function Fact({ label, value, mono = false }: { label: string; value: string; mono?: boolean }) {
  return <div className="min-w-0 bg-[var(--surface-muted)] p-4"><dt className="text-xs text-[var(--text-muted)]">{label}</dt><dd className={`mt-1 break-words text-sm font-medium text-[var(--text-strong)] ${mono ? "font-mono text-xs" : ""}`}>{value}</dd></div>;
}

function FactInline({ label, value }: { label: string; value: string }) {
  return <div><p className="text-xs text-[var(--text-muted)]">{label}</p><p className="mt-1 tabular-nums text-[var(--text-strong)]">{value}</p></div>;
}

function DrawerLoading() {
  return <div aria-busy="true" aria-label="Loading Agent run" className="space-y-4" role="status">{Array.from({ length: 5 }).map((_, index) => <div className="h-20 animate-pulse rounded-[var(--radius-lg)] bg-[var(--surface-muted)]" key={index} />)}<span className="sr-only">Loading Agent run…</span></div>;
}

function DrawerMessage({ title, detail, action }: { title: string; detail: string; action?: React.ReactNode }) {
  return <div className="flex gap-3 rounded-[var(--radius-lg)] border border-[var(--border)] p-4" role="alert"><AlertTriangle className="mt-0.5 size-5 shrink-0 text-[var(--tone-danger)]" /><div><h3 className="font-semibold text-[var(--text-strong)]">{title}</h3><p className="mt-1 text-sm leading-6 text-[var(--text-muted)]">{detail}</p>{action ? <div className="mt-4">{action}</div> : null}</div></div>;
}

function optionalCount(value: number | null) {
  return value == null ? "Not recorded" : formatCount(value);
}

function optionalAmount(value: number | null, currency: string | null, exponent: number | null) {
  return value == null ? "Not recorded" : formatUsageAmount(value, currency, exponent);
}

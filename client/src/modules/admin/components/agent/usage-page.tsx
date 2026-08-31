/**
 * Renders tenant Agent usage from immutable server evidence with URL-owned bounded filters.
 * Unknown provider usage remains visibly unknown, and costs are never combined across currency,
 * exponent, or pricing-version tuples.
 */

import React, { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Download, Gauge, RefreshCw, SlidersHorizontal } from "lucide-react";
import toast from "react-hot-toast";

import { SearchableSelect } from "@/components/searchable-select";
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
import { Input, Label, Select } from "@/components/ui/input";
import { usePageChrome } from "@/modules/admin/layouts/page-chrome";
import { useAuthStore } from "@/stores/auth-store";

import { agentGovernanceService, governanceErrorMessage, isGovernanceForbidden } from "./service";
import {
  AgentMetric,
  ForbiddenPanel,
  formatCount,
  formatTimestamp,
  formatUsageAmount,
  statusLabel,
} from "./shared";
import type { AgentUsageFilterOptions, AgentUsageFilters, AgentUsageReport } from "./types";

export function AgentUsagePage({
  filters,
  onFiltersChange,
}: {
  filters: AgentUsageFilters;
  onFiltersChange: (filters: AgentUsageFilters) => void;
}) {
  const permissions = useAuthStore((state) => state.user?.permissions);
  const canExport = permissions?.includes("*") || permissions?.includes("agent_usage:export");
  const [report, setReport] = useState<AgentUsageReport | null>(null);
  const [options, setOptions] = useState<AgentUsageFilterOptions | null>(null);
  const [draft, setDraft] = useState(filters);
  const [loading, setLoading] = useState(true);
  const [exporting, setExporting] = useState(false);
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
      const [optionsResponse, usageResponse] = await Promise.all([
        agentGovernanceService.usageOptions(),
        agentGovernanceService.usage(filters),
      ]);
      if (requestGeneration !== generation.current) return;
      if (!optionsResponse.success || !optionsResponse.data) {
        setForbidden(isGovernanceForbidden(optionsResponse));
        setError(governanceErrorMessage(optionsResponse, "Usage filters could not be loaded."));
        return;
      }
      if (!usageResponse.success || !usageResponse.data) {
        setForbidden(isGovernanceForbidden(usageResponse));
        setError(governanceErrorMessage(usageResponse, "Agent usage could not be loaded."));
        return;
      }
      setOptions(optionsResponse.data);
      setReport(usageResponse.data);
    } catch {
      if (requestGeneration === generation.current) {
        setError("Campus Pilot could not reach Agent usage. Check the connection and try again.");
      }
    } finally {
      if (requestGeneration === generation.current) setLoading(false);
    }
  }, [filters]);

  useEffect(() => {
    void load();
    return () => { generation.current += 1; };
  }, [load]);

  const exportReport = useCallback(async () => {
    if (exporting) return;
    setExporting(true);
    try {
      const result = await agentGovernanceService.exportUsage(filters);
      const url = URL.createObjectURL(result.blob);
      const link = document.createElement("a");
      link.href = url;
      link.download = "campus-pilot-agent-usage.csv";
      document.body.append(link);
      link.click();
      link.remove();
      URL.revokeObjectURL(url);
      if (result.truncated) toast("The export was limited to 10,000 rows.");
    } catch {
      toast.error("Agent usage could not be exported.");
    } finally {
      setExporting(false);
    }
  }, [exporting, filters]);

  const action = useMemo(() => (
    <div className="flex items-center gap-2">
      <Button disabled={loading} onClick={() => void load()} size="sm" variant="secondary">
        <RefreshCw className={`size-4 ${loading ? "animate-spin" : ""}`} />Refresh
      </Button>
      {canExport ? (
        <Button disabled={exporting || loading || !report} onClick={() => void exportReport()} size="sm">
          <Download className="size-4" />{exporting ? "Exporting…" : "Export CSV"}
        </Button>
      ) : null}
    </div>
  ), [canExport, exportReport, exporting, load, loading, report]);
  usePageChrome("Usage and limits", action);

  if (forbidden) return <ForbiddenPanel area="campus-wide Agent usage" />;

  const applyDates = (next: AgentUsageFilters) => {
    const from = dateStart(next.from);
    const to = dateEndExclusive(next.to);
    onFiltersChange({ ...next, from, to });
  };

  return (
    <div className="space-y-6">
      <form
        className="grid gap-4 border border-[var(--border)] bg-[var(--surface)] p-4 shadow-[var(--shadow-card)] md:grid-cols-2 xl:grid-cols-4"
        onSubmit={(event) => { event.preventDefault(); applyDates(draft); }}
      >
        <div><Label htmlFor="agent-usage-from">From</Label><Input className="mt-1.5" id="agent-usage-from" onChange={(event) => setDraft((value) => ({ ...value, from: event.target.value || undefined }))} type="date" value={dateValue(draft.from)} /></div>
        <div><Label htmlFor="agent-usage-to">To</Label><Input className="mt-1.5" id="agent-usage-to" onChange={(event) => setDraft((value) => ({ ...value, to: event.target.value || undefined }))} type="date" value={dateValue(draft.to, true)} /></div>
        <div><Label htmlFor="agent-usage-person">Person</Label><SearchableSelect className="mt-1.5" id="agent-usage-person" loading={!options && loading} onChange={(value) => setDraft((current) => ({ ...current, person_id: value || undefined }))} options={(options?.people || []).map((person) => ({ id: person.id, value: person.name, label: person.name }))} placeholder="All people" value={draft.person_id || null} /></div>
        <div><Label htmlFor="agent-usage-origin">Opened from</Label><Select className="mt-1.5" id="agent-usage-origin" onChange={(event) => setDraft((value) => ({ ...value, origin_module: event.target.value || undefined }))} value={draft.origin_module || ""}><option value="">All modules</option>{options?.modules.map((module) => <option key={module.key} value={module.key}>{module.label}</option>)}</Select></div>
        <div><Label htmlFor="agent-usage-cap-module">Capability module</Label><Select className="mt-1.5" id="agent-usage-cap-module" onChange={(event) => setDraft((value) => ({ ...value, capability_module: event.target.value || undefined }))} value={draft.capability_module || ""}><option value="">All modules</option>{options?.modules.map((module) => <option key={module.key} value={module.key}>{module.label}</option>)}</Select></div>
        <div><Label htmlFor="agent-usage-capability">Capability</Label><SearchableSelect className="mt-1.5" id="agent-usage-capability" loading={!options && loading} onChange={(value) => setDraft((current) => ({ ...current, capability: value || undefined }))} options={(options?.capabilities || []).map((capability) => ({ id: capability.key, value: capability.label, label: capability.label, description: capability.key }))} placeholder="All capabilities" value={draft.capability || null} /></div>
        <div><Label htmlFor="agent-usage-provider">Provider</Label><Select className="mt-1.5" id="agent-usage-provider" onChange={(event) => setDraft((value) => ({ ...value, provider: event.target.value || undefined, model: undefined }))} value={draft.provider || ""}><option value="">All providers</option>{options?.providers.map((provider) => <option key={provider} value={provider}>{statusLabel(provider)}</option>)}</Select></div>
        <div><Label htmlFor="agent-usage-model">Model</Label><SearchableSelect className="mt-1.5" id="agent-usage-model" onChange={(value) => setDraft((current) => ({ ...current, model: value || undefined }))} options={(options?.models || []).filter((model) => !draft.provider || model.provider === draft.provider).map((model) => ({ id: model.model, value: model.model, label: model.model, description: statusLabel(model.provider) }))} placeholder="All models" value={draft.model || null} /></div>
        <div><Label htmlFor="agent-usage-outcome">Outcome</Label><Select className="mt-1.5" id="agent-usage-outcome" onChange={(event) => setDraft((value) => ({ ...value, outcome: event.target.value || undefined }))} value={draft.outcome || ""}><option value="">All outcomes</option>{options?.outcomes.map((outcome) => <option key={outcome} value={outcome}>{statusLabel(outcome)}</option>)}</Select></div>
        <div><Label htmlFor="agent-usage-meter">Meter</Label><Select className="mt-1.5" id="agent-usage-meter" onChange={(event) => setDraft((value) => ({ ...value, meter: event.target.value || undefined }))} value={draft.meter || ""}><option value="">All meters</option>{options?.meters.map((meter) => <option key={meter} value={meter}>{meterLabel(meter)}</option>)}</Select></div>
        <div className="flex items-end gap-2 md:col-span-2 xl:col-span-2">
          <Button type="submit"><SlidersHorizontal className="size-4" />Apply filters</Button>
          <Button onClick={() => { setDraft({}); onFiltersChange({}); }} type="button" variant="secondary">Clear</Button>
        </div>
      </form>

      {error && report ? <div className="border border-[var(--tone-danger-border)] bg-[var(--tone-danger-bg)] px-4 py-3 text-sm text-[var(--tone-danger-strong)]" role="alert">{error} Existing results remain visible.</div> : null}

      {!report && loading ? (
        <TableWrap><TableLoading columns={4} label="Loading Agent usage…" rows={6} /></TableWrap>
      ) : !report && error ? (
        <TableWrap><TableError description={error} onRetry={() => void load()} title="Agent usage could not be loaded" /></TableWrap>
      ) : report && report.totals.length === 0 ? (
        <TableWrap><TableEmpty description="Change the date range or filters to inspect other usage." icon={<Gauge />} title="No usage was recorded for this range" /></TableWrap>
      ) : report ? (
        <>
          <section aria-labelledby="usage-totals-title">
            <div className="mb-3"><h2 className="text-xl font-semibold tracking-[-0.025em] text-[var(--text-strong)]" id="usage-totals-title">Totals</h2><p className="mt-1 text-xs text-[var(--text-muted)]">{formatTimestamp(report.from)} to {formatTimestamp(report.to)}</p></div>
            <dl className="grid grid-cols-2 overflow-hidden border border-[var(--border)] bg-[var(--surface)] lg:grid-cols-4">
              {report.totals.slice(0, 8).map((total) => <AgentMetric detail={total.unknown_events ? `${formatCount(total.unknown_events)} unknown` : undefined} key={usageTupleKey(total)} label={meterLabel(total.meter)} value={formatUsageAmount(total.known_amount, total.currency, total.exponent)} />)}
            </dl>
          </section>
          <section aria-labelledby="usage-trend-title">
            <h2 className="mb-3 text-xl font-semibold tracking-[-0.025em] text-[var(--text-strong)]" id="usage-trend-title">Daily usage</h2>
            <TableWrap><TableScroll><Table><THead><TR><TH>Day</TH><TH>Meter</TH><TH>Known amount</TH><TH>Unknown</TH><TH>Pricing</TH></TR></THead><TBody>
              {report.trend.map((point) => <TR key={`${point.day}-${usageTupleKey(point)}`}><TD>{new Date(point.day).toLocaleDateString()}</TD><TD className="font-medium text-[var(--text-strong)]">{meterLabel(point.meter)}</TD><TD className="tabular-nums">{formatUsageAmount(point.known_amount, point.currency, point.exponent)}</TD><TD className="tabular-nums">{point.unknown_events ? formatCount(point.unknown_events) : "—"}</TD><TD className="text-xs text-[var(--text-muted)]">{point.pricing_version || "Not applicable"}</TD></TR>)}
            </TBody></Table></TableScroll></TableWrap>
          </section>
        </>
      ) : null}
    </div>
  );
}

function meterLabel(value: string) {
  return statusLabel(value.replace(/^agent\./, ""));
}

function usageTupleKey(value: { meter: string; currency: string | null; exponent: number | null; pricing_version: string | null }) {
  return `${value.meter}:${value.currency || "count"}:${value.exponent ?? "none"}:${value.pricing_version || "current"}`;
}

function dateValue(value: string | undefined, exclusiveEnd = false) {
  if (!value) return "";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return "";
  if (exclusiveEnd) date.setUTCDate(date.getUTCDate() - 1);
  return date.toISOString().slice(0, 10);
}

function dateStart(value: string | undefined) {
  if (!value) return undefined;
  if (value.includes("T")) return value;
  return new Date(`${value}T00:00:00.000Z`).toISOString();
}

function dateEndExclusive(value: string | undefined) {
  if (!value) return undefined;
  if (value.includes("T")) return value;
  const date = new Date(`${value}T00:00:00.000Z`);
  date.setUTCDate(date.getUTCDate() + 1);
  return date.toISOString();
}

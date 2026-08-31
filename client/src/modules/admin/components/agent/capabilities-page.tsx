/**
 * Renders the code-owned operation coverage matrix intersected with current campus availability.
 * URL-owned filters survive refresh and browser history; the page never treats classification as
 * execution authority.
 */

import React, { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { BrainCircuit, RefreshCw, Search, SlidersHorizontal } from "lucide-react";

import { Button } from "@/components/ui/button";
import {
  Table,
  TableControlsBar,
  TableControlsPagination,
  TableControlsSearch,
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
import { Input, Select } from "@/components/ui/input";
import { usePageChrome } from "@/modules/admin/layouts/page-chrome";

import { agentGovernanceService, governanceErrorMessage, isGovernanceForbidden } from "./service";
import { AgentMetric, AgentStatus, ForbiddenPanel, formatCount, statusLabel } from "./shared";
import type {
  AgentCapabilityAvailability,
  AgentCapabilityFilters,
  AgentCapabilityInventoryPage,
  AgentExposure,
} from "./types";

export function AgentCapabilitiesPage({
  filters,
  onFiltersChange,
}: {
  filters: AgentCapabilityFilters;
  onFiltersChange: (filters: AgentCapabilityFilters) => void;
}) {
  const [data, setData] = useState<AgentCapabilityInventoryPage | null>(null);
  const [search, setSearch] = useState(filters.search || "");
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [forbidden, setForbidden] = useState(false);
  const generation = useRef(0);

  useEffect(() => setSearch(filters.search || ""), [filters.search]);

  const load = useCallback(async () => {
    const requestGeneration = ++generation.current;
    setLoading(true);
    setError(null);
    setForbidden(false);
    try {
      const response = await agentGovernanceService.capabilities(filters);
      if (requestGeneration !== generation.current) return;
      if (!response.success || !response.data) {
        setForbidden(isGovernanceForbidden(response));
        setError(governanceErrorMessage(response, "Capability coverage could not be loaded."));
        return;
      }
      setData(response.data);
    } catch {
      if (requestGeneration === generation.current) {
        setError("Campus Pilot could not reach capability coverage. Check the connection and try again.");
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
  usePageChrome("Capabilities and approvals", action);

  if (forbidden) return <ForbiddenPanel area="capability governance" />;

  const update = (changes: Partial<AgentCapabilityFilters>) => {
    onFiltersChange({ ...filters, ...changes, page: changes.page ?? 1 });
  };

  return (
    <div className="space-y-6">
      {data ? (
        <dl className="grid grid-cols-2 overflow-hidden border border-[var(--border)] bg-[var(--surface)] sm:grid-cols-3 xl:grid-cols-6">
          <AgentMetric label="Operations" value={formatCount(data.summary.total)} />
          <AgentMetric label="Executable" value={formatCount(data.summary.executable)} />
          <AgentMetric label="Exposed" value={formatCount(data.summary.exposed)} />
          <AgentMetric label="Approval" value={formatCount(data.summary.approval_required)} />
          <AgentMetric label="Human only" value={formatCount(data.summary.human_only)} />
          <AgentMetric label="Prohibited" value={formatCount(data.summary.prohibited)} />
        </dl>
      ) : null}

      <TableControlsBar aria-label="Capability filters">
        <TableControlsSearch
          onSubmit={(event) => {
            event.preventDefault();
            update({ search: search.trim() || undefined });
          }}
        >
          <Input
            aria-label="Search capabilities"
            leadingIcon={<Search />}
            onChange={(event) => setSearch(event.target.value)}
            placeholder="Search operation, module, or permission"
            value={search}
          />
          <Button type="submit" variant="secondary">Search</Button>
        </TableControlsSearch>
        <Select
          aria-label="Module filter"
          className="sm:w-48"
          onChange={(event) => update({ module: event.target.value || undefined })}
          value={filters.module || ""}
        >
          <option value="">All modules</option>
          {data?.modules.map((module) => <option key={module.key} value={module.key}>{module.label}</option>)}
        </Select>
        <Select
          aria-label="Exposure filter"
          className="sm:w-44"
          onChange={(event) => update({ exposure: (event.target.value || undefined) as AgentExposure | undefined })}
          value={filters.exposure || ""}
        >
          <option value="">All exposure</option>
          <option value="exposed">Exposed</option>
          <option value="approval_required">Approval required</option>
          <option value="human_only">Human only</option>
          <option value="prohibited">Prohibited</option>
        </Select>
        <Select
          aria-label="Availability filter"
          className="sm:w-48"
          onChange={(event) => update({ availability: (event.target.value || undefined) as AgentCapabilityAvailability | undefined })}
          value={filters.availability || ""}
        >
          <option value="">All availability</option>
          <option value="executable">Executable</option>
          <option value="module_unavailable">Module unavailable</option>
          <option value="approval_not_released">Approval not released</option>
          <option value="handler_unavailable">Handler unavailable</option>
          <option value="human_only">Human only</option>
          <option value="prohibited">Prohibited</option>
        </Select>
        {data && data.total_pages > 0 ? (
          <TableControlsPagination
            onNext={() => onFiltersChange({ ...filters, page: Math.min(data.total_pages, data.page + 1) })}
            onPrevious={() => onFiltersChange({ ...filters, page: Math.max(1, data.page - 1) })}
            page={data.page}
            totalPages={data.total_pages}
          />
        ) : null}
      </TableControlsBar>

      {error && data ? (
        <div className="border border-[var(--tone-danger-border)] bg-[var(--tone-danger-bg)] px-4 py-3 text-sm text-[var(--tone-danger-strong)]" role="alert">
          {error} Existing results remain visible.
        </div>
      ) : null}

      {!data && loading ? (
        <TableWrap><TableLoading columns={5} label="Loading capability coverage…" rows={8} /></TableWrap>
      ) : !data && error ? (
        <TableWrap><TableError description={error} onRetry={() => void load()} title="Capability coverage could not be loaded" /></TableWrap>
      ) : data && data.items.length === 0 ? (
        <TableWrap>
          <TableEmpty
            description="Clear one or more filters to see catalogued operations."
            icon={<SlidersHorizontal />}
            title="No capabilities match these filters"
          />
        </TableWrap>
      ) : data ? (
        <TableWrap aria-busy={loading}>
          <TableScroll>
            <Table>
              <THead><TR>
                <TH>Operation</TH><TH>Module</TH><TH>Exposure</TH><TH>Availability</TH><TH>Permission</TH>
              </TR></THead>
              <TBody>
                {data.items.map((item) => (
                  <TR key={item.operation_key}>
                    <TD className="min-w-64">
                      <div className="flex items-start gap-3">
                        <BrainCircuit className="mt-0.5 size-4 shrink-0 text-[var(--brand-strong)]" />
                        <div className="min-w-0">
                          <p className="font-medium text-[var(--text-strong)]">{item.label}</p>
                          <code className="mt-1 block break-all text-xs text-[var(--text-muted)]">{item.operation_key}</code>
                          {item.availability_reason ? <p className="mt-2 text-xs leading-5 text-[var(--text-muted)]">{item.availability_reason}</p> : null}
                        </div>
                      </div>
                    </TD>
                    <TD><p className="font-medium text-[var(--text-strong)]">{item.module_label}</p><p className="mt-1 text-xs text-[var(--text-muted)]">{statusLabel(item.effect)}</p></TD>
                    <TD><AgentStatus value={item.exposure} /></TD>
                    <TD><AgentStatus value={item.availability} /></TD>
                    <TD><code className="text-xs text-[var(--text-muted)]">{item.permission}</code></TD>
                  </TR>
                ))}
              </TBody>
            </Table>
          </TableScroll>
        </TableWrap>
      ) : null}
    </div>
  );
}

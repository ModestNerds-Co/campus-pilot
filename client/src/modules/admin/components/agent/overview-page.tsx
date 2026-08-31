/**
 * Renders the tenant's current Agent readiness without treating queued work as worker health.
 * Every metric comes from the server and links to the Administration page that owns remediation.
 */

import React, { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Link } from "@tanstack/react-router";
import {
  Activity,
  Bot,
  BrainCircuit,
  Gauge,
  RefreshCw,
  Route,
  ServerCog,
  ShieldCheck,
} from "lucide-react";

import { Button, buttonVariants } from "@/components/ui/button";
import { TableError, TableLoading, TableWrap } from "@/components/ui/data-table";
import { usePageChrome } from "@/modules/admin/layouts/page-chrome";

import { agentGovernanceService, governanceErrorMessage, isGovernanceForbidden } from "./service";
import { AgentMetric, AgentStatus, ForbiddenPanel, formatCount } from "./shared";
import type { AgentReadiness } from "./types";

export function AgentOverviewPage() {
  const [readiness, setReadiness] = useState<AgentReadiness | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [forbidden, setForbidden] = useState(false);
  const generation = useRef(0);

  const load = useCallback(async () => {
    const requestGeneration = ++generation.current;
    setLoading(true);
    setError(null);
    setForbidden(false);
    try {
      const response = await agentGovernanceService.readiness();
      if (requestGeneration !== generation.current) return;
      if (!response.success || !response.data) {
        setForbidden(isGovernanceForbidden(response));
        setError(governanceErrorMessage(response, "Agent readiness could not be loaded."));
        return;
      }
      setReadiness(response.data);
    } catch {
      if (requestGeneration === generation.current) {
        setError("Campus Pilot could not reach Agent administration. Check the connection and try again.");
      }
    } finally {
      if (requestGeneration === generation.current) setLoading(false);
    }
  }, []);

  useEffect(() => {
    void load();
    return () => { generation.current += 1; };
  }, [load]);

  const action = useMemo(() => (
    <Button disabled={loading} onClick={() => void load()} variant="secondary">
      <RefreshCw className={`size-4 ${loading ? "animate-spin" : ""}`} />
      Refresh
    </Button>
  ), [load, loading]);
  usePageChrome("Agent overview", action);

  if (loading && !readiness) {
    return <TableWrap><TableLoading columns={4} label="Loading Agent readiness…" rows={5} /></TableWrap>;
  }
  if (forbidden) return <ForbiddenPanel area="Agent governance" />;
  if (error || !readiness) {
    return <TableWrap><TableError description={error || "Agent readiness could not be loaded."} onRetry={() => void load()} /></TableWrap>;
  }

  const attention = readiness.providers.attention
    + readiness.routing.blocked_targets
    + readiness.runtime.expired_leases
    + (readiness.workers.available ? 0 : 1);

  return (
    <div className="space-y-7">
      <section className="grid gap-5 border-b border-[var(--border)] pb-6 lg:grid-cols-[minmax(0,1fr)_minmax(460px,0.9fr)] lg:items-end">
        <p className="max-w-2xl text-sm leading-6 text-[var(--text-muted)]">
          Current provider, routing, capability, run, and limit state for this campus.
        </p>
        <dl className="grid grid-cols-2 overflow-hidden border border-[var(--border)] bg-[var(--surface)] sm:grid-cols-4">
          <AgentMetric label="Module" value={readiness.module.enabled ? "On" : "Off"} />
          <AgentMetric label="Ready providers" value={formatCount(readiness.providers.ready)} />
          <AgentMetric label="Active runs" value={formatCount(readiness.runtime.active_runs)} />
          <AgentMetric label="Attention" value={formatCount(attention)} />
        </dl>
      </section>

      <div className="grid gap-5 xl:grid-cols-2">
        <ReadinessCard
          action="Manage providers"
          description={`${readiness.providers.ready} ready · ${readiness.providers.attention} need attention`}
          href="/admin/agent/providers"
          icon={Bot}
          status={readiness.providers.ready > 0 ? "ready" : "attention"}
          title="AI providers"
        />
        <ReadinessCard
          action="Manage routing"
          description={`${readiness.routing.route_sets} route sets · ${readiness.routing.blocked_targets} blocked targets`}
          href="/admin/agent/routing"
          icon={Route}
          status={readiness.routing.ready_targets > 0 ? "ready" : "attention"}
          title="Routing"
        />
        <ReadinessCard
          action="Open capabilities"
          description={`${readiness.capabilities.executable_capabilities} executable · ${readiness.capabilities.approval_required} await approval support`}
          href="/admin/agent/capabilities"
          icon={BrainCircuit}
          status={readiness.capabilities.executable_capabilities > 0 ? "ready" : "attention"}
          title="Capabilities"
        />
        <ReadinessCard
          action="Inspect runs"
          description={`${readiness.runtime.queued_runs} queued · ${readiness.runtime.expired_leases} expired leases`}
          href="/admin/agent/runs"
          icon={Activity}
          status={readiness.runtime.expired_leases > 0 ? "attention" : "ready"}
          title="Runtime"
        />
        <ReadinessCard
          description={workerReadinessDescription(readiness)}
          icon={ServerCog}
          status={readiness.workers.available ? "ready" : "attention"}
          title="Execution workers"
        />
        <ReadinessCard
          action="Open usage"
          description={`${readiness.limits.configured_rules} configured limit rules`}
          href="/admin/agent/usage"
          icon={Gauge}
          status={readiness.limits.enforcement_available ? "ready" : "attention"}
          title="Usage and limits"
        />
        <ReadinessCard
          description={`${readiness.capabilities.catalogued_operations} operations classified · ${readiness.capabilities.prohibited} prohibited`}
          icon={ShieldCheck}
          status="ready"
          title="Coverage"
        />
      </div>
    </div>
  );
}

function workerReadinessDescription(readiness: AgentReadiness) {
  if (readiness.workers.available) {
    return `${readiness.workers.ready_instances} worker${readiness.workers.ready_instances === 1 ? "" : "s"} ready`;
  }
  if (readiness.workers.reason === "not_registered") {
    return "No execution worker is registered";
  }
  return `${readiness.workers.registered_instances} registered · no fresh ready heartbeat`;
}

function ReadinessCard({
  action,
  description,
  href,
  icon: Icon,
  status,
  title,
}: {
  action?: string;
  description: string;
  href?: string;
  icon: React.ComponentType<{ className?: string }>;
  status: string;
  title: string;
}) {
  return (
    <section className="flex min-h-40 flex-col border border-[var(--border)] bg-[var(--surface)] p-5 shadow-[var(--shadow-card)]">
      <div className="flex items-start gap-4">
        <span className="flex size-10 shrink-0 items-center justify-center rounded-[9px] bg-[var(--brand-soft)] text-[var(--brand-strong)]">
          <Icon className="size-[18px]" />
        </span>
        <div className="min-w-0 flex-1">
          <div className="flex flex-wrap items-center justify-between gap-2">
            <h2 className="font-semibold text-[var(--text-strong)]">{title}</h2>
            <AgentStatus value={status} />
          </div>
          <p className="mt-2 text-sm leading-6 text-[var(--text-muted)]">{description}</p>
        </div>
      </div>
      {href && action ? (
        <Link className={`${buttonVariants({ variant: "secondary", size: "sm" })} mt-auto self-end`} to={href}>
          {action}
        </Link>
      ) : null}
    </section>
  );
}

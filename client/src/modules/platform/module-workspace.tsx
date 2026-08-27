import React, { useEffect, useState } from "react";
import { Link } from "@tanstack/react-router";
import { ArrowRight, CheckCircle2, CircleDashed } from "lucide-react";

import { ProtectedRoute } from "@/components/protected-route";
import { usePageChrome } from "@/modules/admin/layouts/page-chrome";

import { accessService } from "./access-service";
import { defaultModuleVisual, moduleVisuals, stageLabel } from "./module-registry";
import type { ModuleDefinition } from "./types";
import { TimetableWorkspace } from "@/modules/timetabling";

export const ModuleWorkspace: React.FC<{ moduleKey: string }> = ({ moduleKey }) => {
  const normalizedKey = moduleKey.replace(/-/g, "_");
  const [module, setModule] = useState<ModuleDefinition | null>(null);
  const [isLoading, setIsLoading] = useState(true);

  useEffect(() => {
    let active = true;
    void accessService.getCatalog().then((response) => {
      if (active && response.success && response.data) {
        setModule(response.data.modules.find((item) => item.key === normalizedKey) ?? null);
      }
    }).finally(() => { if (active) setIsLoading(false); });
    return () => { active = false; };
  }, [normalizedKey]);

  if (isLoading) return <div className="h-64 animate-pulse rounded-[var(--radius-xl)] bg-[var(--surface-sunken)]" />;
  if (!module) return <UnknownModule />;

  return (
    <ProtectedRoute requiredModule={module.key} requiredPermission={`${module.permission_namespace}:view`}>
      {module.key === "timetabling" ? <TimetableWorkspace module={module} /> : <ModuleFoundation module={module} />}
    </ProtectedRoute>
  );
};

const ModuleFoundation: React.FC<{ module: ModuleDefinition }> = ({ module }) => {
  const visual = moduleVisuals[module.key] ?? defaultModuleVisual;
  const Icon = visual.icon;
  usePageChrome("Overview");

  if (module.key === "fleet") {
    return (
      <div className="space-y-8">
        <ModuleIntroduction module={module} />
        <section aria-labelledby="fleet-workspaces">
          <p className="text-[11px] font-semibold uppercase tracking-[0.16em] text-[var(--brand-strong)]">Working areas</p>
          <h2 className="mt-1 text-xl font-semibold tracking-[-0.025em] text-[var(--text-strong)]" id="fleet-workspaces">Run daily fleet operations</h2>
          <div className="mt-4 grid gap-4 md:grid-cols-3">
            <FleetLink description="Register and maintain the campus vehicle record." label="Vehicles" to="/modules/fleet/vehicles" />
            <FleetLink description="Manage authorized drivers and licence details." label="Drivers" to="/modules/fleet/drivers" />
            <FleetLink description="Record journeys, mileage, fuel, and daily checks." label="Daily vehicle log" to="/modules/fleet/daily-log" />
          </div>
        </section>
      </div>
    );
  }

  if (module.key === "hr_payroll") {
    return (
      <div className="space-y-8">
        <ModuleIntroduction module={module} />
        <section aria-labelledby="hr-workspaces">
          <p className="text-[11px] font-semibold uppercase tracking-[0.16em] text-[var(--brand-strong)]">Working areas</p>
          <h2 className="mt-1 text-xl font-semibold tracking-[-0.025em] text-[var(--text-strong)]" id="hr-workspaces">Manage the workforce directory</h2>
          <div className="mt-4 grid gap-4 md:grid-cols-3">
            <HrLink description="Maintain the canonical record used across campus modules." label="Employees" to="/modules/hr-payroll/employees" />
            <HrLink description="Organize employees by operational area." label="Departments" to="/modules/hr-payroll/departments" />
            <HrLink description="Maintain the positions employees may hold." label="Positions" to="/modules/hr-payroll/positions" />
          </div>
        </section>
      </div>
    );
  }

  return (
    <div className="space-y-8">
      <ModuleIntroduction module={module} />
      <section aria-labelledby="module-scope">
        <div className="border-b border-[var(--border)] pb-3">
          <p className="text-[11px] font-semibold uppercase tracking-[0.16em] text-[var(--brand-strong)]">Module scope</p>
          <h2 className="mt-1 text-xl font-semibold tracking-[-0.025em] text-[var(--text-strong)]" id="module-scope">Planned areas</h2>
        </div>
        <div className="grid gap-x-8 md:grid-cols-2 xl:grid-cols-3">
          {visual.highlights.map((highlight, index) => (
            <div className="flex items-start gap-4 border-b border-[var(--border-subtle)] py-5" key={highlight}>
              <span className="font-tabular text-xs font-semibold text-[var(--text-subtle)]">{String(index + 1).padStart(2, "0")}</span>
              <span className="text-sm font-medium text-[var(--text-body)]">{highlight}</span>
            </div>
          ))}
        </div>
      </section>
      <section className="flex items-start gap-4 bg-[var(--surface-muted)] p-5">
        <CircleDashed className="mt-0.5 size-5 shrink-0 text-[var(--brand-strong)]" />
        <div>
          <h2 className="text-sm font-semibold text-[var(--text-strong)]">This workspace is not available yet</h2>
          <p className="mt-1 max-w-[55em] text-sm leading-6 text-[var(--text-muted)]">Return to All modules and choose an available workspace.</p>
        </div>
      </section>
    </div>
  );
};

const ModuleIntroduction: React.FC<{ module: ModuleDefinition }> = ({ module }) => {
  const visual = moduleVisuals[module.key] ?? defaultModuleVisual;
  const Icon = visual.icon;
  return (
    <section className="relative overflow-hidden bg-[var(--sidebar)] px-6 py-8 text-[var(--sidebar-foreground)] sm:px-8 sm:py-10">
      <div aria-hidden="true" className="campus-grid-pattern absolute inset-0 opacity-40" />
      <div className="relative flex max-w-3xl items-start gap-4">
        <span className="flex size-11 shrink-0 items-center justify-center rounded-[10px] bg-[var(--brand-highlight)] text-[var(--sidebar-active-fg)]"><Icon className="size-5" /></span>
        <div>
          <div className="flex items-center gap-2 text-[11px] font-semibold uppercase tracking-[0.16em] text-[var(--brand-highlight)]"><CheckCircle2 className="size-3.5" />{stageLabel(module.stage)}</div>
          <h1 className="mt-3 text-2xl font-semibold tracking-[-0.035em] sm:text-3xl">{module.label}</h1>
          <p className="mt-3 max-w-2xl text-sm leading-6 text-[var(--sidebar-muted)]">{module.description}</p>
        </div>
      </div>
    </section>
  );
};

const FleetLink: React.FC<{ description: string; label: string; to: "/modules/fleet/vehicles" | "/modules/fleet/drivers" | "/modules/fleet/daily-log" }> = ({ description, label, to }) => (
  <Link className="group border border-[var(--border)] bg-[var(--surface)] p-5 hover:border-[var(--border-strong)] hover:shadow-[var(--shadow-hover)]" to={to}>
    <span className="flex items-center justify-between gap-4"><span className="font-semibold text-[var(--text-strong)]">{label}</span><ArrowRight className="size-4 text-[var(--text-subtle)] transition-transform group-hover:translate-x-1 group-hover:text-[var(--brand-strong)]" /></span>
    <span className="mt-2 block text-sm leading-5 text-[var(--text-muted)]">{description}</span>
  </Link>
);

const HrLink: React.FC<{ description: string; label: string; to: "/modules/hr-payroll/employees" | "/modules/hr-payroll/departments" | "/modules/hr-payroll/positions" }> = ({ description, label, to }) => (
  <Link className="group border border-[var(--border)] bg-[var(--surface)] p-5 hover:border-[var(--border-strong)] hover:shadow-[var(--shadow-hover)]" to={to}>
    <span className="flex items-center justify-between gap-4"><span className="font-semibold text-[var(--text-strong)]">{label}</span><ArrowRight className="size-4 text-[var(--text-subtle)] transition-transform group-hover:translate-x-1 group-hover:text-[var(--brand-strong)]" /></span>
    <span className="mt-2 block text-sm leading-5 text-[var(--text-muted)]">{description}</span>
  </Link>
);

const UnknownModule = () => {
  usePageChrome("Module not found");
  return <div className="py-16 text-center"><CircleDashed className="mx-auto size-9 text-[var(--text-subtle)]" /><h1 className="mt-4 text-xl font-semibold text-[var(--text-strong)]">This module is not in the Campus Pilot catalog</h1><p className="mt-2 text-sm text-[var(--text-muted)]">Return to All modules and choose an available workspace.</p><Link className="mt-5 inline-flex items-center gap-2 text-sm font-semibold text-[var(--brand-strong)]" to="/home">All modules <ArrowRight className="size-4" /></Link></div>;
};

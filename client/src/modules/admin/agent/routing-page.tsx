/**
 * Administration workspace for ordered, tenant-scoped Agent provider routes.
 * Focused create, edit, and archive workflows use the shared right-side drawer.
 */

import React, { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Link } from "@tanstack/react-router";
import {
  AlertTriangle,
  Archive,
  ArrowDown,
  ArrowUp,
  Bot,
  GitBranch,
  Loader2,
  Pencil,
  Plus,
  ServerOff,
  Trash2,
  Waypoints,
} from "lucide-react";
import toast from "react-hot-toast";

import { SearchableSelect } from "@/components/searchable-select";
import { Badge } from "@/components/ui/badge";
import { Button, buttonVariants } from "@/components/ui/button";
import { DialogBody, DialogFooter, DialogHeader, DialogShell } from "@/components/ui/dialog";
import { Label, Select, Textarea } from "@/components/ui/input";
import { TableError, TableLoading, TableWrap } from "@/components/ui/data-table";
import { cn, formatDate } from "@/lib/utils";
import { usePageChrome } from "@/modules/admin/layouts/page-chrome";
import { useAuthStore } from "@/stores/auth-store";

import { aiProviderErrorMessage } from "./ai-provider-service";
import { aiRoutingService } from "./ai-routing-service";
import type {
  AiOperationClass,
  AiRoutingCapabilityOption,
  AiRoutingModuleOption,
  AiRouteScopeKind,
  AiTaskClass,
  AiTaskRoute,
  AiTaskRouteScope,
  AiTaskRouteTarget,
  AiTaskRouteTargetInput,
} from "./types";

type RouteDrawer =
  | { kind: "create" }
  | { kind: "edit" | "archive"; route: AiTaskRoute }
  | null;

type ProviderModelChoice = {
  key: string;
  connection_id: string;
  account_label: string;
  provider_label: string;
  provider_model_id: string;
  model_display_name: string;
  supports_tools: boolean | null;
};

const scopeOrder: Array<{ kind: AiRouteScopeKind; label: string; shortLabel: string }> = [
  { kind: "capability", label: "Capability override", shortLabel: "Capability" },
  { kind: "module_operation", label: "Module and operation", shortLabel: "Module + effect" },
  { kind: "task_class", label: "Task class", shortLabel: "Task class" },
  { kind: "tenant_default", label: "Campus default", shortLabel: "Default" },
];

const taskClasses: Array<{ value: AiTaskClass; label: string }> = [
  { value: "campus_conversation_search", label: "Campus conversation and search" },
  { value: "module_read_reporting", label: "Module reading and reporting" },
  { value: "document_extraction", label: "Document extraction" },
  { value: "drafting_proposal", label: "Drafting and proposals" },
  { value: "approved_operational_action", label: "Approved operational actions" },
];

const operationClasses: Array<{ value: AiOperationClass; label: string }> = [
  { value: "read", label: "Read" },
  { value: "propose", label: "Propose" },
  { value: "mutate", label: "Change records" },
  { value: "external_side_effect", label: "External side effect" },
];

export function AiRoutingPage() {
  const user = useAuthStore((state) => state.user);
  const canEdit = hasPermission(user?.permissions, "ai_routing:edit");
  const canViewProviders = hasPermission(user?.permissions, "ai_providers:view");
  const [routes, setRoutes] = useState<AiTaskRoute[]>([]);
  const [modules, setModules] = useState<AiRoutingModuleOption[]>([]);
  const [capabilities, setCapabilities] = useState<AiRoutingCapabilityOption[]>([]);
  const [providerChoices, setProviderChoices] = useState<ProviderModelChoice[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [choiceError, setChoiceError] = useState<string | null>(null);
  const [drawer, setDrawer] = useState<RouteDrawer>(null);
  const loadGeneration = useRef(0);

  const load = useCallback(async (quiet = false) => {
    const generation = ++loadGeneration.current;
    if (!quiet) setIsLoading(true);
    setLoadError(null);
    setChoiceError(null);

    try {
      const [routeResponse, optionsResponse] = await Promise.all([
        aiRoutingService.listRoutes(),
        aiRoutingService.listOptions().catch(() => null),
      ]);
      if (generation !== loadGeneration.current) return;

      if (!routeResponse.success || !routeResponse.data) {
        setLoadError(aiProviderErrorMessage(routeResponse, "Agent routes could not be loaded."));
        return;
      }
      setRoutes(routeResponse.data);

      if (!optionsResponse?.success || !optionsResponse.data) {
        setChoiceError(aiProviderErrorMessage(
          optionsResponse ?? { issues: null, message: null },
          "Routing choices could not be loaded.",
        ));
        setModules([]);
        setCapabilities([]);
        setProviderChoices([]);
        return;
      }
      setModules(optionsResponse.data.modules);
      setCapabilities(optionsResponse.data.capabilities);
      setProviderChoices(optionsResponse.data.targets.map((target) => ({
        key: choiceKey(target.connection_id, target.provider_model_id),
        connection_id: target.connection_id,
        account_label: target.account_label,
        provider_label: target.provider_label,
        provider_model_id: target.provider_model_id,
        model_display_name: target.model_display_name,
        supports_tools: target.supports_tools,
      })));
    } catch {
      if (generation === loadGeneration.current) {
        setLoadError("Campus Pilot could not reach Agent routing. Check the connection and try again.");
      }
    } finally {
      if (generation === loadGeneration.current && !quiet) setIsLoading(false);
    }
  }, []);

  useEffect(() => {
    void load();
    return () => {
      loadGeneration.current += 1;
    };
  }, [load]);

  const pageAction = useMemo(() => canEdit ? (
    <Button
      disabled={isLoading || Boolean(loadError) || providerChoices.length === 0}
      onClick={() => setDrawer({ kind: "create" })}
      title={providerChoices.length === 0 ? "A ready provider with cached models is required" : undefined}
    >
      <Plus className="size-4" />
      Add route
    </Button>
  ) : null, [canEdit, isLoading, loadError, providerChoices.length]);
  usePageChrome("Routing", pageAction);

  const targetCount = routes.reduce((total, route) => total + route.targets.length, 0);
  const scopeCount = new Set(routes.map((route) => route.scope.scope_kind)).size;

  const upsertRoute = (route: AiTaskRoute) => {
    setRoutes((current) => sortRoutes(
      current.some((item) => item.id === route.id)
        ? current.map((item) => item.id === route.id ? route : item)
        : [...current, route],
    ));
    setDrawer(null);
  };

  return (
    <div className="space-y-7">
      <section className="grid gap-5 border-b border-[var(--border)] pb-6 lg:grid-cols-[minmax(0,1fr)_minmax(420px,0.8fr)] lg:items-end">
        <p className="max-w-2xl text-sm leading-6 text-[var(--text-muted)]">
          Choose the provider and model order Agent uses for each kind of work. Only ready connections with cached models can be added.
        </p>
        {!isLoading && !loadError ? (
          <dl className="grid grid-cols-3 border border-[var(--border)] bg-[var(--surface)]">
            <Metric label="Routes" value={routes.length} />
            <Metric label="Targets" value={targetCount} />
            <Metric label="Scopes" value={`${scopeCount}/4`} />
          </dl>
        ) : null}
      </section>

      <section aria-labelledby="route-precedence-title">
        <div className="mb-3">
          <p className="text-[11px] font-semibold uppercase tracking-[0.16em] text-[var(--text-muted)]">Selection order</p>
          <h2 className="mt-1 text-lg font-semibold tracking-[-0.025em] text-[var(--text-strong)]" id="route-precedence-title">Route precedence</h2>
        </div>
        <ol className="grid border border-[var(--border)] bg-[var(--surface)] sm:grid-cols-2 lg:grid-cols-4">
          {scopeOrder.map((scope, index) => (
            <li className="flex items-center gap-3 border-b border-[var(--border-subtle)] px-4 py-3 last:border-b-0 sm:border-r sm:[&:nth-child(2)]:border-r-0 lg:border-b-0 lg:[&:nth-child(2)]:border-r" key={scope.kind}>
              <span className="flex size-7 shrink-0 items-center justify-center rounded-full bg-[var(--brand-soft)] text-xs font-semibold text-[var(--brand-strong)]">{index + 1}</span>
              <span className="text-sm font-medium text-[var(--text-strong)]">{scope.shortLabel}</span>
            </li>
          ))}
        </ol>
        <p className="mt-2 text-xs leading-5 text-[var(--text-muted)]">Agent uses the first matching route, then tries its targets in their numbered order.</p>
      </section>

      {choiceError ? (
        <div className="flex flex-wrap items-start justify-between gap-4 border border-[var(--tone-warn-bd)] bg-[var(--tone-warn-bg)] p-4" role="status">
          <div className="flex min-w-0 items-start gap-3 text-sm leading-6 text-[var(--tone-warn-strong)]">
            <AlertTriangle className="mt-1 size-4 shrink-0" />
            <span>{choiceError}</span>
          </div>
          {canViewProviders ? (
            <Link className={cn(buttonVariants({ variant: "secondary", size: "sm" }), "shrink-0")} to="/admin/agent/providers">Open AI providers</Link>
          ) : null}
        </div>
      ) : null}

      {isLoading ? (
        <TableWrap>
          <TableLoading columns={4} label="Loading Agent routes…" rows={4} />
        </TableWrap>
      ) : loadError ? (
        <TableWrap>
          <TableError description={loadError} onRetry={() => void load()} title="Agent routes could not be loaded" />
        </TableWrap>
      ) : routes.length === 0 ? (
        <RoutingEmptyState
          canEdit={canEdit}
          canViewProviders={canViewProviders}
          hasProviderChoices={providerChoices.length > 0}
          onCreate={() => setDrawer({ kind: "create" })}
        />
      ) : (
        <section aria-labelledby="configured-routes-title">
          <div className="mb-3">
            <p className="text-[11px] font-semibold uppercase tracking-[0.16em] text-[var(--text-muted)]">Current configuration</p>
            <h2 className="mt-1 text-xl font-semibold tracking-[-0.025em] text-[var(--text-strong)]" id="configured-routes-title">Configured routes</h2>
          </div>
          <ul className="divide-y divide-[var(--border)] border border-[var(--border)] bg-[var(--surface)] shadow-[var(--shadow-card)]">
            {sortRoutes(routes).map((route) => (
              <RouteRow
                canEdit={canEdit}
                key={route.id}
                onArchive={() => setDrawer({ kind: "archive", route })}
                onEdit={() => setDrawer({ kind: "edit", route })}
                route={route}
                scopeDisplayLabel={scopeLabel(route.scope, capabilities, modules)}
              />
            ))}
          </ul>
        </section>
      )}

      <RouteWorkflowDrawer
        capabilities={capabilities}
        drawer={drawer}
        key={drawerKey(drawer)}
        modules={modules}
        onArchived={(routeId) => {
          setRoutes((current) => current.filter((route) => route.id !== routeId));
          setDrawer(null);
        }}
        onClose={() => setDrawer(null)}
        onSaved={upsertRoute}
        providerChoices={providerChoices}
      />
    </div>
  );
}

function RouteRow({
  canEdit,
  onArchive,
  onEdit,
  route,
  scopeDisplayLabel,
}: {
  canEdit: boolean;
  onArchive: () => void;
  onEdit: () => void;
  route: AiTaskRoute;
  scopeDisplayLabel: string;
}) {
  const label = scopeDisplayLabel;
  return (
    <li className="grid gap-5 px-5 py-5 lg:grid-cols-[minmax(220px,0.75fr)_minmax(0,1.55fr)_auto] lg:items-start">
      <div className="min-w-0">
        <div className="flex items-start gap-3">
          <span className="flex size-10 shrink-0 items-center justify-center rounded-[9px] bg-[var(--brand-soft)] text-[var(--brand-strong)]"><Waypoints className="size-[18px]" /></span>
          <div className="min-w-0">
            <p className="truncate font-semibold text-[var(--text-strong)]">{label}</p>
            <div className="mt-2 flex flex-wrap gap-2">
              <Badge tone="info">Precedence {scopePrecedence(route.scope.scope_kind)}</Badge>
              {route.requires_tools ? <Badge tone="brand">Tools required</Badge> : <Badge tone="neutral">Tools optional</Badge>}
            </div>
          </div>
        </div>
        <p className="mt-3 text-xs text-[var(--text-subtle)] lg:ml-[52px]">Updated {formatDate(route.updated_at)}</p>
      </div>

      <ol className="space-y-2" aria-label={`${label} fallback order`}>
        {route.targets.map((target, index) => (
          <li className="flex min-w-0 items-center gap-3 border border-[var(--border-subtle)] bg-[var(--surface-muted)] px-3 py-2.5" key={`${target.connection_id}-${target.provider_model_id}`}>
            <span className="flex size-6 shrink-0 items-center justify-center rounded-full bg-[var(--surface)] text-[11px] font-semibold text-[var(--brand-strong)]">{index + 1}</span>
            <div className="min-w-0 flex-1">
              <p className="truncate text-sm font-semibold text-[var(--text-strong)]">{target.model_display_name || target.provider_model_id}</p>
              <p className="mt-0.5 truncate text-xs text-[var(--text-muted)]">{target.account_label} · {humanize(target.provider)}</p>
            </div>
            <Badge tone={target.readiness === "ready" ? "success" : "warn"}>{readinessLabel(target.readiness)}</Badge>
          </li>
        ))}
      </ol>

      {canEdit ? (
        <div className="flex gap-2 lg:justify-end">
          <Button aria-label={`Edit ${label} route`} onClick={onEdit} size="icon-sm" variant="secondary"><Pencil className="size-3.5" /></Button>
          <Button aria-label={`Archive ${label} route`} onClick={onArchive} size="icon-sm" variant="ghost"><Archive className="size-3.5 text-[var(--tone-danger)]" /></Button>
        </div>
      ) : null}
    </li>
  );
}

function RoutingEmptyState({
  canEdit,
  canViewProviders,
  hasProviderChoices,
  onCreate,
}: {
  canEdit: boolean;
  canViewProviders: boolean;
  hasProviderChoices: boolean;
  onCreate: () => void;
}) {
  const ready = hasProviderChoices;
  return (
    <section aria-labelledby="routes-empty-title" className="border border-dashed border-[var(--border-strong)] bg-[var(--surface)] px-5 py-10 sm:px-8">
      <div className="flex max-w-2xl flex-col items-start gap-5 sm:flex-row">
        <span className="flex size-11 shrink-0 items-center justify-center rounded-[10px] bg-[var(--brand-soft)] text-[var(--brand-strong)]">
          {ready ? <GitBranch className="size-5" /> : <ServerOff className="size-5" />}
        </span>
        <div>
          <h2 className="font-semibold text-[var(--text-strong)]" id="routes-empty-title">{ready ? "No Agent routes" : "Provider setup required"}</h2>
          <p className="mt-1 text-sm leading-6 text-[var(--text-muted)]">
            {ready
              ? "Add a campus default or a more specific route before Agent can select a model."
              : "A provider connection must be ready and have cached models before a route can be added."}
          </p>
          <div className="mt-4 flex flex-wrap gap-3">
            {canEdit && ready ? <Button onClick={onCreate}><Plus className="size-4" />Add route</Button> : null}
            {canViewProviders && !ready ? (
              <Link className={buttonVariants({ variant: "secondary" })} to="/admin/agent/providers"><Bot className="size-4" />Open AI providers</Link>
            ) : null}
          </div>
        </div>
      </div>
    </section>
  );
}

type DraftTarget = {
  key: string;
  choice_key: string | null;
};

function RouteWorkflowDrawer({
  capabilities,
  drawer,
  modules,
  onArchived,
  onClose,
  onSaved,
  providerChoices,
}: {
  capabilities: AiRoutingCapabilityOption[];
  drawer: RouteDrawer;
  modules: AiRoutingModuleOption[];
  onArchived: (routeId: string) => void;
  onClose: () => void;
  onSaved: (route: AiTaskRoute) => void;
  providerChoices: ProviderModelChoice[];
}) {
  const route = drawer && "route" in drawer ? drawer.route : null;
  const routeDisplayLabel = route ? scopeLabel(route.scope, capabilities, modules) : null;
  const [scopeKind, setScopeKind] = useState<AiRouteScopeKind>(route?.scope.scope_kind ?? "tenant_default");
  const [taskClass, setTaskClass] = useState<AiTaskClass>(
    route?.scope.scope_kind === "task_class" ? route.scope.task_class : "campus_conversation_search",
  );
  const [moduleKey, setModuleKey] = useState<string | null>(
    route?.scope.scope_kind === "module_operation" ? route.scope.module_key : null,
  );
  const [operationClass, setOperationClass] = useState<AiOperationClass>(
    route?.scope.scope_kind === "module_operation" ? route.scope.operation_class : "read",
  );
  const [capabilityChoice, setCapabilityChoice] = useState<string | null>(
    route?.scope.scope_kind === "capability"
      ? capabilityChoiceKey(route.scope.capability_key, route.scope.capability_version)
      : null,
  );
  const [requiresTools, setRequiresTools] = useState(route?.requires_tools ?? true);
  const [targets, setTargets] = useState<DraftTarget[]>(
    route?.targets.map(targetToDraft) ?? [emptyTarget()],
  );
  const [auditReason, setAuditReason] = useState("");
  const [busy, setBusy] = useState(false);
  const [formError, setFormError] = useState<string | null>(null);

  if (!drawer) return null;
  const close = busy ? () => undefined : onClose;

  if (drawer.kind === "archive" && route) {
    const archive = () => {
      if (!auditReason.trim()) {
        setFormError("Enter the reason this route is being archived.");
        return;
      }
      setBusy(true);
      setFormError(null);
      void aiRoutingService.archiveRoute(route.id, route.version, auditReason.trim())
        .then((response) => {
          if (!response.success || !response.data) {
            setFormError(aiProviderErrorMessage(response, "The route could not be archived."));
            return;
          }
          toast.success("Route archived");
          onArchived(route.id);
        })
        .catch(() => setFormError("Campus Pilot could not reach Agent routing. Try again."))
        .finally(() => setBusy(false));
    };

    return (
      <DialogShell onClose={close} open>
        <DialogHeader onClose={busy ? undefined : onClose} title="Archive route" />
        <DialogBody className="space-y-6">
          <RouteIdentity label={routeDisplayLabel ?? scopeLabel(route.scope)} route={route} />
          <DrawerNotice danger icon={<Archive className="size-5" />} text="This route will stop taking part in model selection. Existing run records remain available." />
          <Field label="Reason" labelFor="archive-route-reason">
            <Textarea id="archive-route-reason" onChange={(event) => setAuditReason(event.target.value)} placeholder="Why is this route no longer needed?" required rows={4} value={auditReason} />
          </Field>
          <FormError message={formError} />
        </DialogBody>
        <DialogFooter>
          <Button data-autofocus="true" disabled={busy} onClick={onClose} type="button" variant="secondary">Keep route</Button>
          <Button disabled={busy || !auditReason.trim()} onClick={archive} type="button" variant="destructive">
            {busy ? <Loader2 className="size-4 animate-spin" /> : <Archive className="size-4" />}
            {busy ? "Archiving…" : "Archive route"}
          </Button>
        </DialogFooter>
      </DialogShell>
    );
  }

  const allChoices = mergeExistingChoices(providerChoices, route?.targets ?? []);
  const eligibleConnectionCount = new Set(
    allChoices
      .filter((choice) => !requiresTools || choice.supports_tools === true)
      .map((choice) => choice.connection_id),
  ).size;
  const submit = (event: React.FormEvent) => {
    event.preventDefault();
    const selectedCapability = capabilities.find(
      (capability) => capabilityChoiceKey(capability.capability_key, capability.capability_version) === capabilityChoice,
    );
    const scope = route?.scope ?? buildScope({ moduleKey, operationClass, scopeKind, selectedCapability, taskClass });
    if (!scope) {
      setFormError(scopeKind === "module_operation" ? "Choose a module." : "Choose a registered capability.");
      return;
    }
    const parsedTargets = buildTargets(targets, allChoices, requiresTools);
    if (typeof parsedTargets === "string") {
      setFormError(parsedTargets);
      return;
    }
    if (!auditReason.trim()) {
      setFormError("Enter the reason for this routing change.");
      return;
    }

    setBusy(true);
    setFormError(null);
    const request = route
      ? aiRoutingService.updateRoute(route.id, {
        expected_version: route.version,
        requires_tools: requiresTools,
        targets: parsedTargets,
        audit_reason: auditReason.trim(),
      })
      : aiRoutingService.createRoute({
        ...scope,
        requires_tools: requiresTools,
        targets: parsedTargets,
        audit_reason: auditReason.trim(),
      });
    void request
      .then((response) => {
        if (!response.success || !response.data) {
          setFormError(aiProviderErrorMessage(response, `The route could not be ${route ? "updated" : "created"}.`));
          return;
        }
        toast.success(route ? "Route updated" : "Route created");
        onSaved(response.data);
      })
      .catch(() => setFormError("Campus Pilot could not reach Agent routing. Try again."))
      .finally(() => setBusy(false));
  };

  return (
    <DialogShell onClose={close} open panelClassName="sm:max-w-[720px]">
      <DialogHeader onClose={busy ? undefined : onClose} title={route ? "Edit route" : "Add route"} />
      <form onSubmit={submit}>
        <DialogBody className="space-y-7">
          {route ? <RouteIdentity label={routeDisplayLabel ?? scopeLabel(route.scope)} route={route} /> : <DrawerNotice icon={<GitBranch className="size-5" />} text="The most specific matching scope wins. Targets are tried from first to last when fallback is allowed." />}

          <section className="space-y-5" aria-labelledby="route-details-title">
            <h3 className="border-b border-[var(--border)] pb-2 text-sm font-semibold text-[var(--text-strong)]" id="route-details-title">Route details</h3>
            {!route ? (
              <>
                <Field label="Scope" labelFor="route-scope">
                  <Select data-autofocus="true" id="route-scope" onChange={(event) => setScopeKind(event.target.value as AiRouteScopeKind)} value={scopeKind}>
                    <option value="tenant_default">Campus default</option>
                    <option value="task_class">Task class</option>
                    <option value="module_operation">Module and operation</option>
                    <option value="capability">Capability</option>
                  </Select>
                  <FieldHint>Precedence {scopePrecedence(scopeKind)} of 4. Archive and recreate a route to change its scope.</FieldHint>
                </Field>

                {scopeKind === "task_class" ? (
                  <Field label="Task class" labelFor="route-task-class">
                    <Select id="route-task-class" onChange={(event) => setTaskClass(event.target.value as AiTaskClass)} value={taskClass}>
                      {taskClasses.map((item) => <option key={item.value} value={item.value}>{item.label}</option>)}
                    </Select>
                  </Field>
                ) : null}

                {scopeKind === "module_operation" ? (
                  <div className="grid gap-5 sm:grid-cols-2">
                    <Field label="Module" labelFor="route-module">
                      <SearchableSelect
                        allowClear={false}
                        id="route-module"
                        onChange={setModuleKey}
                        options={modules.map((module) => ({ id: module.module_key, value: module.label, label: module.label }))}
                        placeholder="Choose module"
                        value={moduleKey}
                      />
                    </Field>
                    <Field label="Operation" labelFor="route-operation-class">
                      <Select id="route-operation-class" onChange={(event) => setOperationClass(event.target.value as AiOperationClass)} value={operationClass}>
                        {operationClasses.map((item) => <option key={item.value} value={item.value}>{item.label}</option>)}
                      </Select>
                    </Field>
                  </div>
                ) : null}

                {scopeKind === "capability" ? (
                  <Field label="Capability" labelFor="route-capability">
                    <SearchableSelect
                      allowClear={false}
                      id="route-capability"
                      onChange={setCapabilityChoice}
                      options={capabilities.map((capability) => {
                        const module = modules.find((item) => item.module_key === capability.module_key);
                        const operation = operationClasses.find((item) => item.value === capability.operation_class)?.label ?? humanize(capability.operation_class);
                        return {
                          id: capabilityChoiceKey(capability.capability_key, capability.capability_version),
                          value: capability.label,
                          label: `${module?.label ?? humanize(capability.module_key)} · ${operation}`,
                        };
                      })}
                      placeholder="Choose registered capability"
                      value={capabilityChoice}
                    />
                    {capabilities.length === 0 ? <FieldHint>No registered capabilities are available for routing.</FieldHint> : null}
                  </Field>
                ) : null}
              </>
            ) : null}

            <label className="flex cursor-pointer items-start gap-3 border border-[var(--border)] bg-[var(--surface-muted)] p-4" htmlFor="route-requires-tools">
              <input
                checked={requiresTools}
                className="mt-0.5 size-4 rounded border-[var(--border-strong)] accent-[var(--brand)] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--focus-ring)]"
                id="route-requires-tools"
                onChange={(event) => setRequiresTools(event.target.checked)}
                type="checkbox"
              />
              <span>
                <span className="block text-sm font-semibold text-[var(--text-strong)]">Require tool-capable models</span>
                <span className="mt-1 block text-xs leading-5 text-[var(--text-muted)]">Only models confirmed to support Agent capabilities can be selected.</span>
              </span>
            </label>
          </section>

          <section aria-labelledby="fallback-targets-title">
            <div className="flex flex-wrap items-end justify-between gap-3 border-b border-[var(--border)] pb-2">
              <div>
                <h3 className="text-sm font-semibold text-[var(--text-strong)]" id="fallback-targets-title">Provider fallback</h3>
                <p className="mt-1 text-xs leading-5 text-[var(--text-muted)]">Move targets to set the order Agent tries them.</p>
              </div>
              <Button disabled={targets.length >= 3 || targets.length >= eligibleConnectionCount} onClick={() => setTargets((current) => [...current, emptyTarget()])} size="sm" type="button" variant="secondary"><Plus className="size-3.5" />Add target</Button>
            </div>
            <ol className="mt-4 space-y-4">
              {targets.map((target, index) => {
                const selected = target.choice_key ? allChoices.find((choice) => choice.key === target.choice_key) : undefined;
                const selectedConnectionIds = new Set(
                  targets
                    .filter((item) => item.key !== target.key)
                    .map((item) => allChoices.find((choice) => choice.key === item.choice_key)?.connection_id)
                    .filter((connectionId): connectionId is string => Boolean(connectionId)),
                );
                const available = allChoices.filter((choice) =>
                  !selectedConnectionIds.has(choice.connection_id) &&
                  (choice.key === target.choice_key || !requiresTools || choice.supports_tools === true),
                );
                return (
                  <li className="border border-[var(--border)] bg-[var(--surface-muted)] p-4" key={target.key}>
                    <div className="mb-4 flex items-center gap-3">
                      <span className="flex size-7 shrink-0 items-center justify-center rounded-full bg-[var(--brand-soft)] text-xs font-semibold text-[var(--brand-strong)]">{index + 1}</span>
                      <div className="min-w-0 flex-1">
                        <p className="truncate text-sm font-semibold text-[var(--text-strong)]">{selected?.model_display_name || `Target ${index + 1}`}</p>
                        {selected ? <p className="truncate text-xs text-[var(--text-muted)]">{selected.account_label} · {selected.provider_label}</p> : null}
                      </div>
                      <div className="flex gap-1">
                        <Button aria-label={`Move target ${index + 1} up`} disabled={index === 0} onClick={() => setTargets((current) => moveItem(current, index, index - 1))} size="icon-sm" type="button" variant="ghost"><ArrowUp className="size-3.5" /></Button>
                        <Button aria-label={`Move target ${index + 1} down`} disabled={index === targets.length - 1} onClick={() => setTargets((current) => moveItem(current, index, index + 1))} size="icon-sm" type="button" variant="ghost"><ArrowDown className="size-3.5" /></Button>
                        <Button aria-label={`Remove target ${index + 1}`} disabled={targets.length === 1} onClick={() => setTargets((current) => current.filter((item) => item.key !== target.key))} size="icon-sm" type="button" variant="ghost"><Trash2 className="size-3.5 text-[var(--tone-danger)]" /></Button>
                      </div>
                    </div>
                    <div className="space-y-4">
                      <Field label="Connection and model" labelFor={`route-target-${target.key}`}>
                        <SearchableSelect
                          allowClear={false}
                          id={`route-target-${target.key}`}
                          onChange={(value) => updateTarget(setTargets, target.key, { choice_key: value })}
                          options={available.map((choice) => ({
                            id: choice.key,
                            value: choice.model_display_name,
                            label: `${choice.account_label} · ${choice.provider_label}`,
                            description: choice.provider_model_id,
                          }))}
                          placeholder="Choose ready model"
                          value={target.choice_key}
                        />
                      </Field>
                      {available.length === 0 ? <FieldHint>No additional ready model is available from another connection.</FieldHint> : null}
                      {selected?.supports_tools === false ? <FieldHint>This model does not support Agent tools.</FieldHint> : null}
                      {selected?.supports_tools === null ? <FieldHint>Tool support is unknown for this model.</FieldHint> : null}
                    </div>
                  </li>
                );
              })}
            </ol>
          </section>

          <Field label="Reason for change" labelFor="route-audit-reason">
            <Textarea id="route-audit-reason" onChange={(event) => setAuditReason(event.target.value)} placeholder="What should this routing change achieve?" required rows={4} value={auditReason} />
          </Field>
          <FormError message={formError} />
        </DialogBody>
        <DialogFooter>
          <Button disabled={busy} onClick={onClose} type="button" variant="secondary">Cancel</Button>
          <Button disabled={busy || !auditReason.trim()} type="submit">
            {busy ? <Loader2 className="size-4 animate-spin" /> : <GitBranch className="size-4" />}
            {busy ? "Saving…" : route ? "Save changes" : "Add route"}
          </Button>
        </DialogFooter>
      </form>
    </DialogShell>
  );
}

function Metric({ label, value }: { label: string; value: number | string }) {
  return <div className="border-r border-[var(--border-subtle)] px-4 py-3 last:border-r-0"><dt className="text-[10px] font-semibold uppercase tracking-[0.12em] text-[var(--text-muted)]">{label}</dt><dd className="mt-1 text-lg font-semibold text-[var(--text-strong)]">{value}</dd></div>;
}

function Field({ children, label, labelFor }: { children: React.ReactNode; label: string; labelFor: string }) {
  return <div><Label htmlFor={labelFor}>{label}</Label><div className="mt-2">{children}</div></div>;
}

function FieldHint({ children }: { children: React.ReactNode }) {
  return <p className="mt-2 text-xs leading-5 text-[var(--text-muted)]">{children}</p>;
}

function DrawerNotice({ danger = false, icon, text }: { danger?: boolean; icon: React.ReactNode; text: string }) {
  return <div className={`flex items-start gap-3 border p-4 ${danger ? "border-[var(--tone-danger-bd)] bg-[var(--tone-danger-bg)]" : "border-[var(--brand-100)] bg-[var(--brand-soft)]"}`}><span className={`mt-0.5 shrink-0 ${danger ? "text-[var(--tone-danger-strong)]" : "text-[var(--brand-strong)]"}`}>{icon}</span><p className={`text-sm leading-6 ${danger ? "text-[var(--tone-danger-strong)]" : "text-[var(--text-body)]"}`}>{text}</p></div>;
}

function RouteIdentity({ label, route }: { label: string; route: AiTaskRoute }) {
  return <div className="flex items-start gap-3 border-b border-[var(--border)] pb-5"><span className="flex size-10 shrink-0 items-center justify-center rounded-[9px] bg-[var(--brand-soft)] text-[var(--brand-strong)]"><Waypoints className="size-[18px]" /></span><div className="min-w-0"><p className="truncate font-semibold text-[var(--text-strong)]">{label}</p><p className="mt-0.5 text-sm text-[var(--text-muted)]">{route.requires_tools ? "Tool-capable models required" : "Tool support optional"}</p></div><Badge className="ml-auto" tone="info">Precedence {scopePrecedence(route.scope.scope_kind)}</Badge></div>;
}

function FormError({ message }: { message: string | null }) {
  return message ? <div className="flex items-start gap-3 border border-[var(--tone-danger-bd)] bg-[var(--tone-danger-bg)] p-4 text-sm leading-5 text-[var(--tone-danger-strong)]" role="alert"><AlertTriangle className="mt-0.5 size-4 shrink-0" /><span>{message}</span></div> : <div aria-live="polite" className="sr-only" />;
}

function buildScope({
  moduleKey,
  operationClass,
  scopeKind,
  selectedCapability,
  taskClass,
}: {
  moduleKey: string | null;
  operationClass: AiOperationClass;
  scopeKind: AiRouteScopeKind;
  selectedCapability: AiRoutingCapabilityOption | undefined;
  taskClass: AiTaskClass;
}): AiTaskRouteScope | null {
  if (scopeKind === "tenant_default") return { scope_kind: "tenant_default" };
  if (scopeKind === "task_class") return { scope_kind: "task_class", task_class: taskClass };
  if (scopeKind === "module_operation") {
    return moduleKey ? { scope_kind: "module_operation", module_key: moduleKey, operation_class: operationClass } : null;
  }
  return selectedCapability ? {
    scope_kind: "capability",
    capability_key: selectedCapability.capability_key,
    capability_version: selectedCapability.capability_version,
  } : null;
}

function buildTargets(targets: DraftTarget[], choices: ProviderModelChoice[], requiresTools: boolean): AiTaskRouteTargetInput[] | string {
  if (targets.length === 0) return "Add at least one provider target.";
  if (targets.length > 3) return "A route can contain at most three provider targets.";
  const seenConnections = new Set<string>();
  const result: AiTaskRouteTargetInput[] = [];
  for (const target of targets) {
    if (!target.choice_key) return "Choose a connection and model for every target.";
    const choice = choices.find((item) => item.key === target.choice_key);
    if (!choice) return "One of the selected provider models is no longer available. Reload the page and choose another.";
    if (seenConnections.has(choice.connection_id)) return "Each provider connection can appear only once in a route.";
    seenConnections.add(choice.connection_id);
    if (requiresTools && choice.supports_tools !== true) return "Every selected model must have confirmed tool support for this route.";
    result.push({
      connection_id: choice.connection_id,
      provider_model_id: choice.provider_model_id,
    });
  }
  return result;
}

function emptyTarget(): DraftTarget {
  return {
    key: crypto.randomUUID(),
    choice_key: null,
  };
}

function targetToDraft(target: AiTaskRouteTarget): DraftTarget {
  return {
    key: crypto.randomUUID(),
    choice_key: choiceKey(target.connection_id, target.provider_model_id),
  };
}

function mergeExistingChoices(choices: ProviderModelChoice[], targets: AiTaskRouteTarget[]) {
  const merged = new Map(choices.map((choice) => [choice.key, choice]));
  for (const target of targets) {
    const key = choiceKey(target.connection_id, target.provider_model_id);
    if (!merged.has(key)) {
      merged.set(key, {
        key,
        connection_id: target.connection_id,
        account_label: target.account_label,
        provider_label: humanize(target.provider),
        provider_model_id: target.provider_model_id,
        model_display_name: target.model_display_name,
        supports_tools: target.supports_tools,
      });
    }
  }
  return [...merged.values()];
}

function updateTarget(
  setTargets: React.Dispatch<React.SetStateAction<DraftTarget[]>>,
  targetKey: string,
  update: Partial<DraftTarget>,
) {
  setTargets((current) => current.map((target) => target.key === targetKey ? { ...target, ...update } : target));
}

function moveItem<T>(items: T[], from: number, to: number) {
  if (to < 0 || to >= items.length) return items;
  const next = [...items];
  const [item] = next.splice(from, 1);
  next.splice(to, 0, item);
  return next;
}

function choiceKey(connectionId: string, modelId: string) {
  return `${connectionId}\u001f${modelId}`;
}

function capabilityChoiceKey(capabilityKey: string, capabilityVersion: number) {
  return `${capabilityKey}\u001f${capabilityVersion}`;
}

function scopeLabel(
  scope: AiTaskRouteScope,
  capabilities: AiRoutingCapabilityOption[] = [],
  modules: AiRoutingModuleOption[] = [],
) {
  if (scope.scope_kind === "tenant_default") return "Campus default";
  if (scope.scope_kind === "task_class") return taskClasses.find((item) => item.value === scope.task_class)?.label ?? scope.task_class;
  if (scope.scope_kind === "module_operation") {
    const operation = operationClasses.find((item) => item.value === scope.operation_class)?.label ?? scope.operation_class;
    const module = modules.find((item) => item.module_key === scope.module_key);
    return `${module?.label ?? humanize(scope.module_key)} · ${operation}`;
  }
  const capability = capabilities.find(
    (item) => item.capability_key === scope.capability_key && item.capability_version === scope.capability_version,
  );
  return capability ? capability.label : `${scope.capability_key} · v${scope.capability_version}`;
}

function scopePrecedence(kind: AiRouteScopeKind) {
  return scopeOrder.findIndex((item) => item.kind === kind) + 1;
}

function readinessLabel(value: AiTaskRouteTarget["readiness"]) {
  if (value === "ready") return "Ready";
  if (value === "connection_unavailable") return "Connection unavailable";
  if (value === "stale_model") return "Model changed";
  return "Tools unavailable";
}

function humanize(value: string) {
  return value.replace(/_/g, " ").replace(/^./, (character) => character.toUpperCase());
}

function sortRoutes(routes: AiTaskRoute[]) {
  return [...routes].sort((left, right) => {
    const precedence = scopePrecedence(left.scope.scope_kind) - scopePrecedence(right.scope.scope_kind);
    return precedence || scopeLabel(left.scope).localeCompare(scopeLabel(right.scope));
  });
}

function drawerKey(drawer: RouteDrawer) {
  if (!drawer) return "closed";
  return "route" in drawer ? `${drawer.kind}-${drawer.route.id}-${drawer.route.version}` : drawer.kind;
}

function hasPermission(permissions: string[] | undefined, permission: string) {
  return permissions?.includes("*") || permissions?.includes(permission) || false;
}

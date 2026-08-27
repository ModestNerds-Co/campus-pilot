// campus-pilot — installation licensing operations and entitled module controls
import React, { useEffect, useMemo, useState } from "react";
import {
  CheckCircle2,
  Clock3,
  ExternalLink,
  KeyRound,
  Loader2,
  Power,
  RefreshCw,
  ShieldCheck,
  Upload,
} from "lucide-react";
import toast from "react-hot-toast";

import { Badge } from "@/components/ui/badge";
import { Button, buttonVariants } from "@/components/ui/button";
import { ConfirmDrawer } from "@/components/ui/confirm-drawer";
import { DialogBody, DialogFooter, DialogHeader, DialogShell } from "@/components/ui/dialog";
import { Input, Label } from "@/components/ui/input";
import { cn, formatDateTime } from "@/lib/utils";
import { usePageChrome } from "@/modules/admin/layouts/page-chrome";
import type { ApiEnvelope } from "@/modules/users/types";

import { accessService } from "./access-service";
import { defaultModuleVisual, moduleVisuals } from "./module-registry";
import type { LicensingState, ModuleDefinition, TenantModule } from "./types";

type LicenseDrawer = "connect" | "import" | null;

export const LicensingPanel: React.FC = () => {
  const [catalog, setCatalog] = useState<ModuleDefinition[]>([]);
  const [entitlements, setEntitlements] = useState<TenantModule[]>([]);
  const [licensing, setLicensing] = useState<LicensingState | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [drawer, setDrawer] = useState<LicenseDrawer>(null);
  const [activationCode, setActivationCode] = useState("");
  const [offlineBundle, setOfflineBundle] = useState("");
  const [offlineFileName, setOfflineFileName] = useState("");
  const [isUpdating, setIsUpdating] = useState(false);
  const [pendingDisable, setPendingDisable] = useState<ModuleDefinition | null>(null);
  const [isDisabling, setIsDisabling] = useState(false);

  const load = async () => {
    setIsLoading(true);
    setLoadError(null);
    try {
      const [catalogResponse, moduleResponse, licenseResponse] = await Promise.all([
        accessService.getCatalog(),
        accessService.listModules(),
        accessService.getLicensingState(),
      ]);
      if (
        !catalogResponse.success || !catalogResponse.data ||
        !moduleResponse.success || !moduleResponse.data ||
        !licenseResponse.success || !licenseResponse.data
      ) {
        setLoadError("Licensing information could not be loaded.");
        return;
      }
      setCatalog(catalogResponse.data.modules.filter((module) => !module.core));
      setEntitlements(moduleResponse.data.modules);
      setLicensing(licenseResponse.data);
    } catch {
      setLoadError("Campus Pilot could not reach licensing. Check the connection and try again.");
    } finally {
      setIsLoading(false);
    }
  };

  useEffect(() => { void load(); }, []);

  const pageAction = useMemo(
    () => licensing?.connected ? (
      <Button disabled={isUpdating} onClick={() => void refresh()}>
        {isUpdating ? <Loader2 className="size-4 animate-spin" /> : <RefreshCw className="size-4" />}
        Refresh license
      </Button>
    ) : (
      <Button onClick={() => setDrawer("connect")}><KeyRound className="size-4" />Connect</Button>
    ),
    [isUpdating, licensing?.connected],
  );
  const customerPortalLoginUrl = useMemo(
    () => licensing?.portal_url ? portalLoginUrl(licensing.portal_url) : null,
    [licensing?.portal_url],
  );
  usePageChrome("Licensing", pageAction);

  const statusByKey = useMemo(() => new Map(entitlements.map((item) => [item.key, item])), [entitlements]);

  const connect = async (event: React.FormEvent) => {
    event.preventDefault();
    if (!activationCode.trim()) {
      toast.error("Enter an activation code");
      return;
    }
    setIsUpdating(true);
    try {
      const response = await accessService.connectLicense(activationCode.trim());
      if (!response.success) {
        toast.error(firstIssue(response, "The activation code could not be used"));
        return;
      }
      toast.success("License connected");
      setActivationCode("");
      setDrawer(null);
      await load();
    } catch {
      toast.error("Campus Pilot could not reach licensing. Try again.");
    } finally {
      setIsUpdating(false);
    }
  };

  const refresh = async () => {
    setIsUpdating(true);
    try {
      const response = await accessService.refreshLicense();
      if (!response.success) {
        toast.error(firstIssue(response, "The license could not be refreshed"));
        return;
      }
      toast.success("License refreshed");
      await load();
    } catch {
      toast.error("Campus Pilot could not reach licensing. Try again.");
    } finally {
      setIsUpdating(false);
    }
  };

  const importBundle = async (event: React.FormEvent) => {
    event.preventDefault();
    if (!offlineBundle.trim()) {
      toast.error("Choose a .cp-license file");
      return;
    }
    setIsUpdating(true);
    try {
      const response = await accessService.importLicense(offlineBundle);
      if (!response.success) {
        toast.error(firstIssue(response, "The license file could not be imported"));
        return;
      }
      toast.success("Offline license imported");
      setOfflineBundle("");
      setOfflineFileName("");
      setDrawer(null);
      await load();
    } catch {
      toast.error("The license file could not be imported");
    } finally {
      setIsUpdating(false);
    }
  };

  const selectOfflineFile = async (event: React.ChangeEvent<HTMLInputElement>) => {
    const file = event.target.files?.[0];
    if (!file) return;
    if (!file.name.endsWith(".cp-license")) {
      toast.error("Choose a .cp-license file");
      event.target.value = "";
      return;
    }
    try {
      setOfflineBundle(await file.text());
      setOfflineFileName(file.name);
    } catch {
      toast.error("The license file could not be read");
    }
  };

  const disable = async () => {
    if (!pendingDisable) return;
    setIsDisabling(true);
    try {
      const response = await accessService.disableModule(pendingDisable.key);
      if (!response.success) {
        toast.error(firstIssue(response, "The module could not be disabled"));
        return;
      }
      toast.success(`${pendingDisable.label} disabled`);
      setPendingDisable(null);
      await load();
    } catch {
      toast.error("Campus Pilot could not update the module. Try again.");
    } finally {
      setIsDisabling(false);
    }
  };

  return (
    <div className="space-y-8">
      {isLoading ? (
        <div className="grid gap-4 lg:grid-cols-3">
          <div className="h-32 animate-pulse bg-[var(--surface-sunken)] lg:col-span-2" />
          <div className="h-32 animate-pulse bg-[var(--surface-sunken)]" />
        </div>
      ) : null}

      {!isLoading && loadError ? (
        <div className="border border-[var(--tone-danger-bd)] bg-[var(--tone-danger-bg)] p-5" role="alert">
          <h2 className="font-semibold text-[var(--tone-danger-strong)]">Licensing could not be loaded</h2>
          <p className="mt-1 text-sm text-[var(--tone-danger-strong)]">{loadError}</p>
          <Button className="mt-4" onClick={() => void load()} variant="secondary"><RefreshCw className="size-4" />Try again</Button>
        </div>
      ) : null}

      {!isLoading && !loadError && licensing ? (
        <>
          <section className="grid border border-[var(--border)] bg-[var(--surface)] lg:grid-cols-[minmax(0,1fr)_260px]">
            <div className="p-5 sm:p-6">
              <div className="flex flex-wrap items-start justify-between gap-4">
                <div>
                  <p className="text-[11px] font-semibold uppercase tracking-[0.16em] text-[var(--text-muted)]">Installation license</p>
                  <div className="mt-2 flex flex-wrap items-center gap-2">
                    <h2 className="text-xl font-semibold tracking-[-0.025em] text-[var(--text-strong)]">{licensing.connected ? "Connected" : "Not connected"}</h2>
                    <Badge tone={licenseTone(licensing)} dot>{licenseLabel(licensing)}</Badge>
                  </div>
                </div>
                <div className="flex flex-wrap gap-2">
                  {licensing.connected ? <Button onClick={() => setDrawer("import")} variant="secondary"><Upload className="size-4" />Import file</Button> : null}
                  {customerPortalLoginUrl ? (
                    <a className={cn(buttonVariants({ variant: "secondary" }), "no-underline")} href={customerPortalLoginUrl} rel="noreferrer" target="_blank">Customer portal <ExternalLink className="size-4" /></a>
                  ) : null}
                </div>
              </div>
              <dl className="mt-6 grid gap-5 border-t border-[var(--border-subtle)] pt-5 sm:grid-cols-3">
                <StatusDatum label="Lease sequence" value={licensing.latest_sequence > 0 ? String(licensing.latest_sequence) : "None"} />
                <StatusDatum label="Last refreshed" value={dateLabel(licensing.last_refresh_success_at)} />
                <StatusDatum label="Access through" value={dateLabel(licensing.lease?.grace_until ?? null)} />
              </dl>
            </div>
            <div className="border-t border-[var(--border)] bg-[var(--surface-muted)] p-5 lg:border-l lg:border-t-0">
              <p className="text-xs font-semibold uppercase tracking-[0.12em] text-[var(--text-muted)]">Deployment</p>
              <p className="mt-3 break-all font-mono text-xs leading-5 text-[var(--text-body)]">{licensing.deployment_id}</p>
              {licensing.credential_hint ? <p className="mt-3 text-xs text-[var(--text-muted)]">Credential ending {licensing.credential_hint}</p> : null}
              {!licensing.configured ? <p className="mt-4 text-xs leading-5 text-[var(--tone-warn-strong)]">Licensing configuration is incomplete on this server.</p> : null}
            </div>
          </section>

          <section aria-labelledby="licensed-modules">
            <div className="border-b border-[var(--border)] pb-3">
              <p className="text-[11px] font-semibold uppercase tracking-[0.16em] text-[var(--text-muted)]">Campus catalog</p>
              <h2 className="mt-1 text-xl font-semibold tracking-[-0.025em] text-[var(--text-strong)]" id="licensed-modules">Modules</h2>
            </div>
            <div className="grid gap-x-8 md:grid-cols-2">
              {catalog.map((module) => {
                const entitlement = statusByKey.get(module.key);
                const enabled = entitlement?.enabled ?? false;
                const visual = moduleVisuals[module.key] ?? defaultModuleVisual;
                const Icon = visual.icon;
                return (
                  <article className="flex min-h-[168px] items-start gap-4 border-b border-[var(--border-subtle)] py-6" key={module.key}>
                    <span className={`flex size-11 shrink-0 items-center justify-center rounded-[10px] ${enabled ? "bg-[var(--brand-soft)] text-[var(--brand-strong)]" : "bg-[var(--surface-sunken)] text-[var(--text-muted)]"}`}><Icon className="size-[19px]" /></span>
                    <div className="min-w-0 flex-1">
                      <div className="flex flex-wrap items-center justify-between gap-2">
                        <h3 className="font-semibold text-[var(--text-strong)]">{module.label}</h3>
                        <Badge tone={enabled ? "success" : "neutral"}>{enabled ? "Enabled" : moduleStatus(entitlement)}</Badge>
                      </div>
                      <p className="mt-1.5 text-sm leading-5 text-[var(--text-muted)]">{module.description}</p>
                      <div className="mt-3 flex flex-wrap items-center gap-3 text-xs text-[var(--text-subtle)]">
                        <span className="inline-flex items-center gap-1.5">{enabled ? <CheckCircle2 className="size-3.5 text-[var(--tone-success)]" /> : <ShieldCheck className="size-3.5" />}{sourceLabel(entitlement?.source)}</span>
                        {entitlement?.expires_at ? <span className="inline-flex items-center gap-1.5"><Clock3 className="size-3.5" />{dateLabel(entitlement.expires_at)}</span> : null}
                        {enabled && entitlement?.source === "license" ? <button className="inline-flex min-h-8 items-center gap-1.5 font-semibold text-[var(--tone-danger)] hover:underline" onClick={() => setPendingDisable(module)} type="button"><Power className="size-3.5" />Disable</button> : null}
                      </div>
                    </div>
                  </article>
                );
              })}
            </div>
          </section>
        </>
      ) : null}

      <DialogShell onClose={() => !isUpdating && setDrawer(null)} open={drawer === "connect"}>
        <DialogHeader onClose={() => setDrawer(null)} title="Connect licensing" />
        <form className="flex min-h-0 flex-1 flex-col" onSubmit={connect}>
          <DialogBody className="space-y-6">
            <div className="space-y-3 text-sm leading-6 text-[var(--text-muted)]">
              <p>Create a one-time code under Installations in the customer portal, then paste it below. Campus Pilot will obtain and store the license automatically.</p>
              {customerPortalLoginUrl ? (
                <a className={cn(buttonVariants({ variant: "secondary" }), "no-underline")} href={customerPortalLoginUrl} rel="noreferrer" target="_blank">
                  Open customer portal
                  <ExternalLink className="size-4" />
                </a>
              ) : null}
            </div>
            <div>
              <Label htmlFor="activation-code">Activation code</Label>
              <Input autoCapitalize="none" autoComplete="off" className="mt-2 font-mono" data-autofocus="true" id="activation-code" onChange={(event) => setActivationCode(event.target.value)} placeholder="cpact_…" spellCheck={false} value={activationCode} />
            </div>
          </DialogBody>
          <DialogFooter>
            <Button disabled={isUpdating} onClick={() => setDrawer(null)} type="button" variant="ghost">Cancel</Button>
            <Button disabled={isUpdating || !activationCode.trim()} type="submit">{isUpdating ? <><Loader2 className="size-4 animate-spin" />Connecting…</> : <><KeyRound className="size-4" />Connect</>}</Button>
          </DialogFooter>
        </form>
      </DialogShell>

      <DialogShell onClose={() => !isUpdating && setDrawer(null)} open={drawer === "import"}>
        <DialogHeader onClose={() => setDrawer(null)} title="Import offline license" />
        <form className="flex min-h-0 flex-1 flex-col" onSubmit={importBundle}>
          <DialogBody className="space-y-6">
            <p className="text-sm leading-6 text-[var(--text-muted)]">Use a signed .cp-license file issued for this installation.</p>
            <div>
              <Label htmlFor="offline-license">License file</Label>
              <label className="mt-2 flex min-h-28 cursor-pointer flex-col items-center justify-center gap-2 border border-dashed border-[var(--border-strong)] bg-[var(--surface-muted)] px-5 text-center hover:bg-[var(--surface-sunken)]" htmlFor="offline-license">
                <Upload className="size-5 text-[var(--brand-strong)]" />
                <span className="text-sm font-medium text-[var(--text-strong)]">{offlineFileName || "Choose a .cp-license file"}</span>
              </label>
              <input accept=".cp-license,application/json" className="sr-only" id="offline-license" onChange={(event) => void selectOfflineFile(event)} type="file" />
            </div>
          </DialogBody>
          <DialogFooter>
            <Button disabled={isUpdating} onClick={() => setDrawer(null)} type="button" variant="ghost">Cancel</Button>
            <Button disabled={isUpdating || !offlineBundle} type="submit">{isUpdating ? <><Loader2 className="size-4 animate-spin" />Importing…</> : <><Upload className="size-4" />Import</>}</Button>
          </DialogFooter>
        </form>
      </DialogShell>

      <ConfirmDrawer confirmLabel="Disable module" description={`Disable ${pendingDisable?.label || "this module"}? It will disappear from the campus launcher and its API will reject access until it is enabled again.`} isPending={isDisabling} onClose={() => setPendingDisable(null)} onConfirm={() => void disable()} open={pendingDisable !== null} title="Disable module?" />
    </div>
  );
};

function StatusDatum({ label, value }: { label: string; value: string }) {
  return <div><dt className="text-xs text-[var(--text-muted)]">{label}</dt><dd className="mt-1 text-sm font-semibold text-[var(--text-strong)]">{value}</dd></div>;
}

function sourceLabel(source?: TenantModule["source"]) {
  if (source === "core") return "Included";
  if (source === "license") return "License";
  if (source === "legacy") return "Enabled";
  return "License required";
}

function moduleStatus(module?: TenantModule) {
  if (module?.status === "disabled") return "Disabled";
  if (module?.status === "expired") return "Expired";
  if (module?.status === "revoked") return "Revoked";
  return "Not enabled";
}

function licenseTone(licensing: LicensingState): "success" | "warn" | "danger" | "neutral" {
  if (!licensing.configured || !licensing.connected) return "neutral";
  if (licensing.status === "active" && licensing.lease && new Date(licensing.lease.lease_expires_at) > new Date()) return "success";
  if (licensing.status === "active" && licensing.lease && new Date(licensing.lease.grace_until) > new Date()) return "warn";
  return "danger";
}

function licenseLabel(licensing: LicensingState) {
  if (!licensing.configured) return "Setup required";
  if (!licensing.connected) return "Not connected";
  if (licensing.status !== "active") return licensing.status[0].toUpperCase() + licensing.status.slice(1);
  if (licensing.lease && new Date(licensing.lease.lease_expires_at) > new Date()) return "Active";
  if (licensing.lease && new Date(licensing.lease.grace_until) > new Date()) return "Grace period";
  return "Expired";
}

function dateLabel(value: string | null) {
  return value ? formatDateTime(value) : "Not available";
}

function firstIssue<T>(response: ApiEnvelope<T>, fallback: string) {
  const issue = response.issues?.[0];
  if (typeof issue === "string") return issue;
  if (issue?.detail) return issue.detail;
  return response.message || fallback;
}

function portalLoginUrl(baseUrl: string) {
  try {
    const url = new URL(baseUrl);
    url.pathname = "/login";
    url.search = "";
    url.hash = "";
    return url.toString();
  } catch {
    return null;
  }
}

import React, { useEffect, useMemo, useState } from "react";
import { CheckCircle2, KeyRound, Loader2, LockKeyhole, Power, RefreshCw, ShieldCheck } from "lucide-react";
import toast from "react-hot-toast";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { ConfirmDrawer } from "@/components/ui/confirm-drawer";
import { DialogBody, DialogFooter, DialogHeader, DialogShell } from "@/components/ui/dialog";
import { Label, Textarea } from "@/components/ui/input";
import { usePageChrome } from "@/modules/admin/layouts/page-chrome";
import type { ApiEnvelope } from "@/modules/users/types";

import { accessService } from "./access-service";
import { defaultModuleVisual, moduleVisuals } from "./module-registry";
import type { ModuleDefinition, TenantModule } from "./types";

export const LicensingPanel: React.FC = () => {
  const [catalog, setCatalog] = useState<ModuleDefinition[]>([]);
  const [entitlements, setEntitlements] = useState<TenantModule[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [licenseDrawerOpen, setLicenseDrawerOpen] = useState(false);
  const [licenseKey, setLicenseKey] = useState("");
  const [isActivating, setIsActivating] = useState(false);
  const [pendingDisable, setPendingDisable] = useState<ModuleDefinition | null>(null);
  const [isDisabling, setIsDisabling] = useState(false);

  const load = async () => {
    setIsLoading(true);
    setLoadError(null);
    try {
      const [catalogResponse, moduleResponse] = await Promise.all([
        accessService.getCatalog(),
        accessService.listModules(),
      ]);
      if (!catalogResponse.success || !catalogResponse.data || !moduleResponse.success || !moduleResponse.data) {
        setLoadError("Licensing information could not be loaded.");
        return;
      }
      setCatalog(catalogResponse.data.modules.filter((module) => !module.core));
      setEntitlements(moduleResponse.data.modules);
    } catch {
      setLoadError("Campus Pilot could not reach licensing. Check the connection and try again.");
    } finally {
      setIsLoading(false);
    }
  };

  useEffect(() => { void load(); }, []);

  usePageChrome(
    "Licensing",
    <Button onClick={() => setLicenseDrawerOpen(true)}><KeyRound className="size-4" />Activate license</Button>,
  );

  const statusByKey = useMemo(() => new Map(entitlements.map((item) => [item.key, item])), [entitlements]);

  const activate = async (event: React.FormEvent) => {
    event.preventDefault();
    if (!licenseKey.trim()) {
      toast.error("Paste a signed license key to continue");
      return;
    }
    setIsActivating(true);
    try {
      const response = await accessService.activateLicense(licenseKey.trim());
      if (!response.success) {
        toast.error(firstIssue(response, "The license key could not be activated"));
        return;
      }
      toast.success("Module license activated");
      setLicenseKey("");
      setLicenseDrawerOpen(false);
      await load();
    } catch {
      toast.error("Campus Pilot could not reach licensing. Try again.");
    } finally {
      setIsActivating(false);
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
      <section className="grid gap-6 lg:grid-cols-[minmax(0,1fr)_minmax(320px,0.45fr)] lg:items-end">
        <div>
          <p className="text-[11px] font-semibold uppercase tracking-[0.18em] text-[var(--brand-strong)]">Module entitlements</p>
          <h1 className="mt-3 max-w-[21ch] text-3xl font-semibold leading-[1.08] tracking-[-0.045em] text-[var(--text-strong)]">Enable school capabilities without changing anyone’s role.</h1>
          <p className="mt-4 max-w-[34em] text-sm leading-6 text-[var(--text-muted)]">A module must be licensed and permitted by a role before it appears on the campus launcher.</p>
        </div>
        <div className="flex items-start gap-3 bg-[var(--surface-muted)] p-4">
          <LockKeyhole className="mt-0.5 size-5 shrink-0 text-[var(--brand-strong)]" />
          <p className="text-sm leading-6 text-[var(--text-muted)]">License keys are signature-verified. Campus Pilot stores only a fingerprint and entitlement claims, never the original key.</p>
        </div>
      </section>

      {isLoading ? <div className="grid gap-4 md:grid-cols-2"><div className="h-36 animate-pulse bg-[var(--surface-sunken)]" /><div className="h-36 animate-pulse bg-[var(--surface-sunken)]" /></div> : null}

      {!isLoading && loadError ? (
        <div className="border border-[var(--tone-danger-bd)] bg-[var(--tone-danger-bg)] p-5" role="alert">
          <h2 className="font-semibold text-[var(--tone-danger-strong)]">Licensing could not be loaded</h2>
          <p className="mt-1 text-sm text-[var(--tone-danger-strong)]">{loadError}</p>
          <Button className="mt-4" onClick={() => void load()} variant="secondary"><RefreshCw className="size-4" />Try again</Button>
        </div>
      ) : null}

      {!isLoading && !loadError ? (
        <section aria-labelledby="licensed-modules">
          <div className="border-b border-[var(--border)] pb-3">
            <p className="text-[11px] font-semibold uppercase tracking-[0.16em] text-[var(--text-muted)]">Campus catalog</p>
            <h2 className="mt-1 text-xl font-semibold tracking-[-0.025em] text-[var(--text-strong)]" id="licensed-modules">Licensed and available modules</h2>
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
                      <Badge tone={enabled ? "success" : "neutral"}>{enabled ? "Enabled" : "Not enabled"}</Badge>
                    </div>
                    <p className="mt-1.5 text-sm leading-5 text-[var(--text-muted)]">{module.description}</p>
                    <div className="mt-3 flex flex-wrap items-center gap-3 text-xs text-[var(--text-subtle)]">
                      <span className="inline-flex items-center gap-1.5">{enabled ? <CheckCircle2 className="size-3.5 text-[var(--tone-success)]" /> : <ShieldCheck className="size-3.5" />}{sourceLabel(entitlement?.source)}</span>
                      {entitlement?.expires_at ? <span>Expires {new Date(entitlement.expires_at).toLocaleDateString()}</span> : null}
                      {enabled ? <button className="inline-flex min-h-8 items-center gap-1.5 font-semibold text-[var(--tone-danger)] hover:underline" onClick={() => setPendingDisable(module)} type="button"><Power className="size-3.5" />Disable</button> : null}
                    </div>
                  </div>
                </article>
              );
            })}
          </div>
        </section>
      ) : null}

      <DialogShell onClose={() => !isActivating && setLicenseDrawerOpen(false)} open={licenseDrawerOpen}>
        <DialogHeader onClose={() => setLicenseDrawerOpen(false)} title="Activate license" />
        <form className="contents" onSubmit={activate}>
          <DialogBody className="space-y-6">
            <p className="text-xs font-semibold uppercase tracking-[0.14em] text-[var(--text-subtle)]">Signed module entitlement</p>
            <div className="bg-[var(--surface-muted)] p-4 text-sm leading-6 text-[var(--text-muted)]">The key must be issued for this campus. It may enable one module or a licensed bundle and can include an expiry date.</div>
            <div>
              <Label htmlFor="license-key">License key</Label>
              <Textarea autoComplete="off" className="mt-2 min-h-40 resize-y font-mono text-xs" id="license-key" onChange={(event) => setLicenseKey(event.target.value)} placeholder="Paste the complete signed license key" spellCheck={false} value={licenseKey} />
              <p className="mt-2 text-xs leading-5 text-[var(--text-subtle)]">The original key is discarded after successful verification.</p>
            </div>
          </DialogBody>
          <DialogFooter>
            <Button disabled={isActivating} onClick={() => setLicenseDrawerOpen(false)} type="button" variant="ghost">Keep current licensing</Button>
            <Button disabled={isActivating || !licenseKey.trim()} type="submit">{isActivating ? <><Loader2 className="size-4 animate-spin" />Verifying license…</> : <><KeyRound className="size-4" />Activate license</>}</Button>
          </DialogFooter>
        </form>
      </DialogShell>

      <ConfirmDrawer confirmLabel="Disable module" description={`Disable ${pendingDisable?.label || "this module"}? It will disappear from the campus launcher and its API will reject access until it is enabled again.`} isPending={isDisabling} onClose={() => setPendingDisable(null)} onConfirm={() => void disable()} open={pendingDisable !== null} title="Disable module?" />
    </div>
  );
};

function sourceLabel(source?: TenantModule["source"]) {
  if (source === "core") return "Core module";
  if (source === "license") return "Signed license";
  if (source === "legacy") return "Existing installation";
  return "License required";
}

function firstIssue<T>(response: ApiEnvelope<T>, fallback: string) {
  const issue = response.issues?.[0];
  if (typeof issue === "string") return issue;
  if (issue?.detail) return issue.detail;
  return response.message || fallback;
}

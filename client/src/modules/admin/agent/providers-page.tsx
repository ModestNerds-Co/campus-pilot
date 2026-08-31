/**
 * Administration workspace for tenant-scoped, write-only AI provider connections.
 * All focused and destructive workflows use the shared accessible right-side drawer.
 */

import React, { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  AlertTriangle,
  Bot,
  CheckCircle2,
  KeyRound,
  Loader2,
  Pencil,
  Plug,
  RefreshCw,
  RotateCw,
  SearchCheck,
  ServerCog,
  ShieldCheck,
  Trash2,
} from "lucide-react";
import toast from "react-hot-toast";

import { SearchableSelect } from "@/components/searchable-select";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { DialogBody, DialogFooter, DialogHeader, DialogShell } from "@/components/ui/dialog";
import { Input, Label, Textarea } from "@/components/ui/input";
import { TableError, TableLoading, TableWrap } from "@/components/ui/data-table";
import { formatDate } from "@/lib/utils";
import { usePageChrome } from "@/modules/admin/layouts/page-chrome";
import { useAuthStore } from "@/stores/auth-store";

import { aiProviderErrorMessage, aiProviderService } from "./ai-provider-service";
import type {
  AiProviderConnection,
  AiProviderConnectionStatus,
  AiProviderDataApprovalClass,
  AiProviderKey,
  ProviderCatalogEntry,
  ProviderModelSnapshot,
  ProviderTestOutcome,
} from "./types";

type ProviderDrawer =
  | { kind: "connect" }
  | { kind: "update" | "rotate" | "test" | "models" | "approval" | "disconnect"; connection: AiProviderConnection }
  | null;

export function AiProvidersPage() {
  const user = useAuthStore((state) => state.user);
  const canEdit = hasPermission(user?.permissions, "ai_providers:edit");
  const [providers, setProviders] = useState<ProviderCatalogEntry[]>([]);
  const [connections, setConnections] = useState<AiProviderConnection[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [drawer, setDrawer] = useState<ProviderDrawer>(null);
  const loadGeneration = useRef(0);

  const load = useCallback(async (quiet = false) => {
    const generation = ++loadGeneration.current;
    if (!quiet) setIsLoading(true);
    setLoadError(null);
    try {
      const [providerResponse, connectionResponse] = await Promise.all([
        aiProviderService.listProviders(),
        aiProviderService.listConnections(),
      ]);
      if (generation !== loadGeneration.current) return;
      if (!providerResponse.success || !providerResponse.data) {
        setLoadError(aiProviderErrorMessage(providerResponse, "The provider catalogue could not be loaded."));
        return;
      }
      if (!connectionResponse.success || !connectionResponse.data) {
        setLoadError(aiProviderErrorMessage(connectionResponse, "Provider connections could not be loaded."));
        return;
      }
      setProviders(providerResponse.data);
      setConnections(connectionResponse.data);
      setDrawer((current) => {
        if (!current || !("connection" in current)) return current;
        const refreshed = connectionResponse.data?.find((connection) => connection.id === current.connection.id);
        return refreshed ? { ...current, connection: refreshed } : current;
      });
    } catch {
      if (generation === loadGeneration.current) {
        setLoadError("Campus Pilot could not reach AI provider administration. Check the connection and try again.");
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

  const pageAction = useMemo(
    () => canEdit ? (
      <Button disabled={isLoading || Boolean(loadError) || providers.length === 0} onClick={() => setDrawer({ kind: "connect" })}>
        <Plug className="size-4" />
        Connect provider
      </Button>
    ) : null,
    [canEdit, isLoading, loadError, providers.length],
  );
  usePageChrome("AI providers", pageAction);

  const readyCount = connections.filter((connection) => connection.status === "ready").length;
  const attentionCount = connections.length - readyCount;
  const modelCount = connections.reduce((total, connection) => total + connection.model_count, 0);

  const replaceConnection = (connection: AiProviderConnection) => {
    setConnections((current) => current.some((item) => item.id === connection.id)
      ? current.map((item) => item.id === connection.id ? connection : item)
      : [connection, ...current]);
    setDrawer((current) => current && "connection" in current ? { ...current, connection } : current);
  };

  return (
    <div className="space-y-7">
      <section className="grid gap-5 border-b border-[var(--border)] pb-6 lg:grid-cols-[minmax(0,1fr)_minmax(420px,0.8fr)] lg:items-end">
        <div>
          <p className="max-w-2xl text-sm leading-6 text-[var(--text-muted)]">
            Connect the provider accounts this campus can use for Agent runs. API keys are write-only and are not shown again.
          </p>
        </div>
        {!isLoading && !loadError ? (
          <dl className="grid grid-cols-2 border border-[var(--border)] bg-[var(--surface)] sm:grid-cols-4">
            <Metric label="Connections" value={connections.length} />
            <Metric label="Ready" value={readyCount} />
            <Metric label="Attention" value={attentionCount} />
            <Metric label="Models" value={modelCount} />
          </dl>
        ) : null}
      </section>

      {isLoading ? (
        <TableWrap>
          <TableLoading columns={4} label="Loading AI providers…" rows={4} />
        </TableWrap>
      ) : loadError ? (
        <TableWrap>
          <TableError description={loadError} onRetry={() => void load()} title="AI providers could not be loaded" />
        </TableWrap>
      ) : connections.length === 0 ? (
        <section className="border border-dashed border-[var(--border-strong)] bg-[var(--surface)] px-5 py-10 sm:px-8" aria-labelledby="providers-empty-title">
          <div className="flex max-w-2xl flex-col items-start gap-5 sm:flex-row">
            <span className="flex size-11 shrink-0 items-center justify-center rounded-[10px] bg-[var(--brand-soft)] text-[var(--brand-strong)]">
              <ServerCog className="size-5" />
            </span>
            <div>
              <h2 className="font-semibold text-[var(--text-strong)]" id="providers-empty-title">No provider connections</h2>
              <p className="mt-1 text-sm leading-6 text-[var(--text-muted)]">
                Agent runs cannot contact a model until a connection is saved and passes a connection test.
              </p>
              {canEdit ? (
                <Button className="mt-4" disabled={providers.length === 0} onClick={() => setDrawer({ kind: "connect" })}>
                  <Plug className="size-4" />Connect provider
                </Button>
              ) : null}
            </div>
          </div>
        </section>
      ) : (
        <section aria-labelledby="provider-connections-title">
          <div className="mb-3 flex flex-wrap items-end justify-between gap-3">
            <div>
              <p className="text-[11px] font-semibold uppercase tracking-[0.16em] text-[var(--text-muted)]">Campus connections</p>
              <h2 className="mt-1 text-xl font-semibold tracking-[-0.025em] text-[var(--text-strong)]" id="provider-connections-title">Connections</h2>
            </div>
            {readyCount === 0 ? (
              <p className="flex items-center gap-2 text-xs font-medium text-[var(--tone-warn-strong)]" role="status">
                <AlertTriangle className="size-4" />No connection is ready
              </p>
            ) : null}
          </div>
          <div className="overflow-hidden border border-[var(--border)] bg-[var(--surface)] shadow-[var(--shadow-card)]">
            <div className="hidden grid-cols-[minmax(190px,1.2fr)_minmax(190px,1fr)_minmax(150px,0.7fr)_auto] gap-5 border-b border-[var(--border)] bg-[var(--table-header-bg)] px-5 py-3 text-[11px] font-semibold uppercase tracking-[0.13em] text-[var(--table-header-text)] lg:grid">
              <span>Connection</span><span>Health</span><span>Models</span><span className="text-right">Actions</span>
            </div>
            <ul className="divide-y divide-[var(--border-subtle)]">
              {connections.map((connection) => (
                <ConnectionRow
                  canEdit={canEdit}
                  connection={connection}
                  key={connection.id}
                  onOpen={(kind) => setDrawer({ kind, connection })}
                  provider={providers.find((item) => item.key === connection.provider)}
                />
              ))}
            </ul>
          </div>
        </section>
      )}

      <ProviderWorkflowDrawer
        canEdit={canEdit}
        drawer={drawer}
        key={drawerKey(drawer)}
        onClose={() => setDrawer(null)}
        onDisconnected={(connectionId) => {
          setConnections((current) => current.filter((connection) => connection.id !== connectionId));
          setDrawer(null);
        }}
        onApprovalSaved={() => void load(true)}
        onModelsRefreshed={() => void load(true)}
        onUpdated={replaceConnection}
        providers={providers}
      />
    </div>
  );
}

function ConnectionRow({
  canEdit,
  connection,
  onOpen,
  provider,
}: {
  canEdit: boolean;
  connection: AiProviderConnection;
  onOpen: (kind: Exclude<ProviderDrawer, null | { kind: "connect" }>["kind"]) => void;
  provider: ProviderCatalogEntry | undefined;
}) {
  const testCopy = connection.last_tested_at
    ? `${connection.last_test_status === "succeeded" ? "Passed" : "Failed"} ${formatDate(connection.last_tested_at)}`
    : "Not tested";
  return (
    <li className="grid gap-5 px-5 py-5 lg:grid-cols-[minmax(190px,1.2fr)_minmax(190px,1fr)_minmax(150px,0.7fr)_auto] lg:items-center">
      <div className="min-w-0">
        <div className="flex items-center gap-3">
          <span className="flex size-10 shrink-0 items-center justify-center rounded-[9px] bg-[var(--brand-soft)] text-[var(--brand-strong)]">
            <Bot className="size-[18px]" />
          </span>
          <div className="min-w-0">
            <p className="truncate font-semibold text-[var(--text-strong)]">{connection.account_label}</p>
            <p className="mt-0.5 truncate text-xs text-[var(--text-muted)]">{connection.provider_label}</p>
          </div>
        </div>
        <p className="mt-3 text-xs text-[var(--text-subtle)] lg:ml-[52px]">
          Connected by {connection.configured_by_name}
        </p>
      </div>

      <div className="min-w-0">
        <Badge dot tone={statusTone(connection.status)}>{statusLabel(connection.status)}</Badge>
        <p className="mt-2 text-xs text-[var(--text-muted)]">{testCopy}</p>
        <div className="mt-2 flex flex-wrap items-center gap-2">
          <Badge tone={approvalTone(connection.provider_data_approval_class)}>
            {approvalLabel(connection.provider_data_approval_class)}
          </Badge>
          <span className="text-xs text-[var(--text-subtle)]">
            Decision {connection.provider_data_approval_version}
          </span>
        </div>
        {connection.status === "error" && connection.last_failure_category ? (
          <p className="mt-1 text-xs text-[var(--tone-danger-strong)]">{friendlyFailure(connection.last_failure_category)}</p>
        ) : null}
      </div>

      <div>
        <p className="text-sm font-semibold text-[var(--text-strong)]">{connection.model_count} cached</p>
        <p className="mt-1 text-xs text-[var(--text-muted)]">
          {connection.model_catalog_refreshed_at ? `Updated ${formatDate(connection.model_catalog_refreshed_at)}` : "Not refreshed"}
        </p>
      </div>

      <div className="flex flex-wrap gap-2 lg:max-w-[330px] lg:justify-end">
        <Button onClick={() => onOpen("models")} size="sm" variant="secondary">
          <SearchCheck className="size-3.5" />Models
        </Button>
        {canEdit ? (
          <>
            <Button onClick={() => onOpen("approval")} size="sm" variant="secondary">
              <ShieldCheck className="size-3.5" />Data approval
            </Button>
            <Button disabled={provider?.supports_connection_test === false} onClick={() => onOpen("test")} size="sm" variant={connection.status === "ready" ? "ghost" : "secondary"}>
              <CheckCircle2 className="size-3.5" />Test
            </Button>
            <Button aria-label={`Edit ${connection.account_label}`} onClick={() => onOpen("update")} size="icon-sm" variant="ghost">
              <Pencil className="size-3.5" />
            </Button>
            <Button aria-label={`Rotate credential for ${connection.account_label}`} onClick={() => onOpen("rotate")} size="icon-sm" variant="ghost">
              <RotateCw className="size-3.5" />
            </Button>
            <Button aria-label={`Disconnect ${connection.account_label}`} onClick={() => onOpen("disconnect")} size="icon-sm" variant="ghost">
              <Trash2 className="size-3.5 text-[var(--tone-danger)]" />
            </Button>
          </>
        ) : null}
      </div>
    </li>
  );
}

function ProviderWorkflowDrawer({
  canEdit,
  drawer,
  onClose,
  onDisconnected,
  onApprovalSaved,
  onModelsRefreshed,
  onUpdated,
  providers,
}: {
  canEdit: boolean;
  drawer: ProviderDrawer;
  onClose: () => void;
  onDisconnected: (connectionId: string) => void;
  onApprovalSaved: () => void;
  onModelsRefreshed: () => void;
  onUpdated: (connection: AiProviderConnection) => void;
  providers: ProviderCatalogEntry[];
}) {
  const connection = drawer && "connection" in drawer ? drawer.connection : null;
  const connectionProvider = providers.find((provider) => provider.key === connection?.provider);
  const [providerKey, setProviderKey] = useState<AiProviderKey | null>(providers[0]?.key ?? null);
  const [accountLabel, setAccountLabel] = useState(connection?.account_label ?? "");
  const [apiKey, setApiKey] = useState("");
  const [busy, setBusy] = useState(false);
  const busyRef = useRef(false);
  const [formError, setFormError] = useState<string | null>(null);
  const [approvalClass, setApprovalClass] = useState<Exclude<AiProviderDataApprovalClass, "unapproved"> | null>(null);
  const [approvalReason, setApprovalReason] = useState("");
  const [approvalConnection, setApprovalConnection] = useState<AiProviderConnection | null>(connection);
  const [approvalLoading, setApprovalLoading] = useState(drawer?.kind === "approval");
  const [approvalLoadError, setApprovalLoadError] = useState<string | null>(null);
  const [approvalReload, setApprovalReload] = useState(0);
  const [testOutcome, setTestOutcome] = useState<ProviderTestOutcome["outcome"] | null>(null);
  const [models, setModels] = useState<ProviderModelSnapshot | null>(null);
  const [modelsLoading, setModelsLoading] = useState(drawer?.kind === "models");
  const [modelsError, setModelsError] = useState<string | null>(null);
  const [modelsReload, setModelsReload] = useState(0);

  useEffect(() => {
    if (drawer?.kind !== "models" || !connection) return;
    let active = true;
    setModelsLoading(true);
    setModelsError(null);
    void aiProviderService.listModels(connection.id)
      .then((response) => {
        if (!active) return;
        if (response.success && response.data) setModels(response.data);
        else setModelsError(aiProviderErrorMessage(response, "Models could not be loaded."));
      })
      .catch(() => {
        if (active) setModelsError("Campus Pilot could not load the cached models. Try again.");
      })
      .finally(() => {
        if (active) setModelsLoading(false);
      });
    return () => { active = false; };
  }, [drawer?.kind, connection?.id, modelsReload]);

  useEffect(() => {
    if (drawer?.kind !== "approval" || !connection) return;
    let active = true;
    setApprovalLoading(true);
    setApprovalLoadError(null);
    void aiProviderService.getConnection(connection.id)
      .then((response) => {
        if (!active) return;
        if (response.success && response.data) {
          setApprovalConnection(response.data);
        } else {
          setApprovalLoadError(aiProviderErrorMessage(response, "The current data approval could not be loaded."));
        }
      })
      .catch(() => {
        if (active) setApprovalLoadError("Campus Pilot could not load the current data approval. Try again.");
      })
      .finally(() => {
        if (active) setApprovalLoading(false);
      });
    return () => { active = false; };
  }, [drawer?.kind, connection?.id, approvalReload]);

  if (!drawer) return null;
  const close = busy ? () => undefined : onClose;

  const run = async (work: () => Promise<void>) => {
    if (busyRef.current) return;
    busyRef.current = true;
    setBusy(true);
    setFormError(null);
    try {
      await work();
    } catch {
      setFormError("Campus Pilot could not reach the provider service. Try again.");
    } finally {
      busyRef.current = false;
      setBusy(false);
    }
  };

  const create = (event: React.FormEvent) => {
    event.preventDefault();
    if (!providerKey || !accountLabel.trim() || !apiKey.trim()) {
      setFormError("Choose a provider, enter an account label, and enter its API key.");
      return;
    }
    void run(async () => {
      const response = await aiProviderService.createConnection({
        provider: providerKey,
        auth_method: "api_key",
        account_label: accountLabel.trim(),
        api_key: apiKey.trim(),
      });
      if (!response.success || !response.data) {
        setFormError(aiProviderErrorMessage(response, "The provider connection could not be saved."));
        return;
      }
      setApiKey("");
      toast.success("Provider connection saved");
      onUpdated(response.data);
      onClose();
    });
  };

  const update = (event: React.FormEvent) => {
    event.preventDefault();
    if (!connection || !accountLabel.trim()) {
      setFormError("Enter an account label.");
      return;
    }
    void run(async () => {
      const response = await aiProviderService.updateConnection(connection.id, {
        account_label: accountLabel.trim(),
        expected_version: connection.version,
      });
      if (!response.success || !response.data) {
        setFormError(aiProviderErrorMessage(response, "The connection could not be updated."));
        return;
      }
      toast.success("Connection updated");
      onUpdated(response.data);
      onClose();
    });
  };

  const rotate = (event: React.FormEvent) => {
    event.preventDefault();
    if (!connection || !apiKey.trim()) {
      setFormError("Enter the replacement API key.");
      return;
    }
    void run(async () => {
      const response = await aiProviderService.rotateCredential(connection.id, {
        api_key: apiKey.trim(),
        expected_version: connection.version,
      });
      if (!response.success || !response.data) {
        setFormError(aiProviderErrorMessage(response, "The credential could not be replaced."));
        return;
      }
      setApiKey("");
      toast.success("Provider credential replaced");
      onUpdated(response.data);
      onClose();
    });
  };

  const test = () => {
    if (!connection) return;
    void run(async () => {
      const response = await aiProviderService.testConnection(connection.id, connection.version);
      if (!response.success || !response.data) {
        setFormError(aiProviderErrorMessage(response, "The connection test could not run."));
        return;
      }
      setTestOutcome(response.data.outcome);
      onUpdated(response.data.connection);
      if (response.data.outcome.status === "succeeded") toast.success("Connection test passed");
    });
  };

  const refreshModels = () => {
    if (!connection) return;
    void run(async () => {
      const response = await aiProviderService.refreshModels(connection.id, connection.version);
      if (!response.success || !response.data) {
        setFormError(aiProviderErrorMessage(response, "The provider model list could not be refreshed."));
        return;
      }
      setModels(response.data);
      setModelsError(null);
      toast.success("Models refreshed");
      onModelsRefreshed();
    });
  };

  const setDataApproval = (event: React.FormEvent) => {
    event.preventDefault();
    const current = approvalConnection;
    const reason = approvalReason.trim();
    if (!current || !approvalClass) {
      setFormError("Choose the data class this connection may receive.");
      return;
    }
    if (reason.length < 3 || reason.length > 500) {
      setFormError("Enter a reason between 3 and 500 characters.");
      return;
    }
    void run(async () => {
      const response = await aiProviderService.setDataApproval(current.id, {
        approval_class: approvalClass,
        expected_approval_version: current.provider_data_approval_version,
        change_reason: reason,
      });
      if (!response.success || !response.data) {
        setFormError(aiProviderErrorMessage(response, "The data approval could not be saved."));
        return;
      }
      const updatedConnection: AiProviderConnection = {
        ...current,
        provider_data_approval_id: response.data.id,
        provider_data_approval_version: response.data.approval_version,
        provider_data_approval_class: response.data.approval_class,
        execution_environment_class: response.data.execution_environment_class,
      };
      onUpdated(updatedConnection);
      onApprovalSaved();
      setApprovalClass(null);
      setApprovalReason("");
      toast.success("Provider data approval saved");
      onClose();
    });
  };

  const disconnect = () => {
    if (!connection) return;
    void run(async () => {
      const response = await aiProviderService.disconnect(connection.id, connection.version);
      if (!response.success || !response.data) {
        setFormError(aiProviderErrorMessage(response, "The provider could not be disconnected."));
        return;
      }
      toast.success("Provider disconnected");
      onDisconnected(response.data.disconnected_id);
    });
  };

  if (drawer.kind === "connect") {
    const selectedProvider = providers.find((provider) => provider.key === providerKey);
    return (
      <DialogShell onClose={close} open>
        <DialogHeader onClose={busy ? undefined : onClose} title="Connect provider" />
        <form onSubmit={create}>
          <DialogBody className="space-y-6">
            <DrawerIntro icon={<Plug className="size-5" />} text="Save a campus-owned provider connection. It must pass a test before Agent can use it." />
            <Field label="Provider" labelFor="provider-key">
              <SearchableSelect
                allowClear={false}
                id="provider-key"
                onChange={(value) => setProviderKey(value)}
                options={providers.map((provider) => ({
                  id: provider.key,
                  value: provider.label,
                  label: provider.auth_methods.includes("api_key") ? "API key" : "Unavailable",
                }))}
                placeholder="Choose provider"
                value={providerKey}
              />
            </Field>
            <Field label="Account label" labelFor="provider-account-label">
              <Input data-autofocus="true" id="provider-account-label" onChange={(event) => setAccountLabel(event.target.value)} placeholder="e.g. School OpenAI account" required value={accountLabel} />
              <FieldHint>Use a label administrators will recognize when configuring routes.</FieldHint>
            </Field>
            <Field label="API key" labelFor="provider-api-key">
              <Input autoComplete="new-password" id="provider-api-key" onChange={(event) => setApiKey(event.target.value)} required type="password" value={apiKey} />
              <FieldHint>{selectedProvider?.credential_hint || "Enter the key issued by the provider."} It will not be shown again.</FieldHint>
            </Field>
            <FormError message={formError} />
          </DialogBody>
          <DialogFooter>
            <Button disabled={busy} onClick={onClose} type="button" variant="secondary">Cancel</Button>
            <Button disabled={busy || !providerKey || !accountLabel.trim() || !apiKey.trim()} type="submit">
              {busy ? <Loader2 className="size-4 animate-spin" /> : <KeyRound className="size-4" />}
              {busy ? "Saving…" : "Save connection"}
            </Button>
          </DialogFooter>
        </form>
      </DialogShell>
    );
  }

  if (!connection) return null;

  if (drawer.kind === "update") {
    return (
      <DialogShell onClose={close} open>
        <DialogHeader onClose={busy ? undefined : onClose} title="Edit connection" />
        <form onSubmit={update}>
          <DialogBody className="space-y-6">
            <ConnectionIdentity connection={connection} />
            <Field label="Account label" labelFor="edit-provider-label">
              <Input data-autofocus="true" id="edit-provider-label" onChange={(event) => setAccountLabel(event.target.value)} required value={accountLabel} />
            </Field>
            <FormError message={formError} />
          </DialogBody>
          <DialogFooter>
            <Button disabled={busy} onClick={onClose} type="button" variant="secondary">Cancel</Button>
            <Button disabled={busy || !accountLabel.trim() || accountLabel.trim() === connection.account_label} type="submit">
              {busy ? <Loader2 className="size-4 animate-spin" /> : null}{busy ? "Saving…" : "Save changes"}
            </Button>
          </DialogFooter>
        </form>
      </DialogShell>
    );
  }

  if (drawer.kind === "rotate") {
    return (
      <DialogShell onClose={close} open>
        <DialogHeader onClose={busy ? undefined : onClose} title="Replace API key" />
        <form onSubmit={rotate}>
          <DialogBody className="space-y-6">
            <ConnectionIdentity connection={connection} />
            <DrawerIntro icon={<RotateCw className="size-5" />} text="The saved key is replaced immediately. Test the connection before relying on it for Agent runs." />
            <Field label="New API key" labelFor="replacement-api-key">
              <Input autoComplete="new-password" data-autofocus="true" id="replacement-api-key" onChange={(event) => setApiKey(event.target.value)} required type="password" value={apiKey} />
              <FieldHint>The existing key cannot be viewed. This replacement will not be shown again.</FieldHint>
            </Field>
            <FormError message={formError} />
          </DialogBody>
          <DialogFooter>
            <Button disabled={busy} onClick={onClose} type="button" variant="secondary">Cancel</Button>
            <Button disabled={busy || !apiKey.trim()} type="submit">
              {busy ? <Loader2 className="size-4 animate-spin" /> : <KeyRound className="size-4" />}{busy ? "Replacing…" : "Replace key"}
            </Button>
          </DialogFooter>
        </form>
      </DialogShell>
    );
  }

  if (drawer.kind === "approval") {
    const currentApproval = approvalConnection;
    const trimmedReasonLength = approvalReason.trim().length;
    return (
      <DialogShell onClose={close} open>
        <DialogHeader onClose={busy ? undefined : onClose} title="Provider data approval" />
        <form onSubmit={setDataApproval}>
          <DialogBody className="space-y-6">
            <ConnectionIdentity connection={connection} />
            {approvalLoading ? (
              <div className="flex min-h-44 items-center justify-center gap-3 text-sm text-[var(--text-muted)]" role="status">
                <Loader2 className="size-4 animate-spin" />Loading current approval…
              </div>
            ) : approvalLoadError ? (
              <TableError
                description={approvalLoadError}
                onRetry={() => setApprovalReload((current) => current + 1)}
                title="Data approval could not be loaded"
              />
            ) : currentApproval ? (
              <>
                <section aria-labelledby="current-provider-approval">
                  <p className="text-[11px] font-semibold uppercase tracking-[0.14em] text-[var(--text-muted)]" id="current-provider-approval">
                    Current decision
                  </p>
                  <div className="mt-3 border border-[var(--border)] bg-[var(--surface-muted)] p-4">
                    <div className="flex flex-wrap items-center gap-2">
                      <Badge tone={approvalTone(currentApproval.provider_data_approval_class)}>
                        {approvalLabel(currentApproval.provider_data_approval_class)}
                      </Badge>
                      <span className="text-xs text-[var(--text-muted)]">
                        Decision {currentApproval.provider_data_approval_version}
                      </span>
                    </div>
                    <p className="mt-3 text-sm leading-6 text-[var(--text-body)]">
                      {approvalDescription(currentApproval.provider_data_approval_class)}
                    </p>
                    <p className="mt-2 text-xs text-[var(--text-muted)]">
                      {environmentLabel(currentApproval.execution_environment_class)}
                    </p>
                  </div>
                </section>

                <DrawerIntro
                  icon={<ShieldCheck className="size-5" />}
                  text="Saving records a new decision. Routes using the previous decision must be updated before they can run again."
                />

                <fieldset disabled={busy}>
                  <legend className="text-sm font-medium leading-none text-[var(--text-strong)]">Allow this connection to receive</legend>
                  <div className="mt-3 space-y-3">
                    <ApprovalChoice
                      checked={approvalClass === "campus_approved"}
                      description="General and personal campus data. Sensitive and highly sensitive data remain blocked."
                      id="provider-approval-campus"
                      label="Campus data"
                      onChange={() => setApprovalClass("campus_approved")}
                      value="campus_approved"
                    />
                    <ApprovalChoice
                      checked={approvalClass === "sensitive_data_approved"}
                      description="General, personal, and sensitive campus data. Highly sensitive data still requires an installation-local provider."
                      id="provider-approval-sensitive"
                      label="Sensitive campus data"
                      onChange={() => setApprovalClass("sensitive_data_approved")}
                      value="sensitive_data_approved"
                    />
                  </div>
                </fieldset>

                <Field label="Reason for approval" labelFor="provider-approval-reason">
                  <Textarea
                    aria-describedby="provider-approval-reason-hint"
                    id="provider-approval-reason"
                    maxLength={500}
                    onChange={(event) => setApprovalReason(event.target.value)}
                    placeholder="Why may this provider receive the selected data class?"
                    required
                    rows={5}
                    value={approvalReason}
                  />
                  <p className="mt-2 flex justify-between gap-4 text-xs leading-5 text-[var(--text-muted)]" id="provider-approval-reason-hint">
                    <span>This reason is retained with the decision.</span>
                    <span>{approvalReason.length}/500</span>
                  </p>
                </Field>
                <FormError message={formError} />
                {formError ? (
                  <Button
                    disabled={busy}
                    onClick={() => {
                      setFormError(null);
                      setApprovalReload((current) => current + 1);
                    }}
                    size="sm"
                    type="button"
                    variant="secondary"
                  >
                    <RefreshCw className="size-3.5" />Reload current decision
                  </Button>
                ) : null}
              </>
            ) : (
              <div className="border border-dashed border-[var(--border)] p-6 text-center">
                <p className="text-sm font-semibold text-[var(--text-strong)]">Connection unavailable</p>
                <p className="mt-1 text-sm text-[var(--text-muted)]">Close this drawer and reload the provider list.</p>
              </div>
            )}
          </DialogBody>
          <DialogFooter>
            <Button disabled={busy} onClick={close} type="button" variant="secondary">Cancel</Button>
            <Button
              disabled={busy || approvalLoading || Boolean(approvalLoadError) || !currentApproval || !approvalClass || trimmedReasonLength < 3}
              type="submit"
            >
              {busy ? <Loader2 className="size-4 animate-spin" /> : <ShieldCheck className="size-4" />}
              {busy ? "Saving…" : "Save approval"}
            </Button>
          </DialogFooter>
        </form>
      </DialogShell>
    );
  }

  if (drawer.kind === "test") {
    const visibleOutcome = testOutcome ?? (connection.last_test_status && connection.last_tested_at ? {
      status: connection.last_test_status,
      failure_category: connection.last_failure_category,
      tested_at: connection.last_tested_at,
    } : null);
    return (
      <DialogShell onClose={close} open>
        <DialogHeader onClose={busy ? undefined : onClose} title="Test connection" />
        <DialogBody className="space-y-6">
          <ConnectionIdentity connection={connection} />
          <DrawerIntro icon={<CheckCircle2 className="size-5" />} text="The test verifies the saved credential without sending campus records to the provider." />
          {visibleOutcome ? (
            <div className={`border p-4 ${visibleOutcome.status === "succeeded" ? "border-[var(--tone-success-bd)] bg-[var(--tone-success-bg)]" : "border-[var(--tone-danger-bd)] bg-[var(--tone-danger-bg)]"}`} role="status">
              <p className={`font-semibold ${visibleOutcome.status === "succeeded" ? "text-[var(--tone-success-strong)]" : "text-[var(--tone-danger-strong)]"}`}>
                {visibleOutcome.status === "succeeded" ? "Connection test passed" : "Connection test failed"}
              </p>
              <p className="mt-1 text-sm text-[var(--text-muted)]">{formatDate(visibleOutcome.tested_at)}</p>
              {visibleOutcome.failure_category ? <p className="mt-2 text-sm text-[var(--tone-danger-strong)]">{friendlyFailure(visibleOutcome.failure_category)}</p> : null}
            </div>
          ) : <p className="text-sm text-[var(--text-muted)]">This connection has not been tested.</p>}
          <FormError message={formError} />
        </DialogBody>
        <DialogFooter>
          <Button disabled={busy} onClick={onClose} type="button" variant="secondary">Close</Button>
          <Button disabled={busy} onClick={test} type="button">
            {busy ? <Loader2 className="size-4 animate-spin" /> : <CheckCircle2 className="size-4" />}{busy ? "Testing…" : "Run connection test"}
          </Button>
        </DialogFooter>
      </DialogShell>
    );
  }

  if (drawer.kind === "models") {
    return (
      <DialogShell onClose={close} open panelClassName="sm:max-w-[720px]">
        <DialogHeader onClose={busy ? undefined : onClose} title="Provider models" />
        <DialogBody className="space-y-6">
          <ConnectionIdentity connection={connection} />
          {modelsLoading ? <div className="py-10"><TableLoading columns={3} label="Loading cached models…" rows={5} /></div> : null}
          {!modelsLoading && modelsError ? <TableError description={modelsError} onRetry={() => setModelsReload((current) => current + 1)} title="Models could not be loaded" /> : null}
          {!modelsLoading && !modelsError && models ? (
            <>
              <div className="flex flex-wrap items-center justify-between gap-3 border-b border-[var(--border)] pb-3">
                <p className="text-sm font-semibold text-[var(--text-strong)]">{models.models.length} cached models</p>
                <p className="text-xs text-[var(--text-muted)]">{models.refreshed_at ? `Refreshed ${formatDate(models.refreshed_at)}` : "Not refreshed"}</p>
              </div>
              {models.models.length === 0 ? (
                <div className="border border-dashed border-[var(--border)] p-6 text-center">
                  <p className="text-sm font-semibold text-[var(--text-strong)]">No cached models</p>
                  <p className="mt-1 text-sm text-[var(--text-muted)]">Refresh the catalogue after this connection passes its test.</p>
                </div>
              ) : (
                <ul className="divide-y divide-[var(--border-subtle)] border-y border-[var(--border-subtle)]">
                  {models.models.map((model) => (
                    <li className="grid gap-2 py-4 sm:grid-cols-[minmax(0,1fr)_auto] sm:items-center" key={model.id}>
                      <div className="min-w-0">
                        <p className="truncate text-sm font-semibold text-[var(--text-strong)]">{model.display_name}</p>
                        <p className="mt-0.5 truncate text-xs text-[var(--text-muted)]">{model.id}</p>
                      </div>
                      <div className="flex flex-wrap gap-2">
                        {model.context_window_tokens ? <Badge tone="neutral">{formatTokens(model.context_window_tokens)} context</Badge> : null}
                        {model.supports_tools === true ? <Badge tone="info">Tools</Badge> : null}
                        {model.supports_tools === false ? <Badge tone="neutral">No tools</Badge> : null}
                      </div>
                    </li>
                  ))}
                </ul>
              )}
            </>
          ) : null}
          {connection.status !== "ready" ? <FieldHint>Run a successful connection test before refreshing models.</FieldHint> : null}
          <FormError message={formError} />
        </DialogBody>
        <DialogFooter>
          <Button disabled={busy} onClick={onClose} type="button" variant="secondary">Close</Button>
          {canEdit ? (
            <Button disabled={busy || connection.status !== "ready" || connectionProvider?.supports_model_refresh === false} onClick={refreshModels} type="button">
              {busy ? <Loader2 className="size-4 animate-spin" /> : <RefreshCw className="size-4" />}{busy ? "Refreshing…" : "Refresh models"}
            </Button>
          ) : null}
        </DialogFooter>
      </DialogShell>
    );
  }

  return (
    <DialogShell onClose={close} open>
      <DialogHeader onClose={busy ? undefined : onClose} title="Disconnect provider" />
      <DialogBody className="space-y-6">
        <ConnectionIdentity connection={connection} />
        <DrawerIntro danger icon={<Trash2 className="size-5" />} text="Disconnecting removes this campus connection. It will be refused while an Agent route still uses it." />
        <p className="text-sm leading-6 text-[var(--text-muted)]">To reconnect later, an administrator must enter a new API key.</p>
        <FormError message={formError} />
      </DialogBody>
      <DialogFooter>
        <Button data-autofocus="true" disabled={busy} onClick={onClose} type="button" variant="secondary">Keep connection</Button>
        <Button disabled={busy} onClick={disconnect} type="button" variant="destructive">
          {busy ? <Loader2 className="size-4 animate-spin" /> : <Trash2 className="size-4" />}{busy ? "Disconnecting…" : "Disconnect"}
        </Button>
      </DialogFooter>
    </DialogShell>
  );
}

function Metric({ label, value }: { label: string; value: number }) {
  return <div className="border-r border-[var(--border-subtle)] px-4 py-3 last:border-r-0"><dt className="text-[10px] font-semibold uppercase tracking-[0.12em] text-[var(--text-muted)]">{label}</dt><dd className="mt-1 text-lg font-semibold text-[var(--text-strong)]">{value}</dd></div>;
}

function Field({ children, label, labelFor }: { children: React.ReactNode; label: string; labelFor: string }) {
  return <div><Label htmlFor={labelFor}>{label}</Label><div className="mt-2">{children}</div></div>;
}

function FieldHint({ children }: { children: React.ReactNode }) {
  return <p className="mt-2 text-xs leading-5 text-[var(--text-muted)]">{children}</p>;
}

function DrawerIntro({ danger = false, icon, text }: { danger?: boolean; icon: React.ReactNode; text: string }) {
  return <div className={`flex items-start gap-3 border p-4 ${danger ? "border-[var(--tone-danger-bd)] bg-[var(--tone-danger-bg)]" : "border-[var(--brand-100)] bg-[var(--brand-soft)]"}`}><span className={`mt-0.5 shrink-0 ${danger ? "text-[var(--tone-danger-strong)]" : "text-[var(--brand-strong)]"}`}>{icon}</span><p className={`text-sm leading-6 ${danger ? "text-[var(--tone-danger-strong)]" : "text-[var(--text-body)]"}`}>{text}</p></div>;
}

function ConnectionIdentity({ connection }: { connection: AiProviderConnection }) {
  return <div className="flex items-start gap-3 border-b border-[var(--border)] pb-5"><span className="flex size-10 shrink-0 items-center justify-center rounded-[9px] bg-[var(--brand-soft)] text-[var(--brand-strong)]"><Bot className="size-[18px]" /></span><div className="min-w-0"><p className="truncate font-semibold text-[var(--text-strong)]">{connection.account_label}</p><p className="mt-0.5 text-sm text-[var(--text-muted)]">{connection.provider_label}</p></div><Badge className="ml-auto" dot tone={statusTone(connection.status)}>{statusLabel(connection.status)}</Badge></div>;
}

function ApprovalChoice({
  checked,
  description,
  id,
  label,
  onChange,
  value,
}: {
  checked: boolean;
  description: string;
  id: string;
  label: string;
  onChange: () => void;
  value: Exclude<AiProviderDataApprovalClass, "unapproved">;
}) {
  return (
    <label
      className={`flex cursor-pointer items-start gap-3 border p-4 transition-colors ${checked ? "border-[var(--brand-strong)] bg-[var(--brand-soft)]" : "border-[var(--border)] bg-[var(--surface)] hover:border-[var(--border-strong)]"}`}
      htmlFor={id}
    >
      <input
        checked={checked}
        className="mt-1 size-4 accent-[var(--brand-strong)] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--focus-ring)]"
        id={id}
        name="provider-data-approval"
        onChange={onChange}
        type="radio"
        value={value}
      />
      <span>
        <span className="block text-sm font-semibold text-[var(--text-strong)]">{label}</span>
        <span className="mt-1 block text-xs leading-5 text-[var(--text-muted)]">{description}</span>
      </span>
    </label>
  );
}

function FormError({ message }: { message: string | null }) {
  return message ? <div className="flex items-start gap-3 border border-[var(--tone-danger-bd)] bg-[var(--tone-danger-bg)] p-4 text-sm leading-5 text-[var(--tone-danger-strong)]" role="alert"><AlertTriangle className="mt-0.5 size-4 shrink-0" /><span>{message}</span></div> : <div aria-live="polite" className="sr-only" />;
}

function hasPermission(permissions: string[] | undefined, permission: string) {
  return permissions?.includes("*") || permissions?.includes(permission) || false;
}

function statusLabel(status: AiProviderConnectionStatus) {
  if (status === "ready") return "Ready";
  if (status === "error") return "Needs attention";
  return "Not tested";
}

function statusTone(status: AiProviderConnectionStatus): "success" | "danger" | "neutral" {
  if (status === "ready") return "success";
  if (status === "error") return "danger";
  return "neutral";
}

function approvalLabel(approvalClass: AiProviderDataApprovalClass) {
  if (approvalClass === "sensitive_data_approved") return "Sensitive data approved";
  if (approvalClass === "campus_approved") return "Campus data approved";
  return "Data not approved";
}

function approvalDescription(approvalClass: AiProviderDataApprovalClass) {
  if (approvalClass === "sensitive_data_approved") {
    return "This connection may receive general, personal, and sensitive campus data. Highly sensitive data remains blocked for provider-managed connections.";
  }
  if (approvalClass === "campus_approved") {
    return "This connection may receive general and personal campus data. Sensitive and highly sensitive data remain blocked.";
  }
  return "This connection cannot receive campus data for Agent runs.";
}

function approvalTone(approvalClass: AiProviderDataApprovalClass): "info" | "warn" | "danger" {
  if (approvalClass === "sensitive_data_approved") return "warn";
  if (approvalClass === "campus_approved") return "info";
  return "danger";
}

function environmentLabel(environment: AiProviderConnection["execution_environment_class"]) {
  return environment === "installation_local"
    ? "Runs inside this Campus Pilot installation."
    : "Runs in the provider-managed environment.";
}

function friendlyFailure(value: string) {
  const labels: Record<string, string> = {
    authentication: "The provider rejected the saved credential.",
    authorization: "The saved credential does not have the required access.",
    rate_limited: "The provider temporarily limited requests.",
    unavailable: "The provider is temporarily unavailable.",
    timeout: "The provider did not respond in time.",
    invalid_response: "The provider returned an invalid response.",
  };
  return labels[value] ?? "The provider connection needs attention.";
}

function formatTokens(value: number) {
  return new Intl.NumberFormat("en", { notation: "compact", maximumFractionDigits: 1 }).format(value);
}

function drawerKey(drawer: ProviderDrawer) {
  return drawer ? `${drawer.kind}:${"connection" in drawer ? drawer.connection.id : "new"}` : "closed";
}

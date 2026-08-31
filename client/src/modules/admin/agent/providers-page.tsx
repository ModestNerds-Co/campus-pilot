/**
 * Administration workspace for tenant-scoped, write-only AI provider connections.
 * All focused and destructive workflows use the shared accessible right-side drawer.
 */

import React, { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  AlertTriangle,
  Bot,
  CheckCircle2,
  Copy,
  ExternalLink,
  KeyRound,
  Link2,
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
  AiApiKeyProviderKey,
  AiDeviceCodeProviderKey,
  AiOAuthProviderKey,
  AiProviderConnection,
  AiProviderConnectionStatus,
  AiProviderDataApprovalClass,
  AiProviderKey,
  ProviderCatalogEntry,
  ProviderDeviceCodeStart,
  ProviderModelSnapshot,
  ProviderOAuthStart,
  ProviderTestOutcome,
} from "./types";

type ProviderDrawer =
  | { kind: "connect"; initialProvider?: AiProviderKey; reconnectConnectionId?: string }
  | { kind: "update" | "rotate" | "test" | "models" | "approval" | "disconnect"; connection: AiProviderConnection }
  | null;

type ProviderOption = {
  key: AiProviderKey;
  label: string;
  detail: string;
  authMethod: "api_key" | "oauth" | "device_code";
  mark: string;
};

const SUBSCRIPTION_PROVIDERS: ProviderOption[] = [
  {
    key: "codex",
    label: "ChatGPT",
    detail: "Connect a ChatGPT subscription through Codex.",
    authMethod: "oauth",
    mark: "GPT",
  },
  {
    key: "claude_code",
    label: "Claude.ai",
    detail: "Connect a Claude.ai subscription through Claude Code.",
    authMethod: "oauth",
    mark: "AI",
  },
  {
    key: "kimi_code",
    label: "Kimi Code",
    detail: "Connect a Kimi Code subscription with device login.",
    authMethod: "device_code",
    mark: "Kimi",
  },
];

const API_KEY_PROVIDERS: ProviderOption[] = [
  { key: "openai", label: "OpenAI API", detail: "Use a campus-owned OpenAI API key.", authMethod: "api_key", mark: "OA" },
  { key: "anthropic", label: "Anthropic API", detail: "Use a campus-owned Anthropic API key.", authMethod: "api_key", mark: "A" },
  { key: "openrouter", label: "OpenRouter API", detail: "Use one key to reach approved OpenRouter models.", authMethod: "api_key", mark: "OR" },
];

const PROVIDER_OPTIONS = [...SUBSCRIPTION_PROVIDERS, ...API_KEY_PROVIDERS];

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
                  onReconnect={() => setDrawer({
                    kind: "connect",
                    initialProvider: connection.provider,
                    reconnectConnectionId: connection.id,
                  })}
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
        onConnectionCompleted={() => {
          setDrawer(null);
          void load(true);
        }}
        onModelsRefreshed={() => void load(true)}
        onUpdated={replaceConnection}
        connections={connections}
        providers={providers}
      />
    </div>
  );
}

function ConnectionRow({
  canEdit,
  connection,
  onOpen,
  onReconnect,
  provider,
}: {
  canEdit: boolean;
  connection: AiProviderConnection;
  onOpen: (kind: Exclude<ProviderDrawer, null | { kind: "connect" }>["kind"]) => void;
  onReconnect: () => void;
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
            {connection.auth_method === "api_key" ? (
              <Button aria-label={`Rotate API key for ${connection.account_label}`} onClick={() => onOpen("rotate")} size="icon-sm" variant="ghost">
                <RotateCw className="size-3.5" />
              </Button>
            ) : (
              <Button onClick={onReconnect} size="sm" variant="ghost">
                <ExternalLink className="size-3.5" />Reconnect
              </Button>
            )}
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
  connections,
  drawer,
  onClose,
  onConnectionCompleted,
  onDisconnected,
  onApprovalSaved,
  onModelsRefreshed,
  onUpdated,
  providers,
}: {
  canEdit: boolean;
  connections: AiProviderConnection[];
  drawer: ProviderDrawer;
  onClose: () => void;
  onConnectionCompleted: () => void;
  onDisconnected: (connectionId: string) => void;
  onApprovalSaved: () => void;
  onModelsRefreshed: () => void;
  onUpdated: (connection: AiProviderConnection) => void;
  providers: ProviderCatalogEntry[];
}) {
  const connection = drawer && "connection" in drawer ? drawer.connection : null;
  const connectionProvider = providers.find((provider) => provider.key === connection?.provider);
  const initialProvider = drawer?.kind === "connect" ? drawer.initialProvider ?? null : null;
  const reconnectConnectionId = drawer?.kind === "connect" ? drawer.reconnectConnectionId : undefined;
  const [providerKey, setProviderKey] = useState<AiProviderKey | null>(initialProvider);
  const [accountLabel, setAccountLabel] = useState(connection?.account_label ?? "");
  const [apiKey, setApiKey] = useState("");
  const [oauthFlow, setOauthFlow] = useState<(ProviderOAuthStart & { callbackValue: string }) | null>(null);
  const [deviceFlow, setDeviceFlow] = useState<ProviderDeviceCodeStart | null>(null);
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
    if (!providerKey || !isApiKeyProvider(providerKey) || !accountLabel.trim() || !apiKey.trim()) {
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

  const startOAuth = (provider: AiOAuthProviderKey) => {
    const popup = window.open("about:blank", "_blank");
    if (popup) popup.opener = null;
    void run(async () => {
      const response = await aiProviderService.startOAuth(provider, reconnectConnectionId);
      if (!response.success || !response.data) {
        popup?.close();
        setFormError(aiProviderErrorMessage(response, "The provider login could not be started."));
        return;
      }
      setOauthFlow({ ...response.data, callbackValue: "" });
      setDeviceFlow(null);
      if (popup) popup.location.assign(response.data.authorize_url);
    });
  };

  const completeOAuth = (event: React.FormEvent) => {
    event.preventDefault();
    if (!oauthFlow?.callbackValue.trim()) {
      setFormError("Paste the provider callback before finishing the connection.");
      return;
    }
    void run(async () => {
      const response = await aiProviderService.completeOAuth({
        attempt_id: oauthFlow.attempt_id,
        callback_value: oauthFlow.callbackValue.trim(),
      });
      if (!response.success) {
        setFormError(aiProviderErrorMessage(response, "The provider login could not be completed."));
        return;
      }
      toast.success(`${providerLabel(oauthFlow.provider)} connected`);
      onConnectionCompleted();
    });
  };

  const startDeviceCode = (provider: AiDeviceCodeProviderKey) => {
    const popup = window.open("about:blank", "_blank");
    if (popup) popup.opener = null;
    void run(async () => {
      const response = await aiProviderService.startDeviceCode(provider, reconnectConnectionId);
      if (!response.success || !response.data) {
        popup?.close();
        setFormError(aiProviderErrorMessage(response, "The Kimi Code login could not be started."));
        return;
      }
      setDeviceFlow(response.data);
      setOauthFlow(null);
      if (popup) popup.location.assign(response.data.verification_uri_complete);
    });
  };

  const pollDeviceCode = () => {
    if (!deviceFlow) return;
    void run(async () => {
      const response = await aiProviderService.pollDeviceCode(deviceFlow.attempt_id);
      if (!response.success || !response.data) {
        setFormError(aiProviderErrorMessage(response, "Kimi Code approval could not be checked."));
        return;
      }
      if (response.data.status === "connected") {
        toast.success("Kimi Code connected");
        onConnectionCompleted();
        return;
      }
      if (response.data.status === "pending") {
        toast("Kimi Code is still waiting for approval");
        return;
      }
      setDeviceFlow(null);
      setFormError(response.data.status === "expired"
        ? "The Kimi Code login expired. Start it again."
        : "The Kimi Code login was denied. Start it again if this was a mistake.");
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
    const selectedOption = PROVIDER_OPTIONS.find((provider) => provider.key === providerKey);
    const selectedCatalog = providers.find((provider) => provider.key === providerKey);
    const selectedAvailable = selectedOption
      ? providerIsAvailable(selectedOption, selectedCatalog)
      : false;
    const selectedSetupReason = selectedOption
      ? providerSetupReason(selectedOption, selectedCatalog)
      : null;
    const isReconnect = Boolean(reconnectConnectionId && providerKey === initialProvider);

    if (oauthFlow) {
      const isCodex = oauthFlow.provider === "codex";
      return (
        <DialogShell onClose={close} open>
          <DialogHeader onClose={busy ? undefined : onClose} title={`Finish connecting ${providerLabel(oauthFlow.provider)}`} />
          <form onSubmit={completeOAuth}>
            <DialogBody className="space-y-6">
              <DrawerIntro
                icon={<Link2 className="size-5" />}
                text={isCodex
                  ? "After approval reaches localhost, copy the complete URL from the address bar and paste it here."
                  : "After approval, copy the complete code#state value shown by Claude and paste it here."}
              />
              <a
                className="inline-flex min-h-11 items-center gap-2 rounded-[var(--radius-md)] border border-[var(--border)] bg-[var(--surface)] px-4 text-sm font-semibold text-[var(--text-strong)] hover:bg-[var(--surface-muted)] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--focus-ring)]"
                href={oauthFlow.authorize_url}
                rel="noreferrer"
                target="_blank"
              >
                Open {providerLabel(oauthFlow.provider)} <ExternalLink className="size-4" />
              </a>
              <Field label={isCodex ? "Localhost callback URL" : "Claude authorization code"} labelFor="provider-oauth-callback">
                <Input
                  autoComplete="off"
                  data-autofocus="true"
                  id="provider-oauth-callback"
                  onChange={(event) => setOauthFlow((current) => current ? { ...current, callbackValue: event.target.value } : null)}
                  placeholder={isCodex ? "http://localhost:1455/auth/callback?code=…&state=…" : "code#state"}
                  required
                  value={oauthFlow.callbackValue}
                />
              </Field>
              <FormError message={formError} />
            </DialogBody>
            <DialogFooter>
              <Button disabled={busy} onClick={onClose} type="button" variant="secondary">Cancel</Button>
              <Button disabled={busy || !oauthFlow.callbackValue.trim()} type="submit">
                {busy ? <Loader2 className="size-4 animate-spin" /> : <Link2 className="size-4" />}
                {busy ? "Connecting…" : "Finish connection"}
              </Button>
            </DialogFooter>
          </form>
        </DialogShell>
      );
    }

    if (deviceFlow) {
      return (
        <DialogShell onClose={close} open>
          <DialogHeader onClose={busy ? undefined : onClose} title="Finish connecting Kimi Code" />
          <DialogBody className="space-y-6">
            <DrawerIntro icon={<ExternalLink className="size-5" />} text="Approve the Kimi Code login in the provider window, then check its status here." />
            <div className="border border-[var(--border)] bg-[var(--surface-muted)] p-5">
              <p className="text-[11px] font-semibold uppercase tracking-[0.14em] text-[var(--text-muted)]">Verification code</p>
              <div className="mt-2 flex flex-wrap items-center justify-between gap-3">
                <code className="text-xl font-semibold tracking-[0.12em] text-[var(--text-strong)]">{deviceFlow.user_code}</code>
                <Button
                  onClick={() => void navigator.clipboard.writeText(deviceFlow.user_code)
                    .then(() => toast.success("Verification code copied"))
                    .catch(() => toast.error("Verification code could not be copied"))}
                  size="sm"
                  type="button"
                  variant="secondary"
                >
                  <Copy className="size-3.5" />Copy code
                </Button>
              </div>
            </div>
            <a
              className="inline-flex min-h-11 items-center gap-2 rounded-[var(--radius-md)] border border-[var(--border)] bg-[var(--surface)] px-4 text-sm font-semibold text-[var(--text-strong)] hover:bg-[var(--surface-muted)] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--focus-ring)]"
              href={deviceFlow.verification_uri_complete}
              rel="noreferrer"
              target="_blank"
            >
              Open Kimi Code login <ExternalLink className="size-4" />
            </a>
            <FieldHint>Approval checks may be repeated every {Math.max(deviceFlow.interval, 1)} seconds.</FieldHint>
            <FormError message={formError} />
          </DialogBody>
          <DialogFooter>
            <Button disabled={busy} onClick={onClose} type="button" variant="secondary">Cancel</Button>
            <Button disabled={busy} onClick={pollDeviceCode} type="button">
              {busy ? <Loader2 className="size-4 animate-spin" /> : <CheckCircle2 className="size-4" />}
              {busy ? "Checking…" : "Check approval"}
            </Button>
          </DialogFooter>
        </DialogShell>
      );
    }

    const submitConnection = (event: React.FormEvent) => {
      if (!providerKey) {
        event.preventDefault();
        setFormError("Choose a provider.");
        return;
      }
      if (!selectedOption || !selectedAvailable) {
        event.preventDefault();
        setFormError(selectedSetupReason || "Server setup required");
        return;
      }
      if (isApiKeyProvider(providerKey)) {
        create(event);
        return;
      }
      event.preventDefault();
      if (isOAuthProvider(providerKey)) startOAuth(providerKey);
      else startDeviceCode(providerKey);
    };

    return (
      <DialogShell onClose={close} open>
        <DialogHeader onClose={busy ? undefined : onClose} title={isReconnect && selectedOption ? `Reconnect ${selectedOption.label}` : "Connect provider"} />
        <form onSubmit={submitConnection}>
          <DialogBody className="space-y-6">
            <DrawerIntro icon={<Plug className="size-5" />} text="Choose a subscription login or a campus-owned API key." />
            <ProviderOptionGroup
              catalog={providers}
              connectedProviders={connections.map((item) => item.provider)}
              label="Subscription accounts"
              onSelect={(key) => {
                setProviderKey(key);
                setFormError(null);
              }}
              options={SUBSCRIPTION_PROVIDERS}
              selected={providerKey}
            />
            <ProviderOptionGroup
              catalog={providers}
              connectedProviders={connections.map((item) => item.provider)}
              label="API keys"
              onSelect={(key) => {
                setProviderKey(key);
                setFormError(null);
              }}
              options={API_KEY_PROVIDERS}
              selected={providerKey}
            />
            {providerKey && isApiKeyProvider(providerKey) && selectedAvailable ? (
              <div className="space-y-5 border-t border-[var(--border)] pt-6">
                <Field label="Account label" labelFor="provider-account-label">
                  <Input data-autofocus="true" id="provider-account-label" onChange={(event) => setAccountLabel(event.target.value)} placeholder={`e.g. School ${selectedOption?.label ?? "provider"} account`} required value={accountLabel} />
                  <FieldHint>Use a label administrators will recognize when configuring routes.</FieldHint>
                </Field>
                <Field label="API key" labelFor="provider-api-key">
                  <Input autoComplete="new-password" id="provider-api-key" onChange={(event) => setApiKey(event.target.value)} required type="password" value={apiKey} />
                  <FieldHint>{selectedCatalog?.credential_hint || "Enter the key issued by the provider."} It will not be shown again.</FieldHint>
                </Field>
              </div>
            ) : providerKey && selectedOption ? (
              <div className={`border p-4 ${selectedAvailable ? "border-[var(--brand-100)] bg-[var(--brand-soft)]" : "border-[var(--tone-warn-bd)] bg-[var(--tone-warn-bg)]"}`}>
                <p className="text-sm font-semibold text-[var(--text-strong)]">{isReconnect ? "Reconnect" : "Connect"} {selectedOption.label}</p>
                <p className="mt-1 text-xs leading-5 text-[var(--text-muted)]">
                  {!selectedAvailable
                    ? selectedSetupReason || "Server setup required"
                    : selectedOption.authMethod === "device_code"
                    ? "A Kimi Code login page will open and provide a verification code."
                    : "The provider approval page will open in a new browser tab."}
                </p>
              </div>
            ) : null}
            <FormError message={formError} />
          </DialogBody>
          <DialogFooter>
            <Button disabled={busy} onClick={onClose} type="button" variant="secondary">Cancel</Button>
            <Button disabled={busy || !providerKey || !selectedAvailable || (isApiKeyProvider(providerKey) && (!accountLabel.trim() || !apiKey.trim()))} type="submit">
              {busy ? <Loader2 className="size-4 animate-spin" /> : providerKey && isApiKeyProvider(providerKey) ? <KeyRound className="size-4" /> : <ExternalLink className="size-4" />}
              {busy ? "Connecting…" : !selectedAvailable && providerKey ? "Server setup required" : providerKey && isApiKeyProvider(providerKey) ? "Save connection" : selectedOption ? `${isReconnect ? "Reconnect" : "Connect"} ${selectedOption.label}` : "Choose provider"}
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
        <p className="text-sm leading-6 text-[var(--text-muted)]">
          {connection.auth_method === "api_key"
            ? "To reconnect later, an administrator must enter a new API key."
            : "To reconnect later, an administrator must complete the provider login again."}
        </p>
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

function ProviderOptionGroup({
  catalog,
  connectedProviders,
  label,
  onSelect,
  options,
  selected,
}: {
  catalog: ProviderCatalogEntry[];
  connectedProviders: AiProviderKey[];
  label: string;
  onSelect: (key: AiProviderKey) => void;
  options: ProviderOption[];
  selected: AiProviderKey | null;
}) {
  return (
    <fieldset>
      <legend className="text-[11px] font-semibold uppercase tracking-[0.14em] text-[var(--text-muted)]">{label}</legend>
      <div className="mt-3 grid gap-3 sm:grid-cols-2">
        {options.map((option) => {
          const entry = catalog.find((item) => item.key === option.key);
          const available = providerIsAvailable(option, entry);
          const connected = connectedProviders.includes(option.key);
          const active = selected === option.key;
          return (
            <button
              aria-pressed={active}
              className={`min-h-[116px] border p-4 text-left transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--focus-ring)] ${active ? "border-[var(--brand-strong)] bg-[var(--brand-soft)]" : "border-[var(--border)] bg-[var(--surface)] hover:border-[var(--border-strong)]"}`}
              key={option.key}
              onClick={() => onSelect(option.key)}
              type="button"
            >
              <span className="flex items-start gap-3">
                <span className={`flex size-9 shrink-0 items-center justify-center rounded-[8px] text-[11px] font-bold ${available ? "bg-[var(--brand-soft)] text-[var(--brand-strong)]" : "bg-[var(--surface-muted)] text-[var(--text-muted)]"}`}>
                  {option.mark}
                </span>
                <span className="min-w-0 flex-1">
                  <span className="flex flex-wrap items-center gap-2">
                    <span className="text-sm font-semibold text-[var(--text-strong)]">{option.label}</span>
                    {connected ? <Badge tone="info">Connected</Badge> : null}
                  </span>
                  <span className="mt-1 block text-xs leading-5 text-[var(--text-muted)]">
                    {available ? option.detail : providerSetupReason(option, entry)}
                  </span>
                </span>
              </span>
            </button>
          );
        })}
      </div>
    </fieldset>
  );
}

function isApiKeyProvider(provider: AiProviderKey): provider is AiApiKeyProviderKey {
  return provider === "openai" || provider === "anthropic" || provider === "openrouter";
}

function isOAuthProvider(provider: AiProviderKey): provider is AiOAuthProviderKey {
  return provider === "codex" || provider === "claude_code";
}

function providerIsAvailable(option: ProviderOption, entry: ProviderCatalogEntry | undefined) {
  if (!entry) return false;
  if (option.authMethod === "api_key") {
    return entry.auth_methods.includes("api_key") && entry.available !== false;
  }
  return entry.auth_methods.includes("subscription_oauth") && entry.available === true;
}

function providerSetupReason(option: ProviderOption, entry: ProviderCatalogEntry | undefined) {
  if (entry?.setup_reason?.trim()) return entry.setup_reason;
  return option.authMethod === "api_key" && !entry
    ? "Provider unavailable"
    : "Server setup required";
}

function providerLabel(provider: AiProviderKey) {
  return PROVIDER_OPTIONS.find((option) => option.key === provider)?.label ?? provider;
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
  if (status === "needs_reconnect") return "Reconnect required";
  if (status === "error") return "Needs attention";
  return "Not tested";
}

function statusTone(status: AiProviderConnectionStatus): "success" | "danger" | "neutral" {
  if (status === "ready") return "success";
  if (status === "error" || status === "needs_reconnect") return "danger";
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

/**
 * Secret-free Administration contracts for campus AI provider connections.
 * Credential input exists only in mutation payloads and is never part of a response type.
 */

export type AiApiKeyProviderKey = "openai" | "anthropic" | "openrouter";
export type AiOAuthProviderKey = "codex" | "claude_code";
export type AiDeviceCodeProviderKey = "kimi_code";
export type AiSubscriptionProviderKey = AiOAuthProviderKey | AiDeviceCodeProviderKey;
export type AiProviderKey = AiApiKeyProviderKey | AiSubscriptionProviderKey;
export type AiProviderAuthMethod = "api_key" | "subscription_oauth";
export type AiProviderConnectionStatus = "untested" | "ready" | "error" | "needs_reconnect";
export type AiProviderTestStatus = "succeeded" | "failed";
export type AiProviderDataApprovalClass =
  | "unapproved"
  | "campus_approved"
  | "sensitive_data_approved";
export type AiProviderExecutionEnvironmentClass = "external_managed" | "installation_local";

export interface ProviderCatalogEntry {
  key: AiProviderKey;
  label: string;
  auth_methods: AiProviderAuthMethod[];
  available?: boolean;
  setup_reason?: string | null;
  credential_hint: string;
  supports_connection_test: boolean;
  supports_model_refresh: boolean;
}

export interface AiProviderConnection {
  id: string;
  provider: AiProviderKey;
  provider_label: string;
  auth_method: AiProviderAuthMethod;
  account_label: string;
  status: AiProviderConnectionStatus;
  credential_fingerprint: string;
  credential_version: number;
  version: number;
  configured_by_name: string;
  last_tested_at: string | null;
  last_test_status: AiProviderTestStatus | null;
  last_failure_category: string | null;
  last_used_at: string | null;
  model_count: number;
  model_catalog_refreshed_at: string | null;
  provider_data_approval_id: string;
  provider_data_approval_version: number;
  provider_data_approval_class: AiProviderDataApprovalClass;
  execution_environment_class: AiProviderExecutionEnvironmentClass;
  created_at: string;
  updated_at: string;
}

export interface ProviderDataApproval {
  id: string;
  connection_id: string;
  approval_version: number;
  approval_class: AiProviderDataApprovalClass;
  execution_environment_class: AiProviderExecutionEnvironmentClass;
  change_source: "system_default" | "administrator";
  changed_by_name: string | null;
  change_reason: string;
  created_at: string;
}

export interface ProviderModel {
  id: string;
  display_name: string;
  context_window_tokens: number | null;
  supports_tools: boolean | null;
  source: "provider";
}

export interface ProviderModelSnapshot {
  connection_id: string;
  provider: AiProviderKey;
  credential_version: number;
  refreshed_at: string | null;
  models: ProviderModel[];
}

export interface ProviderTestOutcome {
  connection: AiProviderConnection;
  outcome: {
    status: AiProviderTestStatus;
    failure_category: string | null;
    tested_at: string;
  };
}

export interface CreateProviderConnectionInput {
  provider: AiApiKeyProviderKey;
  auth_method: "api_key";
  account_label: string;
  api_key: string;
}

export interface UpdateProviderConnectionInput {
  account_label: string;
  expected_version: number;
}

export interface RotateProviderCredentialInput {
  api_key: string;
  expected_version: number;
}

export interface ProviderOAuthStart {
  attempt_id: string;
  provider: AiOAuthProviderKey;
  authorize_url: string;
}

export interface ProviderOAuthCompleteInput {
  attempt_id: string;
  callback_value: string;
}

export interface ProviderDeviceCodeStart {
  attempt_id: string;
  provider: AiDeviceCodeProviderKey;
  verification_uri_complete: string;
  user_code: string;
  interval: number;
}

export type ProviderDeviceCodeStatus = "pending" | "connected" | "expired" | "denied";

export interface ProviderDeviceCodePoll {
  status: ProviderDeviceCodeStatus;
  connection?: AiProviderConnection;
}

export interface SetProviderDataApprovalInput {
  approval_class: Exclude<AiProviderDataApprovalClass, "unapproved">;
  expected_approval_version: number;
  change_reason: string;
}

export type AiRouteScopeKind =
  | "tenant_default"
  | "task_class"
  | "module_operation"
  | "capability";

export type AiTaskClass =
  | "campus_conversation_search"
  | "module_read_reporting"
  | "document_extraction"
  | "drafting_proposal"
  | "approved_operational_action";

export type AiOperationClass = "read" | "propose" | "mutate" | "external_side_effect";
export type AiRouteTargetReadiness =
  | "ready"
  | "connection_unavailable"
  | "stale_model"
  | "tools_unsupported";

export type AiTaskRouteScope =
  | { scope_kind: "tenant_default" }
  | { scope_kind: "task_class"; task_class: AiTaskClass }
  | { scope_kind: "module_operation"; module_key: string; operation_class: AiOperationClass }
  | { scope_kind: "capability"; capability_key: string; capability_version: number };

export interface AiTaskRouteTarget {
  id: string;
  priority: number;
  connection_id: string;
  provider: string;
  account_label: string;
  provider_model_id: string;
  model_display_name: string;
  context_window_tokens: number | null;
  supports_tools: boolean | null;
  readiness: AiRouteTargetReadiness;
}

export interface AiTaskRoute {
  id: string;
  scope: AiTaskRouteScope;
  requires_tools: boolean;
  targets: AiTaskRouteTarget[];
  version: number;
  created_at: string;
  updated_at: string;
}

export interface AiTaskRouteTargetInput {
  connection_id: string;
  provider_model_id: string;
}

export interface CreateAiTaskRouteInput {
  scope_kind: AiRouteScopeKind;
  task_class?: AiTaskClass;
  module_key?: string;
  operation_class?: AiOperationClass;
  capability_key?: string;
  capability_version?: number;
  requires_tools: boolean;
  targets: AiTaskRouteTargetInput[];
  audit_reason: string;
}

export interface UpdateAiTaskRouteInput {
  expected_version: number;
  requires_tools: boolean;
  targets: AiTaskRouteTargetInput[];
  audit_reason: string;
}

export interface ResolveAiTaskRouteInput {
  task_class: AiTaskClass;
  module_key?: string;
  operation_class?: AiOperationClass;
  capability_key?: string;
  capability_version?: number;
  requires_tools: boolean;
}

export interface ResolvedAiTaskRoute {
  route_set_id: string;
  matched_scope: AiTaskRouteScope;
  precedence: "capability" | "module_operation" | "task_class" | "tenant_default";
  route_version: number;
  requires_tools: boolean;
  targets: AiTaskRouteTarget[];
}

export interface ArchivedAiTaskRoute {
  archived_id: string;
  version: number;
}

export interface AiRoutingTargetOption {
  connection_id: string;
  provider: string;
  provider_label: string;
  account_label: string;
  provider_model_id: string;
  model_display_name: string;
  context_window_tokens: number | null;
  supports_tools: boolean | null;
}

export interface AiRoutingCapabilityOption {
  capability_key: string;
  label: string;
  module_key: string;
  operation_class: AiOperationClass;
  capability_version: number;
}

export interface AiRoutingModuleOption {
  module_key: string;
  label: string;
}

export interface AiRoutingOptions {
  targets: AiRoutingTargetOption[];
  capabilities: AiRoutingCapabilityOption[];
  modules: AiRoutingModuleOption[];
}

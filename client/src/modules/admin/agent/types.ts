/**
 * Secret-free Administration contracts for campus AI provider connections.
 * Credential input exists only in mutation payloads and is never part of a response type.
 */

export type AiProviderKey = "openai" | "anthropic" | "openrouter";
export type AiProviderAuthMethod = "api_key";
export type AiProviderConnectionStatus = "untested" | "ready" | "error";
export type AiProviderTestStatus = "succeeded" | "failed";

export interface ProviderCatalogEntry {
  key: AiProviderKey;
  label: string;
  auth_methods: AiProviderAuthMethod[];
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
  created_at: string;
  updated_at: string;
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
  provider: AiProviderKey;
  auth_method: AiProviderAuthMethod;
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


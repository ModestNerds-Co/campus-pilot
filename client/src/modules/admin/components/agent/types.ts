/**
 * Defines secret-free Agent Administration response contracts.
 * Session content, provider credentials, execution artifacts, and raw resource references are
 * intentionally absent from this client boundary.
 */

export interface AgentReadiness {
  module: { enabled: boolean; status: string };
  providers: { total: number; ready: number; attention: number };
  routing: { route_sets: number; ready_targets: number; blocked_targets: number };
  capabilities: {
    catalogued_operations: number;
    executable_capabilities: number;
    approval_required: number;
    human_only: number;
    prohibited: number;
  };
  runtime: { sessions: number; queued_runs: number; active_runs: number; expired_leases: number };
  workers: {
    available: boolean;
    registered_instances: number;
    ready_instances: number;
    reason: "ready" | "not_registered" | "no_fresh_ready_worker";
  };
  limits: {
    configured_rules: number;
    enforcement_available: boolean;
    management_available: boolean;
  };
}

export type AgentExposure = "exposed" | "approval_required" | "human_only" | "prohibited";
export type AgentCapabilityAvailability =
  | "executable"
  | "module_unavailable"
  | "approval_not_released"
  | "handler_unavailable"
  | "human_only"
  | "prohibited";

export interface AgentCapabilityInventoryItem {
  operation_key: string;
  label: string;
  module_key: string;
  module_label: string;
  permission: string;
  effect: string;
  exposure: AgentExposure;
  exposure_reason: string | null;
  availability: AgentCapabilityAvailability;
  availability_reason: string | null;
  capability_version: number | null;
  required_modules: string[];
  required_features: string[];
}

export interface AgentCapabilityInventoryPage {
  summary: {
    total: number;
    exposed: number;
    approval_required: number;
    human_only: number;
    prohibited: number;
    executable: number;
  };
  modules: Array<{ key: string; label: string }>;
  filtered_count: number;
  page: number;
  per_page: number;
  total_pages: number;
  items: AgentCapabilityInventoryItem[];
}

export interface AgentCapabilityFilters {
  search?: string;
  module?: string;
  exposure?: AgentExposure;
  availability?: AgentCapabilityAvailability;
  page?: number;
  per_page?: number;
}

export interface AgentUsageFilters {
  from?: string;
  to?: string;
  person_id?: string;
  origin_module?: string;
  capability_module?: string;
  capability?: string;
  provider?: string;
  model?: string;
  outcome?: string;
  meter?: string;
}

export interface AgentUsageFilterOptions {
  people: Array<{ id: string; name: string }>;
  modules: Array<{ key: string; label: string }>;
  capabilities: Array<{ key: string; label: string }>;
  providers: string[];
  models: Array<{ provider: string; model: string }>;
  outcomes: string[];
  meters: string[];
}

export interface AgentUsageTotal {
  meter: string;
  known_amount: number;
  unknown_events: number;
  currency: string | null;
  exponent: number | null;
  pricing_version: string | null;
}

export interface AgentUsageTrendPoint extends AgentUsageTotal {
  day: string;
}

export interface AgentUsageReport {
  from: string;
  to: string;
  totals: AgentUsageTotal[];
  trend: AgentUsageTrendPoint[];
}

export interface AgentRunFilters {
  from?: string;
  to?: string;
  status?: string;
  person_id?: string;
  origin_module?: string;
  correlation_id?: string;
  search?: string;
  page?: number;
  per_page?: number;
}

export interface AgentRunAuditItem {
  id: string;
  correlation_id: string;
  session_id: string;
  session_title: string;
  requested_by_id: string;
  requested_by_name: string;
  task_class: string;
  origin_module_key: string;
  status: string;
  safe_failure_code: string | null;
  started_at: string | null;
  finished_at: string | null;
  created_at: string;
  provider_attempts: number;
  capability_calls: number;
}

export interface AgentRunAuditPage {
  page: number;
  per_page: number;
  total: number;
  total_pages: number;
  people: Array<{ id: string; name: string }>;
  modules: Array<{ key: string; label: string }>;
  items: AgentRunAuditItem[];
}

export interface AgentProviderAttemptAudit {
  id: string;
  turn_index: number;
  attempt_index: number;
  provider_key: string;
  provider_model_id: string;
  status: string;
  failure_origin: string | null;
  failure_category: string | null;
  input_tokens: number | null;
  output_tokens: number | null;
  cached_tokens: number | null;
  reasoning_tokens: number | null;
  provider_reported_cost_amount: number | null;
  provider_reported_cost_currency: string | null;
  provider_reported_cost_exponent: number | null;
  estimated_cost_amount: number | null;
  estimated_cost_currency: string | null;
  estimated_cost_exponent: number | null;
  started_at: string;
  finished_at: string | null;
}

export interface AgentCapabilityCallAudit {
  id: string;
  call_sequence: number;
  capability_key: string;
  capability_version: number;
  owning_module_key: string;
  required_permission: string;
  scope_kind: string;
  resource_count: number;
  status: string;
  safe_failure_code: string | null;
  duration_ms: number | null;
  started_at: string;
  finished_at: string | null;
}

export interface AgentRunAuditDetail {
  run: AgentRunAuditItem;
  provider_attempts: AgentProviderAttemptAudit[];
  capability_calls: AgentCapabilityCallAudit[];
  events: Array<{ event_id: string; event_type: string; created_at: string }>;
  audit_events: Array<{
    id: string;
    actor_type: string;
    actor_name: string | null;
    action_key: string;
    target_type: string | null;
    outcome: string;
    occurred_at: string;
  }>;
}

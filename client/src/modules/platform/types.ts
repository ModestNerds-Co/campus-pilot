import type React from "react";

export interface PermissionDefinition {
  key: string;
  label: string;
  description: string;
}

export interface ModuleDefinition {
  key: string;
  label: string;
  group: string;
  description: string;
  route: string;
  permission_namespace: string;
  core: boolean;
  stage: "available" | "foundation" | "planned";
  permissions: PermissionDefinition[];
}

export interface ModuleCatalogResponse {
  modules: ModuleDefinition[];
  administration_permissions: PermissionDefinition[];
}

export interface TenantModule {
  key: string;
  status: "enabled" | "disabled" | "expired" | "revoked";
  source: "core" | "legacy" | "license";
  enabled: boolean;
  expires_at: string | null;
  licensed: boolean;
}

export interface TenantModulesResponse {
  modules: TenantModule[];
}

export interface LicenseLimit {
  key: string;
  unit: string;
  period: string;
  value: number;
  enforcement: "report" | "hard";
}

export interface LicenseLeaseState {
  id: string;
  status: "active" | "superseded" | "revoked" | "expired" | "invalid";
  source: "online_activation" | "online_refresh" | "offline_import";
  catalog_version: string;
  issued_at: string;
  refresh_after: string;
  lease_expires_at: string;
  grace_until: string;
  modules: string[];
  features: string[];
  limits: LicenseLimit[];
}

export interface LicensingState {
  configured: boolean;
  connected: boolean;
  status: "unconfigured" | "active" | "suspended" | "revoked" | "error";
  deployment_id: string;
  installation_id: string | null;
  credential_hint: string | null;
  portal_url: string | null;
  latest_sequence: number;
  last_refresh_attempt_at: string | null;
  last_refresh_success_at: string | null;
  last_error_code: string | null;
  lease: LicenseLeaseState | null;
}

export interface LicenseUpdateResponse {
  activated_modules: string[];
  expires_at: string | null;
}

export interface ModuleVisual {
  icon: React.ComponentType<{ className?: string }>;
  highlights: string[];
}

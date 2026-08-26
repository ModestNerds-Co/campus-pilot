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

export interface ModuleVisual {
  icon: React.ComponentType<{ className?: string }>;
  highlights: string[];
}

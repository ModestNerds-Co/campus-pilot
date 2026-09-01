import { createFileRoute } from "@tanstack/react-router";
import { ProtectedRoute } from "@/components/protected-route";
import { InternalAuditSettingsWorkspace } from "@/modules/internal-audit";
export const Route = createFileRoute("/modules/internal-audit/settings")({ component: () => <ProtectedRoute requiredModule="internal_audit" requiredPermission="internal_audit:manage"><InternalAuditSettingsWorkspace /></ProtectedRoute> });

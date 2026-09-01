import { createFileRoute } from "@tanstack/react-router";
import { ProtectedRoute } from "@/components/protected-route";
import { InternalAuditPlansWorkspace } from "@/modules/internal-audit";
export const Route = createFileRoute("/modules/internal-audit/plans")({ component: () => <ProtectedRoute requiredModule="internal_audit" requiredPermission="internal_audit:view"><InternalAuditPlansWorkspace /></ProtectedRoute> });

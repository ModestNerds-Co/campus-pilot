import { createFileRoute } from "@tanstack/react-router";
import { ProtectedRoute } from "@/components/protected-route";
import { InternalAuditFindingsWorkspace } from "@/modules/internal-audit";
export const Route = createFileRoute("/modules/internal-audit/findings")({ component: () => <ProtectedRoute requiredModule="internal_audit" requiredPermission="internal_audit:view"><InternalAuditFindingsWorkspace /></ProtectedRoute> });

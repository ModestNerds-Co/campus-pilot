import { createFileRoute } from "@tanstack/react-router";
import { ProtectedRoute } from "@/components/protected-route";
import { InternalAuditEngagementsWorkspace } from "@/modules/internal-audit";
export const Route = createFileRoute("/modules/internal-audit/engagements")({ component: () => <ProtectedRoute requiredModule="internal_audit" requiredPermission="internal_audit:view"><InternalAuditEngagementsWorkspace /></ProtectedRoute> });

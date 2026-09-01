import { createFileRoute } from "@tanstack/react-router";
import { ProtectedRoute } from "@/components/protected-route";
import { InternalAuditEngagementDetailWorkspace } from "@/modules/internal-audit";
export const Route = createFileRoute("/modules/internal-audit/engagements/$engagementId")({ component: EngagementRoute });
function EngagementRoute() { const { engagementId } = Route.useParams(); return <ProtectedRoute requiredModule="internal_audit" requiredPermission="internal_audit:view"><InternalAuditEngagementDetailWorkspace engagementId={engagementId} /></ProtectedRoute>; }

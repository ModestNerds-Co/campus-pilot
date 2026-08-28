import { createFileRoute } from "@tanstack/react-router";

import { ProtectedRoute } from "@/components/protected-route";
import { RequisitionDetail } from "@/modules/procurement";

export const Route = createFileRoute("/modules/procurement/requisitions_/$requisitionId")({ component: RequisitionRoute });

function RequisitionRoute() {
  const { requisitionId } = Route.useParams();
  return <ProtectedRoute requiredModule="procurement" requiredPermission="procurement:view"><RequisitionDetail requisitionId={requisitionId} /></ProtectedRoute>;
}

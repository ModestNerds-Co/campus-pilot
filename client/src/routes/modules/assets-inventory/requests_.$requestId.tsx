import { createFileRoute } from "@tanstack/react-router";

import { ProtectedRoute } from "@/components/protected-route";
import { StockRequestDetail } from "@/modules/assets-inventory";

export const Route = createFileRoute("/modules/assets-inventory/requests_/$requestId")({ component: StockRequestDetailRoute });

function StockRequestDetailRoute() {
  const { requestId } = Route.useParams();
  return <ProtectedRoute requiredModule="assets_inventory" requiredPermission="assets_inventory:view"><StockRequestDetail requestId={requestId} /></ProtectedRoute>;
}

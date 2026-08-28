import { createFileRoute } from "@tanstack/react-router";

import { ProtectedRoute } from "@/components/protected-route";
import { MovementDetail } from "@/modules/assets-inventory";

export const Route = createFileRoute("/modules/assets-inventory/movements_/$movementId")({ component: MovementDetailRoute });

function MovementDetailRoute() {
  const { movementId } = Route.useParams();
  return <ProtectedRoute requiredModule="assets_inventory" requiredPermission="assets_inventory:view"><MovementDetail movementId={movementId} /></ProtectedRoute>;
}

import { createFileRoute } from "@tanstack/react-router";

import { ProtectedRoute } from "@/components/protected-route";
import { MovementsWorkspace } from "@/modules/assets-inventory";

export const Route = createFileRoute("/modules/assets-inventory/movements")({
  component: () => <ProtectedRoute requiredModule="assets_inventory" requiredPermission="assets_inventory:view"><MovementsWorkspace /></ProtectedRoute>,
});

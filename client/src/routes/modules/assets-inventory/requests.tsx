import { createFileRoute } from "@tanstack/react-router";

import { ProtectedRoute } from "@/components/protected-route";
import { StockRequestsWorkspace } from "@/modules/assets-inventory";

export const Route = createFileRoute("/modules/assets-inventory/requests")({
  component: () => <ProtectedRoute requiredModule="assets_inventory" requiredPermission="assets_inventory:view"><StockRequestsWorkspace /></ProtectedRoute>,
});

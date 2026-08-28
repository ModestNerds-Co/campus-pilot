import { createFileRoute } from "@tanstack/react-router";

import { ProtectedRoute } from "@/components/protected-route";
import { StockWorkspace } from "@/modules/assets-inventory";

export const Route = createFileRoute("/modules/assets-inventory/stock")({
  component: () => <ProtectedRoute requiredModule="assets_inventory" requiredPermission="assets_inventory:view"><StockWorkspace /></ProtectedRoute>,
});

import { createFileRoute } from "@tanstack/react-router";

import { ProtectedRoute } from "@/components/protected-route";
import { InventoryStoresWorkspace } from "@/modules/assets-inventory";

export const Route = createFileRoute("/modules/assets-inventory/stores")({
  component: () => <ProtectedRoute requiredModule="assets_inventory" requiredPermission="assets_inventory:view"><InventoryStoresWorkspace /></ProtectedRoute>,
});

import { createFileRoute } from "@tanstack/react-router";

import { ProtectedRoute } from "@/components/protected-route";
import { InventoryItemsWorkspace } from "@/modules/assets-inventory";

export const Route = createFileRoute("/modules/assets-inventory/items")({
  component: () => <ProtectedRoute requiredModule="assets_inventory" requiredPermission="assets_inventory:view"><InventoryItemsWorkspace /></ProtectedRoute>,
});

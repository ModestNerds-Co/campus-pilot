import { createFileRoute } from "@tanstack/react-router";

import { ProtectedRoute } from "@/components/protected-route";
import { ProcurementReceiptsWorkspace } from "@/modules/assets-inventory";

export const Route = createFileRoute("/modules/assets-inventory/procurement-receipts")({
  component: () => <ProtectedRoute requiredModule="assets_inventory" requiredPermission="assets_inventory:receive"><ProcurementReceiptsWorkspace /></ProtectedRoute>,
});

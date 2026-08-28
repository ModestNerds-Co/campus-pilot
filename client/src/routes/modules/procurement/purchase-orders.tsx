import { createFileRoute } from "@tanstack/react-router";

import { ProtectedRoute } from "@/components/protected-route";
import { PurchaseOrdersWorkspace } from "@/modules/procurement";

export const Route = createFileRoute("/modules/procurement/purchase-orders")({
  component: () => <ProtectedRoute requiredModule="procurement" requiredPermission="procurement:view"><PurchaseOrdersWorkspace /></ProtectedRoute>,
});

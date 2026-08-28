import { createFileRoute } from "@tanstack/react-router";

import { ProtectedRoute } from "@/components/protected-route";
import { PurchaseOrderDetail } from "@/modules/procurement";

export const Route = createFileRoute("/modules/procurement/purchase-orders_/$purchaseOrderId")({ component: PurchaseOrderRoute });

function PurchaseOrderRoute() {
  const { purchaseOrderId } = Route.useParams();
  return <ProtectedRoute requiredModule="procurement" requiredPermission="procurement:view"><PurchaseOrderDetail purchaseOrderId={purchaseOrderId} /></ProtectedRoute>;
}

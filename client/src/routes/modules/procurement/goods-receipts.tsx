import { createFileRoute } from "@tanstack/react-router";

import { ProtectedRoute } from "@/components/protected-route";
import { GoodsReceiptsWorkspace } from "@/modules/procurement";

export const Route = createFileRoute("/modules/procurement/goods-receipts")({
  component: () => <ProtectedRoute requiredModule="procurement" requiredPermission="procurement:view"><GoodsReceiptsWorkspace /></ProtectedRoute>,
});

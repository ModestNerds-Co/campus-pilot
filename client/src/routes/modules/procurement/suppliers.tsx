import { createFileRoute } from "@tanstack/react-router";

import { ProtectedRoute } from "@/components/protected-route";
import { SuppliersWorkspace } from "@/modules/procurement";

export const Route = createFileRoute("/modules/procurement/suppliers")({
  component: () => <ProtectedRoute requiredModule="procurement" requiredPermission="procurement:view"><SuppliersWorkspace /></ProtectedRoute>,
});

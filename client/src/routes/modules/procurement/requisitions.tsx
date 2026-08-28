import { createFileRoute } from "@tanstack/react-router";

import { ProtectedRoute } from "@/components/protected-route";
import { RequisitionsWorkspace } from "@/modules/procurement";

export const Route = createFileRoute("/modules/procurement/requisitions")({
  component: () => <ProtectedRoute requiredModule="procurement" requiredPermission="procurement:view"><RequisitionsWorkspace /></ProtectedRoute>,
});

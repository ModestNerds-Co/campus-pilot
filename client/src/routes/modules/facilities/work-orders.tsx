import { createFileRoute } from "@tanstack/react-router";

import { ProtectedRoute } from "@/components/protected-route";
import { FacilitiesWorkOrdersWorkspace } from "@/modules/facilities";

export const Route = createFileRoute("/modules/facilities/work-orders")({
  component: () => <ProtectedRoute requiredModule="facilities" requiredPermission="facilities:operate"><FacilitiesWorkOrdersWorkspace /></ProtectedRoute>,
});

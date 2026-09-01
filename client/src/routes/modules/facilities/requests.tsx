import { createFileRoute } from "@tanstack/react-router";

import { ProtectedRoute } from "@/components/protected-route";
import { FacilitiesRequestsWorkspace } from "@/modules/facilities";

export const Route = createFileRoute("/modules/facilities/requests")({
  component: () => <ProtectedRoute requiredModule="facilities" requiredPermission="facilities:view"><FacilitiesRequestsWorkspace /></ProtectedRoute>,
});

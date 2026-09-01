import { createFileRoute } from "@tanstack/react-router";

import { ProtectedRoute } from "@/components/protected-route";
import { FacilitiesLocationsWorkspace } from "@/modules/facilities";

export const Route = createFileRoute("/modules/facilities/locations")({
  component: () => <ProtectedRoute requiredModule="facilities" requiredPermission="facilities:manage"><FacilitiesLocationsWorkspace /></ProtectedRoute>,
});

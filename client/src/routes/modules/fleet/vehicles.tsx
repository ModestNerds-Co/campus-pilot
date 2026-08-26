import { createFileRoute } from "@tanstack/react-router";

import { VehiclesList } from "@/modules/fleet";
import { ProtectedRoute } from "@/components/protected-route";

export const Route = createFileRoute("/modules/fleet/vehicles")({
  component: () => (
    <ProtectedRoute requiredModule="fleet" requiredPermission="fleet:view">
      <VehiclesList />
    </ProtectedRoute>
  ),
});

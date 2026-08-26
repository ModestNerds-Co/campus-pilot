import { createFileRoute } from "@tanstack/react-router";

import { DriversList } from "@/modules/fleet";
import { ProtectedRoute } from "@/components/protected-route";

export const Route = createFileRoute("/modules/fleet/drivers")({
  component: () => (
    <ProtectedRoute requiredModule="fleet" requiredPermission="fleet:view">
      <DriversList />
    </ProtectedRoute>
  ),
});

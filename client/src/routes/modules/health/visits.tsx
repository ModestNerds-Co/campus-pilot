import { createFileRoute } from "@tanstack/react-router";

import { ProtectedRoute } from "@/components/protected-route";
import { HealthVisitsWorkspace } from "@/modules/health";

export const Route = createFileRoute("/modules/health/visits")({
  component: () => (
    <ProtectedRoute requiredModule="health" requiredPermission="health:view">
      <HealthVisitsWorkspace />
    </ProtectedRoute>
  ),
});

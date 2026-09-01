import { createFileRoute } from "@tanstack/react-router";

import { ProtectedRoute } from "@/components/protected-route";
import { HealthPatientsWorkspace } from "@/modules/health";

export const Route = createFileRoute("/modules/health/patients")({
  component: () => (
    <ProtectedRoute requiredModule="health" requiredPermission="health:view">
      <HealthPatientsWorkspace />
    </ProtectedRoute>
  ),
});

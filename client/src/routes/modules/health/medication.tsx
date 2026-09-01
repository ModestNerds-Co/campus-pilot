import { createFileRoute } from "@tanstack/react-router";

import { ProtectedRoute } from "@/components/protected-route";
import { HealthMedicationWorkspace } from "@/modules/health";

export const Route = createFileRoute("/modules/health/medication")({
  component: () => (
    <ProtectedRoute requiredModule="health" requiredPermission="health:view">
      <HealthMedicationWorkspace />
    </ProtectedRoute>
  ),
});

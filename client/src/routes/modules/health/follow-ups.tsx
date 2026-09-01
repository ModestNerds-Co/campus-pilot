import { createFileRoute } from "@tanstack/react-router";

import { ProtectedRoute } from "@/components/protected-route";
import { HealthFollowUpsWorkspace } from "@/modules/health";

export const Route = createFileRoute("/modules/health/follow-ups")({
  component: () => (
    <ProtectedRoute requiredModule="health" requiredPermission="health:view">
      <HealthFollowUpsWorkspace />
    </ProtectedRoute>
  ),
});

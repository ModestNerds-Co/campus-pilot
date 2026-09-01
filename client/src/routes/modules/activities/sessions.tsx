import { createFileRoute } from "@tanstack/react-router";

import { ProtectedRoute } from "@/components/protected-route";
import { ActivitiesSessionsWorkspace } from "@/modules/activities";

export const Route = createFileRoute("/modules/activities/sessions")({
  component: () => (
    <ProtectedRoute requiredModule="activities" requiredPermission="activities:view">
      <ActivitiesSessionsWorkspace />
    </ProtectedRoute>
  ),
});

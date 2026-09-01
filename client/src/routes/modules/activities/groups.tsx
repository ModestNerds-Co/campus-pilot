import { createFileRoute } from "@tanstack/react-router";

import { ProtectedRoute } from "@/components/protected-route";
import { ActivitiesGroupsWorkspace } from "@/modules/activities";

export const Route = createFileRoute("/modules/activities/groups")({
  component: () => (
    <ProtectedRoute requiredModule="activities" requiredPermission="activities:view">
      <ActivitiesGroupsWorkspace />
    </ProtectedRoute>
  ),
});

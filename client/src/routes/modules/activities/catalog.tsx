import { createFileRoute } from "@tanstack/react-router";

import { ProtectedRoute } from "@/components/protected-route";
import { ActivitiesCatalogWorkspace } from "@/modules/activities";

export const Route = createFileRoute("/modules/activities/catalog")({
  component: () => (
    <ProtectedRoute requiredModule="activities" requiredPermission="activities:manage">
      <ActivitiesCatalogWorkspace />
    </ProtectedRoute>
  ),
});

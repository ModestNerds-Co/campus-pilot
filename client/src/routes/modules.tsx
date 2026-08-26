import { createFileRoute, Outlet } from "@tanstack/react-router";

import { ProtectedRoute } from "@/components/protected-route";
import { ModuleLayout } from "@/modules/platform/module-layout";

export const Route = createFileRoute("/modules")({
  component: () => (
    <ProtectedRoute>
      <ModuleLayout><Outlet /></ModuleLayout>
    </ProtectedRoute>
  ),
});

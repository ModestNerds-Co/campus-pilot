import { createFileRoute } from "@tanstack/react-router";

import { ProtectedRoute } from "@/components/protected-route";
import { LicensingPanel } from "@/modules/platform/licensing-panel";

export const Route = createFileRoute("/admin/licensing")({
  component: () => (
    <ProtectedRoute requiredModule="administration" requiredPermission="licensing:view">
      <LicensingPanel />
    </ProtectedRoute>
  ),
});

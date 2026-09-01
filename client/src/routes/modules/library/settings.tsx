import { createFileRoute } from "@tanstack/react-router";
import { ProtectedRoute } from "@/components/protected-route";
import { LibrarySettingsWorkspace } from "@/modules/library";
export const Route = createFileRoute("/modules/library/settings")({
  component: () => (
    <ProtectedRoute
      requiredModule="library"
      requiredPermission="library:manage"
    >
      <LibrarySettingsWorkspace />
    </ProtectedRoute>
  ),
});

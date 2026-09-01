import { createFileRoute } from "@tanstack/react-router";
import { ProtectedRoute } from "@/components/protected-route";
import { LibraryCirculationWorkspace } from "@/modules/library";
export const Route = createFileRoute("/modules/library/circulation")({
  component: () => (
    <ProtectedRoute requiredModule="library" requiredPermission="library:view">
      <LibraryCirculationWorkspace />
    </ProtectedRoute>
  ),
});

import { createFileRoute } from "@tanstack/react-router";
import { ProtectedRoute } from "@/components/protected-route";
import { LibraryFinesWorkspace } from "@/modules/library";
export const Route = createFileRoute("/modules/library/fines")({
  component: () => (
    <ProtectedRoute requiredModule="library" requiredPermission="library:view">
      <LibraryFinesWorkspace />
    </ProtectedRoute>
  ),
});

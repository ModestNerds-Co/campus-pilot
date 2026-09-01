import { createFileRoute } from "@tanstack/react-router";
import { ProtectedRoute } from "@/components/protected-route";
import { LibraryHoldsWorkspace } from "@/modules/library";
export const Route = createFileRoute("/modules/library/holds")({
  component: () => (
    <ProtectedRoute requiredModule="library" requiredPermission="library:view">
      <LibraryHoldsWorkspace />
    </ProtectedRoute>
  ),
});

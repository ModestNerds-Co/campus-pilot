import { createFileRoute } from "@tanstack/react-router";
import { ProtectedRoute } from "@/components/protected-route";
import { LibraryMembersWorkspace } from "@/modules/library";
export const Route = createFileRoute("/modules/library/members")({
  component: () => (
    <ProtectedRoute requiredModule="library" requiredPermission="library:view">
      <LibraryMembersWorkspace />
    </ProtectedRoute>
  ),
});

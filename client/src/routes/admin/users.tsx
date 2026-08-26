import { createFileRoute } from "@tanstack/react-router";
import { UsersList } from "@/modules/users";
import { ProtectedRoute } from "@/components/protected-route";

export const Route = createFileRoute("/admin/users")({
  component: () => (
    <ProtectedRoute requiredModule="administration" requiredPermission="users:view">
      <UsersList />
    </ProtectedRoute>
  ),
});

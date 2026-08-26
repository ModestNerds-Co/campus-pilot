import { createFileRoute } from "@tanstack/react-router";
import { RolesList } from "@/modules/users";
import { ProtectedRoute } from "@/components/protected-route";

export const Route = createFileRoute("/admin/roles")({
  component: () => (
    <ProtectedRoute requiredModule="administration" requiredPermission="roles:view">
      <RolesList />
    </ProtectedRoute>
  ),
});

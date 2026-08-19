import { createFileRoute } from "@tanstack/react-router";
import { RolesList } from "@/modules/users";

export const Route = createFileRoute("/admin/roles")({
  component: RolesList,
});

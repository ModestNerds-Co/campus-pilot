import { createFileRoute } from "@tanstack/react-router";
import { UsersList } from "@/modules/users";

export const Route = createFileRoute("/admin/users")({
  component: UsersList,
});

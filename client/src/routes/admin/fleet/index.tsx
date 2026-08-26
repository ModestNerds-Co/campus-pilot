import { createFileRoute, redirect } from "@tanstack/react-router";

export const Route = createFileRoute("/admin/fleet/")({
  beforeLoad: () => {
    throw redirect({ to: "/modules/$moduleKey", params: { moduleKey: "fleet" }, replace: true });
  },
});

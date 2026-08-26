import { createFileRoute, redirect } from "@tanstack/react-router";

export const Route = createFileRoute("/admin/fleet/drivers")({
  beforeLoad: () => {
    throw redirect({ to: "/modules/fleet/drivers", replace: true });
  },
});

import { createFileRoute, redirect } from "@tanstack/react-router";

export const Route = createFileRoute("/admin/fleet/daily-log")({
  beforeLoad: () => {
    throw redirect({ to: "/modules/fleet/daily-log", replace: true });
  },
});

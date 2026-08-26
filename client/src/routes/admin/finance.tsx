import { createFileRoute, redirect } from "@tanstack/react-router";

export const Route = createFileRoute("/admin/finance")({
  beforeLoad: () => {
    throw redirect({ to: "/modules/$moduleKey", params: { moduleKey: "finance" }, replace: true });
  },
});

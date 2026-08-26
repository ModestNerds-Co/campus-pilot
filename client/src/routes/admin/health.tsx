import { createFileRoute, redirect } from "@tanstack/react-router";

export const Route = createFileRoute("/admin/health")({
  beforeLoad: () => {
    throw redirect({ to: "/modules/$moduleKey", params: { moduleKey: "health" }, replace: true });
  },
});

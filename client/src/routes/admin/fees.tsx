import { createFileRoute, redirect } from "@tanstack/react-router";

export const Route = createFileRoute("/admin/fees")({
  beforeLoad: () => {
    throw redirect({ to: "/modules/$moduleKey", params: { moduleKey: "fees" }, replace: true });
  },
});

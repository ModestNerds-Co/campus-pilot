import { createFileRoute, redirect } from "@tanstack/react-router";

export const Route = createFileRoute("/admin/procurement")({
  beforeLoad: () => {
    throw redirect({ to: "/modules/$moduleKey", params: { moduleKey: "procurement" }, replace: true });
  },
});

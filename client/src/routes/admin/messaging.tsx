import { createFileRoute, redirect } from "@tanstack/react-router";

export const Route = createFileRoute("/admin/messaging")({
  beforeLoad: () => {
    throw redirect({ to: "/modules/$moduleKey", params: { moduleKey: "messaging" }, replace: true });
  },
});

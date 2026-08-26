import { createFileRoute, redirect } from "@tanstack/react-router";

export const Route = createFileRoute("/admin/library")({
  beforeLoad: () => {
    throw redirect({ to: "/modules/$moduleKey", params: { moduleKey: "library" }, replace: true });
  },
});

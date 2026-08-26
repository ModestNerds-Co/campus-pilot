import { createFileRoute, redirect } from "@tanstack/react-router";

export const Route = createFileRoute("/admin/students")({
  beforeLoad: () => {
    throw redirect({ to: "/modules/$moduleKey", params: { moduleKey: "sis" }, replace: true });
  },
});

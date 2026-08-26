import { createFileRoute, redirect } from "@tanstack/react-router";

export const Route = createFileRoute("/admin/hostel")({
  beforeLoad: () => {
    throw redirect({ to: "/modules/$moduleKey", params: { moduleKey: "hostel" }, replace: true });
  },
});

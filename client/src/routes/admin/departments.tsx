import { createFileRoute, redirect } from "@tanstack/react-router";

export const Route = createFileRoute("/admin/departments")({
  beforeLoad: () => {
    throw redirect({ to: "/modules/hr-payroll/departments", replace: true });
  },
});

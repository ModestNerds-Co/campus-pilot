import { createFileRoute, redirect } from "@tanstack/react-router";

export const Route = createFileRoute("/admin/hr-payroll")({
  beforeLoad: () => {
    throw redirect({ to: "/modules/$moduleKey", params: { moduleKey: "hr_payroll" }, replace: true });
  },
});

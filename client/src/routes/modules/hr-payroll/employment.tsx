import { createFileRoute } from "@tanstack/react-router";

import { ProtectedRoute } from "@/components/protected-route";
import { EmploymentEngagementsList } from "@/modules/hr-payroll";

export const Route = createFileRoute("/modules/hr-payroll/employment")({
  component: () => <ProtectedRoute requiredModule="hr_payroll" requiredPermission="hr_payroll:view"><EmploymentEngagementsList /></ProtectedRoute>,
});

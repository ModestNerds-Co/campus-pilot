import { createFileRoute } from "@tanstack/react-router";

import { ProtectedRoute } from "@/components/protected-route";
import { EmployeeAvailabilityList } from "@/modules/hr-payroll";

export const Route = createFileRoute("/modules/hr-payroll/availability")({
  component: () => <ProtectedRoute requiredModule="hr_payroll" requiredPermission="hr_payroll:view"><EmployeeAvailabilityList /></ProtectedRoute>,
});

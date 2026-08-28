import { createFileRoute } from "@tanstack/react-router";

import { ProtectedRoute } from "@/components/protected-route";
import { HrImportsWorkspace } from "@/modules/hr-payroll";

export const Route = createFileRoute("/modules/hr-payroll/imports")({
  component: () => <ProtectedRoute requiredModule="hr_payroll" requiredPermission="hr_payroll:view"><HrImportsWorkspace /></ProtectedRoute>,
});

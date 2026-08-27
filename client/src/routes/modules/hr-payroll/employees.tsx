import { createFileRoute } from "@tanstack/react-router";
import { ProtectedRoute } from "@/components/protected-route";
import { EmployeesList } from "@/modules/hr-payroll";
export const Route = createFileRoute("/modules/hr-payroll/employees")({ component: () => <ProtectedRoute requiredModule="hr_payroll" requiredPermission="hr_payroll:view"><EmployeesList /></ProtectedRoute> });

import { createFileRoute } from "@tanstack/react-router";
import { ProtectedRoute } from "@/components/protected-route";
import { DepartmentsList } from "@/modules/hr-payroll";
export const Route = createFileRoute("/modules/hr-payroll/departments")({ component: () => <ProtectedRoute requiredModule="hr_payroll" requiredPermission="hr_payroll:view"><DepartmentsList /></ProtectedRoute> });

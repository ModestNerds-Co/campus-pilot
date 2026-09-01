import { createFileRoute } from "@tanstack/react-router";
import { ProtectedRoute } from "@/components/protected-route";
import { HR_ADMINISTRATION_PERMISSIONS } from "@/modules/hr-payroll/access";
import { DepartmentsList } from "@/modules/hr-payroll";
export const Route = createFileRoute("/modules/hr-payroll/departments")({ component: () => <ProtectedRoute requiredAnyPermissions={HR_ADMINISTRATION_PERMISSIONS} requiredModule="hr_payroll" requiredPermission="hr_payroll:view"><DepartmentsList /></ProtectedRoute> });

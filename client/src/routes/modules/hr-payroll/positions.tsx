import { createFileRoute } from "@tanstack/react-router";
import { ProtectedRoute } from "@/components/protected-route";
import { PositionsList } from "@/modules/hr-payroll";
export const Route = createFileRoute("/modules/hr-payroll/positions")({ component: () => <ProtectedRoute requiredModule="hr_payroll" requiredPermission="hr_payroll:view"><PositionsList /></ProtectedRoute> });

import { createFileRoute } from "@tanstack/react-router";

import { ProtectedRoute } from "@/components/protected-route";
import { AttendanceRegistersWorkspace } from "@/modules/attendance";

export const Route = createFileRoute("/modules/attendance/registers")({
  component: () => <ProtectedRoute requiredModule="attendance" requiredPermission="attendance:view"><AttendanceRegistersWorkspace /></ProtectedRoute>,
});

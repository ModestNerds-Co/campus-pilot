import { createFileRoute } from "@tanstack/react-router";

import { ProtectedRoute } from "@/components/protected-route";
import { AttendanceExceptionWorkspace } from "@/modules/attendance";

export const Route = createFileRoute("/modules/attendance/exceptions_/$exceptionId")({
  component: ExceptionRoute,
});

function ExceptionRoute() {
  const { exceptionId } = Route.useParams();
  return <ProtectedRoute requiredModule="attendance" requiredPermission="attendance:manage" requiredRecordScope="attendance.registers" requiredRecordScopeKind="campus"><AttendanceExceptionWorkspace exceptionId={exceptionId} /></ProtectedRoute>;
}

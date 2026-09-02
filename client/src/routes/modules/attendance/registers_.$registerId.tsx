/** Direct attendance-register route. The URL is the only source of record identity. */
import { createFileRoute } from "@tanstack/react-router";

import { ProtectedRoute } from "@/components/protected-route";
import { AttendanceRegisterWorkspace } from "@/modules/attendance";

export const Route = createFileRoute("/modules/attendance/registers_/$registerId")({
  component: AttendanceRegisterRoute,
});

function AttendanceRegisterRoute() {
  const { registerId } = Route.useParams();
  return <ProtectedRoute requiredModule="attendance" requiredPermission="attendance:view" requiredRecordScope="attendance.registers"><AttendanceRegisterWorkspace registerId={registerId} /></ProtectedRoute>;
}

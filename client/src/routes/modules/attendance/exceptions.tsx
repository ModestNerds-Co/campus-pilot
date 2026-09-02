import { createFileRoute } from "@tanstack/react-router";

import { ProtectedRoute } from "@/components/protected-route";
import { AttendanceExceptionsWorkspace } from "@/modules/attendance";
import { parseExceptionsSearch } from "@/modules/attendance/search";

export const Route = createFileRoute("/modules/attendance/exceptions")({
  validateSearch: parseExceptionsSearch,
  component: ExceptionsRoute,
});

function ExceptionsRoute() {
  const navigate = Route.useNavigate();
  const search = Route.useSearch();
  return <ProtectedRoute requiredModule="attendance" requiredPermission="attendance:manage" requiredRecordScope="attendance.registers" requiredRecordScopeKind="campus">
    <AttendanceExceptionsWorkspace onSearchChange={(next, options) => void navigate({ replace: options?.replace, search: next })} search={search} />
  </ProtectedRoute>;
}

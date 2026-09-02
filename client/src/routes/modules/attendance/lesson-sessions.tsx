import { createFileRoute } from "@tanstack/react-router";

import { ProtectedRoute } from "@/components/protected-route";
import { AttendanceLessonSessionsWorkspace } from "@/modules/attendance";
import { parseLessonSessionsSearch } from "@/modules/attendance/search";

export const Route = createFileRoute("/modules/attendance/lesson-sessions")({
  validateSearch: parseLessonSessionsSearch,
  component: LessonSessionsRoute,
});

function LessonSessionsRoute() {
  const navigate = Route.useNavigate();
  const search = Route.useSearch();
  return <ProtectedRoute requiredModule="attendance" requiredPermission="attendance:view" requiredRecordScope="attendance.registers">
    <ProtectedRoute requiredModule="timetabling">
      <AttendanceLessonSessionsWorkspace onSearchChange={(next, options) => void navigate({ replace: options?.replace, search: next })} search={search} />
    </ProtectedRoute>
  </ProtectedRoute>;
}

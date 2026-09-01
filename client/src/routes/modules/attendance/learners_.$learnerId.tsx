/** Direct learner Attendance history route. */

import { createFileRoute } from "@tanstack/react-router";

import { ProtectedRoute } from "@/components/protected-route";
import { LearnerAttendanceHistoryWorkspace } from "@/modules/attendance";

export const Route = createFileRoute("/modules/attendance/learners_/$learnerId")({
  component: LearnerAttendanceHistoryRoute,
});

function LearnerAttendanceHistoryRoute() {
  const { learnerId } = Route.useParams();
  return <ProtectedRoute requiredModule="attendance" requiredPermission="attendance:view"><LearnerAttendanceHistoryWorkspace learnerId={learnerId} /></ProtectedRoute>;
}

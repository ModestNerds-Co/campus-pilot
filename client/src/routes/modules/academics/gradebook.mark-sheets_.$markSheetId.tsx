/** Direct mark-sheet route. The URL is the only source of record identity. */
import { createFileRoute } from "@tanstack/react-router";

import { ProtectedRoute } from "@/components/protected-route";
import { ACADEMIC_TEACHING_PERMISSIONS } from "@/modules/academics/access";
import { MarkSheetWorkspace } from "@/modules/gradebook";

export const Route = createFileRoute("/modules/academics/gradebook/mark-sheets_/$markSheetId")({
  component: MarkSheetRoute,
});

function MarkSheetRoute() {
  const { markSheetId } = Route.useParams();
  return <ProtectedRoute requiredAnyPermissions={ACADEMIC_TEACHING_PERMISSIONS} requiredModule="academics"><MarkSheetWorkspace markSheetId={markSheetId} /></ProtectedRoute>;
}

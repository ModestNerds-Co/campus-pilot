/** Full-page staged mark import route for one scoped draft mark sheet. */
import { createFileRoute } from "@tanstack/react-router";

import { ProtectedRoute } from "@/components/protected-route";
import { ACADEMIC_TEACHING_PERMISSIONS } from "@/modules/academics/access";
import { MarkImportsWorkspace } from "@/modules/gradebook";

export const Route = createFileRoute("/modules/academics/gradebook_/mark-sheets_/$markSheetId_/imports")({
  component: MarkImportsRoute,
});

function MarkImportsRoute() {
  const { markSheetId } = Route.useParams();
  return <ProtectedRoute requiredAnyPermissions={ACADEMIC_TEACHING_PERMISSIONS} requiredModule="academics"><MarkImportsWorkspace markSheetId={markSheetId} /></ProtectedRoute>;
}

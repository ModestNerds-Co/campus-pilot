/** Direct class-record route. The URL is the only source of record identity. */
import { createFileRoute } from "@tanstack/react-router";

import { ProtectedRoute } from "@/components/protected-route";
import { AcademicClassRecord } from "@/modules/academics";
import { ACADEMIC_ADMINISTRATION_PERMISSIONS } from "@/modules/academics/access";

export const Route = createFileRoute("/modules/academics/classes_/$classId")({
  component: AcademicClassRecordRoute,
});

function AcademicClassRecordRoute() {
  const { classId } = Route.useParams();
  return <ProtectedRoute requiredAnyPermissions={ACADEMIC_ADMINISTRATION_PERMISSIONS} requiredModule="academics"><AcademicClassRecord classId={classId} /></ProtectedRoute>;
}

/** Direct class-record route. The URL is the only source of record identity. */
import { createFileRoute } from "@tanstack/react-router";

import { ProtectedRoute } from "@/components/protected-route";
import { AcademicClassRecord } from "@/modules/academics";

export const Route = createFileRoute("/modules/academics/classes_/$classId")({
  component: AcademicClassRecordRoute,
});

function AcademicClassRecordRoute() {
  const { classId } = Route.useParams();
  return <ProtectedRoute requiredModule="academics" requiredPermission="academics:view"><AcademicClassRecord classId={classId} /></ProtectedRoute>;
}

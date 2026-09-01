import { createFileRoute } from "@tanstack/react-router";

import { ProtectedRoute } from "@/components/protected-route";
import { ApplicationRecord } from "@/modules/sis";
import { SIS_ADMINISTRATION_PERMISSIONS } from "@/modules/sis/access";

export const Route = createFileRoute("/modules/sis/applications_/$applicationId")({
  component: ApplicationRecordRoute,
});

function ApplicationRecordRoute() {
  const { applicationId } = Route.useParams();
  return <ProtectedRoute requiredAnyPermissions={SIS_ADMINISTRATION_PERMISSIONS} requiredModule="sis" requiredPermission="sis:view"><ApplicationRecord applicationId={applicationId} /></ProtectedRoute>;
}

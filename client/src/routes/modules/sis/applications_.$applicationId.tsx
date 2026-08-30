import { createFileRoute } from "@tanstack/react-router";

import { ProtectedRoute } from "@/components/protected-route";
import { ApplicationRecord } from "@/modules/sis";

export const Route = createFileRoute("/modules/sis/applications_/$applicationId")({
  component: ApplicationRecordRoute,
});

function ApplicationRecordRoute() {
  const { applicationId } = Route.useParams();
  return <ProtectedRoute requiredModule="sis" requiredPermission="sis:view"><ApplicationRecord applicationId={applicationId} /></ProtectedRoute>;
}

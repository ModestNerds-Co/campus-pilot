import { createFileRoute } from "@tanstack/react-router";

import { ProtectedRoute } from "@/components/protected-route";
import { HealthPatientRecord } from "@/modules/health";

export const Route = createFileRoute("/modules/health/patients_/$patientId")({
  component: PatientRecordRoute,
});

function PatientRecordRoute() {
  const { patientId } = Route.useParams();
  return (
    <ProtectedRoute requiredModule="health" requiredPermission="health:view">
      <HealthPatientRecord patientId={patientId} />
    </ProtectedRoute>
  );
}

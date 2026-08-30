/** Direct learner-record route. The URL is the only source of record identity. */
import { createFileRoute } from "@tanstack/react-router";

import { ProtectedRoute } from "@/components/protected-route";
import { LearnerRecord } from "@/modules/sis";

export const Route = createFileRoute("/modules/sis/learners_/$learnerId")({
  component: LearnerRecordRoute,
});

function LearnerRecordRoute() {
  const { learnerId } = Route.useParams();
  return <ProtectedRoute requiredModule="sis" requiredPermission="sis:view"><LearnerRecord learnerId={learnerId} /></ProtectedRoute>;
}

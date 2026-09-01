/** Direct transcript route. The URL is the only source of learner identity. */
import { createFileRoute } from "@tanstack/react-router";

import { ProtectedRoute } from "@/components/protected-route";
import { TranscriptWorkspace } from "@/modules/reporting";

export const Route = createFileRoute("/modules/academics/reporting/transcripts_/$learnerId")({
  component: TranscriptRoute,
});

function TranscriptRoute() {
  const { learnerId } = Route.useParams();
  return <ProtectedRoute requiredModule="academics" requiredPermission="academics:view"><TranscriptWorkspace learnerId={learnerId} /></ProtectedRoute>;
}

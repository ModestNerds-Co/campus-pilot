import { createFileRoute } from "@tanstack/react-router";

import { ProtectedRoute } from "@/components/protected-route";
import { LearningScoreTransferWorkspace } from "@/modules/learning";

export const Route = createFileRoute("/modules/learning/score-transfers_/$proposalId")({
  component: LearningScoreTransferRoute,
});

function LearningScoreTransferRoute() {
  const { proposalId } = Route.useParams();
  return <ProtectedRoute requiredPermission="learning:teach" requiredModule="learning"><LearningScoreTransferWorkspace proposalId={proposalId} /></ProtectedRoute>;
}

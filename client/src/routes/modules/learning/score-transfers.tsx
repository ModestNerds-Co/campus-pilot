import { createFileRoute } from "@tanstack/react-router";

import { ProtectedRoute } from "@/components/protected-route";
import { LearningScoreTransfersWorkspace } from "@/modules/learning";
import { parseScoreTransfersSearch } from "@/modules/learning/search";

export const Route = createFileRoute("/modules/learning/score-transfers")({
  validateSearch: parseScoreTransfersSearch,
  component: LearningScoreTransfersRoute,
});

function LearningScoreTransfersRoute() {
  const navigate = Route.useNavigate();
  const search = Route.useSearch();
  return <ProtectedRoute requiredPermission="learning:teach" requiredModule="learning"><LearningScoreTransfersWorkspace onSearchChange={(next) => void navigate({ search: next })} search={search} /></ProtectedRoute>;
}

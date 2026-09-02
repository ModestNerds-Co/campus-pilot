import { createFileRoute } from "@tanstack/react-router";

import { ProtectedRoute } from "@/components/protected-route";
import { LearningQuizzesWorkspace } from "@/modules/learning";
import { parseQuizzesSearch } from "@/modules/learning/search";

export const Route = createFileRoute(
  "/modules/learning/spaces_/$spaceId/quizzes",
)({
  validateSearch: parseQuizzesSearch,
  component: LearningQuizzesRoute,
});

function LearningQuizzesRoute() {
  const { spaceId } = Route.useParams();
  const navigate = Route.useNavigate();
  const search = Route.useSearch();
  return <ProtectedRoute requiredModule="learning" requiredPermission="learning:view">
    <LearningQuizzesWorkspace onSearchChange={(next) => void navigate({ search: next })} search={search} spaceId={spaceId} />
  </ProtectedRoute>;
}

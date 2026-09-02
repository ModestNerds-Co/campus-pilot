import { createFileRoute } from "@tanstack/react-router";

import { ProtectedRoute } from "@/components/protected-route";
import { LearningQuizWorkspace } from "@/modules/learning";

export const Route = createFileRoute(
  "/modules/learning/spaces_/$spaceId/quizzes_/$quizId",
)({ component: LearningQuizRoute });

function LearningQuizRoute() {
  const { quizId, spaceId } = Route.useParams();
  return <ProtectedRoute requiredModule="learning" requiredPermission="learning:view">
    <LearningQuizWorkspace quizId={quizId} spaceId={spaceId} />
  </ProtectedRoute>;
}

import { createFileRoute } from "@tanstack/react-router";

import { ProtectedRoute } from "@/components/protected-route";
import { LearningCompletionWorkspace } from "@/modules/learning";

export const Route = createFileRoute(
  "/modules/learning/spaces_/$spaceId/completion",
)({ component: LearningCompletionRoute });

function LearningCompletionRoute() {
  const { spaceId } = Route.useParams();
  return <ProtectedRoute requiredModule="learning" requiredPermission="learning:view">
    <LearningCompletionWorkspace spaceId={spaceId} />
  </ProtectedRoute>;
}

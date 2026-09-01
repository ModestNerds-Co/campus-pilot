import { createFileRoute } from "@tanstack/react-router";

import { ProtectedRoute } from "@/components/protected-route";
import { LearningUnitWorkspace } from "@/modules/learning";

export const Route = createFileRoute(
  "/modules/learning/spaces_/$spaceId/units_/$unitId",
)({ component: LearningUnitRoute });

function LearningUnitRoute() {
  const { spaceId, unitId } = Route.useParams();
  return (
    <ProtectedRoute requiredModule="learning" requiredPermission="learning:view">
      <LearningUnitWorkspace spaceId={spaceId} unitId={unitId} />
    </ProtectedRoute>
  );
}

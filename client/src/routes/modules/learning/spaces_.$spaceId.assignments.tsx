import { createFileRoute } from "@tanstack/react-router";

import { ProtectedRoute } from "@/components/protected-route";
import { LearningAssignmentsWorkspace } from "@/modules/learning";
import { parseAssignmentsSearch } from "@/modules/learning/search";

export const Route = createFileRoute(
  "/modules/learning/spaces_/$spaceId/assignments",
)({
  validateSearch: parseAssignmentsSearch,
  component: LearningAssignmentsRoute,
});

function LearningAssignmentsRoute() {
  const { spaceId } = Route.useParams();
  const navigate = Route.useNavigate();
  const search = Route.useSearch();
  return (
    <ProtectedRoute requiredModule="learning" requiredPermission="learning:view">
      <LearningAssignmentsWorkspace
        onSearchChange={(next) => void navigate({ search: next })}
        search={search}
        spaceId={spaceId}
      />
    </ProtectedRoute>
  );
}

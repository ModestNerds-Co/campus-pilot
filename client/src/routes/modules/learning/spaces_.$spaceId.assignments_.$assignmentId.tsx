import { createFileRoute } from "@tanstack/react-router";

import { ProtectedRoute } from "@/components/protected-route";
import { LearningAssignmentWorkspace } from "@/modules/learning";
import { parseAssignmentDetailSearch } from "@/modules/learning/search";

export const Route = createFileRoute(
  "/modules/learning/spaces_/$spaceId/assignments_/$assignmentId",
)({
  validateSearch: parseAssignmentDetailSearch,
  component: LearningAssignmentRoute,
});

function LearningAssignmentRoute() {
  const { assignmentId, spaceId } = Route.useParams();
  const navigate = Route.useNavigate();
  const search = Route.useSearch();
  return (
    <ProtectedRoute requiredModule="learning" requiredPermission="learning:view">
      <LearningAssignmentWorkspace
        assignmentId={assignmentId}
        onSearchChange={(next, options) =>
          void navigate({ replace: options?.replace, search: next })
        }
        search={search}
        spaceId={spaceId}
      />
    </ProtectedRoute>
  );
}

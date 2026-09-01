import { createFileRoute } from "@tanstack/react-router";

import { ProtectedRoute } from "@/components/protected-route";
import { LearningProgressWorkspace } from "@/modules/learning";
import { parseProgressSearch } from "@/modules/learning/search";

export const Route = createFileRoute("/modules/learning/spaces_/$spaceId/progress")({
  validateSearch: parseProgressSearch,
  component: LearningProgressRoute,
});

function LearningProgressRoute() {
  const { spaceId } = Route.useParams();
  const navigate = Route.useNavigate();
  const search = Route.useSearch();
  return (
    <ProtectedRoute requiredModule="learning" requiredPermission="learning:view">
      <LearningProgressWorkspace
        onSearchChange={(next, options) =>
          void navigate({ replace: options?.replace, search: next })
        }
        search={search}
        spaceId={spaceId}
      />
    </ProtectedRoute>
  );
}

import { createFileRoute } from "@tanstack/react-router";

import { ProtectedRoute } from "@/components/protected-route";
import { LearningSubmissionWorkspace } from "@/modules/learning";
import { parseSubmissionSearch } from "@/modules/learning/search";

export const Route = createFileRoute("/modules/learning/submissions_/$submissionId")({
  validateSearch: parseSubmissionSearch,
  component: LearningSubmissionRoute,
});

function LearningSubmissionRoute() {
  const { submissionId } = Route.useParams();
  const navigate = Route.useNavigate();
  const search = Route.useSearch();
  return (
    <ProtectedRoute requiredModule="learning" requiredPermission="learning:view">
      <LearningSubmissionWorkspace
        onVersionChange={(versionId: string) =>
          void navigate({
            search: (previous) => ({ ...previous, version: versionId }),
          })
        }
        submissionId={submissionId}
        versionId={search.version}
      />
    </ProtectedRoute>
  );
}

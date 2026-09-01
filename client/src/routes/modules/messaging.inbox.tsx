import { createFileRoute } from "@tanstack/react-router";
import { ProtectedRoute } from "@/components/protected-route";
import { CommunicationInboxWorkspace } from "@/modules/messaging";

export const Route = createFileRoute("/modules/messaging/inbox")({ component: InboxRoute });

function InboxRoute() {
  const navigate = Route.useNavigate();
  const search = Route.useSearch();
  return (
    <ProtectedRoute requiredModule="messaging" requiredPermission="messaging:view">
      <CommunicationInboxWorkspace
        filter={search.filter}
        onFiltersChange={(next, options) =>
          void navigate({
            replace: options?.replace,
            search: (previous) => ({ ...previous, ...next }),
          })
        }
        page={search.page}
      />
    </ProtectedRoute>
  );
}

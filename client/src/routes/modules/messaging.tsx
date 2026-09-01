import { createFileRoute, Outlet, useLocation } from "@tanstack/react-router";
import { ProtectedRoute } from "@/components/protected-route";
import { CommunicationHome } from "@/modules/messaging";
import { parseMessagingSearch } from "@/modules/messaging/search";

export const Route = createFileRoute("/modules/messaging")({
  validateSearch: parseMessagingSearch,
  component: MessagingRoute,
});

function MessagingRoute() {
  const location = useLocation();
  const navigate = Route.useNavigate();
  const search = Route.useSearch();
  if (location.pathname !== "/modules/messaging") return <Outlet />;
  return (
    <ProtectedRoute requiredModule="messaging" requiredPermission="messaging:view">
      <CommunicationHome
        onSearchChange={(next, options) =>
          void navigate({ replace: options?.replace, search: next })
        }
        search={search}
      />
    </ProtectedRoute>
  );
}

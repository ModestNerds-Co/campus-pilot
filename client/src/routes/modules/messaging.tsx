import { createFileRoute, Outlet, useLocation } from "@tanstack/react-router";
import { ProtectedRoute } from "@/components/protected-route";
import { CommunicationHome } from "@/modules/messaging";

export const Route = createFileRoute("/modules/messaging")({ component: MessagingRoute });

function MessagingRoute() {
  const location = useLocation();
  if (location.pathname !== "/modules/messaging") return <Outlet />;
  return <ProtectedRoute requiredModule="messaging" requiredPermission="messaging:view"><CommunicationHome /></ProtectedRoute>;
}

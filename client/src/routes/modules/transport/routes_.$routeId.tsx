import { createFileRoute } from "@tanstack/react-router";
import { ProtectedRoute } from "@/components/protected-route";
import { TransportRouteWorkspace } from "@/modules/transport";
export const Route = createFileRoute("/modules/transport/routes_/$routeId")({ component: RoutePage });
function RoutePage() { const { routeId } = Route.useParams(); return <ProtectedRoute requiredModule="transport" requiredPermission="transport:view"><TransportRouteWorkspace routeId={routeId} /></ProtectedRoute>; }

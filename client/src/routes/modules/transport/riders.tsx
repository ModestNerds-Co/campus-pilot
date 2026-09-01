import { createFileRoute } from "@tanstack/react-router";
import { ProtectedRoute } from "@/components/protected-route";
import { TransportRidersWorkspace } from "@/modules/transport";
export const Route = createFileRoute("/modules/transport/riders")({ component: () => <ProtectedRoute requiredModule="transport" requiredPermission="transport:view"><TransportRidersWorkspace /></ProtectedRoute> });

import { createFileRoute } from "@tanstack/react-router";
import { ProtectedRoute } from "@/components/protected-route";
import { TransportRoutesWorkspace } from "@/modules/transport";
export const Route = createFileRoute("/modules/transport/routes")({ component: () => <ProtectedRoute requiredModule="transport" requiredPermission="transport:view"><TransportRoutesWorkspace /></ProtectedRoute> });

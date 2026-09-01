import { createFileRoute } from "@tanstack/react-router";
import { ProtectedRoute } from "@/components/protected-route";
import { TransportRunsWorkspace } from "@/modules/transport";
export const Route = createFileRoute("/modules/transport/runs")({ component: () => <ProtectedRoute requiredModule="transport" requiredPermission="transport:view"><TransportRunsWorkspace /></ProtectedRoute> });

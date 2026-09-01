import { createFileRoute } from "@tanstack/react-router";
import { ProtectedRoute } from "@/components/protected-route";
import { TransportRunWorkspace } from "@/modules/transport";
export const Route = createFileRoute("/modules/transport/runs_/$runId")({ component: RunPage });
function RunPage() { const { runId } = Route.useParams(); return <ProtectedRoute requiredModule="transport" requiredPermission="transport:view"><TransportRunWorkspace runId={runId} /></ProtectedRoute>; }

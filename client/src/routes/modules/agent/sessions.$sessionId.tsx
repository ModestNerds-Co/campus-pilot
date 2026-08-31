import { createFileRoute } from "@tanstack/react-router";

import { ProtectedRoute } from "@/components/protected-route";
import { AgentWorkspace } from "@/modules/agent";

export const Route = createFileRoute("/modules/agent/sessions/$sessionId")({
  component: AgentSessionRoute,
});

function AgentSessionRoute() {
  const { sessionId } = Route.useParams();
  return <ProtectedRoute requiredModule="agent" requiredPermission="agent:view"><AgentWorkspace selectedSessionId={sessionId} /></ProtectedRoute>;
}


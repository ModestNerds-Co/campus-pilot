import { createFileRoute } from "@tanstack/react-router";

import { ProtectedRoute } from "@/components/protected-route";
import { AgentWorkspace } from "@/modules/agent";

export const Route = createFileRoute("/modules/agent/usage")({
  component: () => <ProtectedRoute requiredModule="agent" requiredPermission="agent:view"><AgentWorkspace view="usage" /></ProtectedRoute>,
});


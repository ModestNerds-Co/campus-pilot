import { createFileRoute } from "@tanstack/react-router";

import { ProtectedRoute } from "@/components/protected-route";
import { AgentOverviewPage } from "@/modules/admin/components/agent/overview-page";

export const Route = createFileRoute("/admin/agent/")({
  component: () => (
    <ProtectedRoute requiredModule="agent" requiredPermission="agent_policy:view">
      <AgentOverviewPage />
    </ProtectedRoute>
  ),
});

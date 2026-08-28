/**
 * Licensed Administration route for tenant Agent provider routing.
 */

import { createFileRoute } from "@tanstack/react-router";

import { ProtectedRoute } from "@/components/protected-route";
import { AiRoutingPage } from "@/modules/admin/agent";

export const Route = createFileRoute("/admin/agent/routing")({
  component: () => (
    <ProtectedRoute requiredModule="agent" requiredPermission="ai_routing:view">
      <AiRoutingPage />
    </ProtectedRoute>
  ),
});

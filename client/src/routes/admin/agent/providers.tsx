/**
 * Licensed Administration route for tenant AI provider connections.
 */

import { createFileRoute } from "@tanstack/react-router";

import { ProtectedRoute } from "@/components/protected-route";
import { AiProvidersPage } from "@/modules/admin/agent";

export const Route = createFileRoute("/admin/agent/providers")({
  component: () => (
    <ProtectedRoute requiredModule="agent" requiredPermission="ai_providers:view">
      <AiProvidersPage />
    </ProtectedRoute>
  ),
});

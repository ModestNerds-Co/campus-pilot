import { createFileRoute } from "@tanstack/react-router";
import { z } from "zod";

import { ProtectedRoute } from "@/components/protected-route";
import { AgentCapabilitiesPage } from "@/modules/admin/components/agent/capabilities-page";

const searchSchema = z.object({
  search: z.string().max(120).optional().catch(undefined),
  module: z.string().max(80).optional().catch(undefined),
  exposure: z.enum(["exposed", "approval_required", "human_only", "prohibited"]).optional().catch(undefined),
  availability: z.enum(["executable", "module_unavailable", "approval_not_released", "handler_unavailable", "human_only", "prohibited"]).optional().catch(undefined),
  page: z.coerce.number().int().min(1).optional().catch(undefined),
  per_page: z.coerce.number().int().min(1).max(100).optional().catch(undefined),
});

export const Route = createFileRoute("/admin/agent/capabilities")({
  validateSearch: (search) => searchSchema.parse(search),
  component: AgentCapabilitiesRoute,
});

function AgentCapabilitiesRoute() {
  const search = Route.useSearch();
  const navigate = Route.useNavigate();
  return (
    <ProtectedRoute requiredModule="agent" requiredPermission="agent_policy:view">
      <AgentCapabilitiesPage filters={search} onFiltersChange={(next) => void navigate({ search: next })} />
    </ProtectedRoute>
  );
}

import { createFileRoute } from "@tanstack/react-router";
import { z } from "zod";

import { ProtectedRoute } from "@/components/protected-route";
import { AgentUsagePage } from "@/modules/admin/components/agent/usage-page";

const bounded = z.string().max(240).optional().catch(undefined);
const searchSchema = z.object({
  from: bounded,
  to: bounded,
  person_id: bounded,
  origin_module: bounded,
  capability_module: bounded,
  capability: bounded,
  provider: bounded,
  model: bounded,
  outcome: bounded,
  meter: bounded,
});

export const Route = createFileRoute("/admin/agent/usage")({
  validateSearch: (search) => searchSchema.parse(search),
  component: AgentUsageRoute,
});

function AgentUsageRoute() {
  const search = Route.useSearch();
  const navigate = Route.useNavigate();
  return (
    <ProtectedRoute requiredModule="agent" requiredPermission="agent_usage:view">
      <AgentUsagePage filters={search} onFiltersChange={(next) => void navigate({ search: next })} />
    </ProtectedRoute>
  );
}

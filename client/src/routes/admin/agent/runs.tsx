import { createFileRoute } from "@tanstack/react-router";
import { z } from "zod";

import { ProtectedRoute } from "@/components/protected-route";
import { AgentRunsPage } from "@/modules/admin/components/agent/runs-page";

const searchSchema = z.object({
  from: z.string().max(240).optional().catch(undefined),
  to: z.string().max(240).optional().catch(undefined),
  status: z.string().max(40).optional().catch(undefined),
  person_id: z.string().max(64).optional().catch(undefined),
  origin_module: z.string().max(80).optional().catch(undefined),
  correlation_id: z.string().max(64).optional().catch(undefined),
  search: z.string().max(120).optional().catch(undefined),
  page: z.coerce.number().int().min(1).optional().catch(undefined),
  per_page: z.coerce.number().int().min(1).max(100).optional().catch(undefined),
  run: z.string().max(64).optional().catch(undefined),
});

export const Route = createFileRoute("/admin/agent/runs")({
  validateSearch: (search) => searchSchema.parse(search),
  component: AgentRunsRoute,
});

function AgentRunsRoute() {
  const search = Route.useSearch();
  const navigate = Route.useNavigate();
  const { run, ...filters } = search;
  return (
    <ProtectedRoute requiredModule="agent" requiredPermission="agent_audit:view">
      <AgentRunsPage
        filters={filters}
        onFiltersChange={(next) => void navigate({ search: { ...next, run } })}
        onSelectedRunChange={(next) => void navigate({ search: { ...filters, run: next } })}
        selectedRunId={run}
      />
    </ProtectedRoute>
  );
}

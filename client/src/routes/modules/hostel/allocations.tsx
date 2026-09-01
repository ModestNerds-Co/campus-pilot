/** Hostel allocations route. */

import { createFileRoute } from "@tanstack/react-router";

import { ProtectedRoute } from "@/components/protected-route";
import { HostelAllocationsWorkspace } from "@/modules/hostel";

export const Route = createFileRoute("/modules/hostel/allocations")({
  component: () => <ProtectedRoute requiredModule="hostel" requiredPermission="hostel:view"><HostelAllocationsWorkspace /></ProtectedRoute>,
});

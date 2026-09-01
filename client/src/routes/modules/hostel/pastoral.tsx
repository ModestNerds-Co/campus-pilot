/** Hostel pastoral records route. */

import { createFileRoute } from "@tanstack/react-router";

import { ProtectedRoute } from "@/components/protected-route";
import { HostelPastoralWorkspace } from "@/modules/hostel";

export const Route = createFileRoute("/modules/hostel/pastoral")({
  component: () => <ProtectedRoute requiredModule="hostel" requiredPermission="hostel:view"><HostelPastoralWorkspace /></ProtectedRoute>,
});

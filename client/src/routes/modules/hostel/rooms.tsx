/** Hostel rooms route. */

import { createFileRoute } from "@tanstack/react-router";

import { ProtectedRoute } from "@/components/protected-route";
import { HostelRoomsWorkspace } from "@/modules/hostel";

export const Route = createFileRoute("/modules/hostel/rooms")({
  component: () => <ProtectedRoute requiredModule="hostel" requiredPermission="hostel:view"><HostelRoomsWorkspace /></ProtectedRoute>,
});

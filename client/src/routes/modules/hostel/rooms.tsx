/** Hostel rooms route. */

import { Navigate, createFileRoute } from "@tanstack/react-router";

import { ProtectedRoute } from "@/components/protected-route";
import { HostelRoomsWorkspace, hostelAccessProfile } from "@/modules/hostel";
import { useAuthStore } from "@/stores/auth-store";

export const Route = createFileRoute("/modules/hostel/rooms")({
  component: HostelRoomsRoute,
});

function HostelRoomsRoute() {
  const user = useAuthStore((state) => state.user);
  if (!hostelAccessProfile(user?.permissions ?? [], user?.record_scopes).hasCampusOccupancy) {
    return <Navigate replace to="/modules/hostel/allocations" />;
  }
  return <ProtectedRoute requiredModule="hostel" requiredPermission="hostel:view"><HostelRoomsWorkspace /></ProtectedRoute>;
}

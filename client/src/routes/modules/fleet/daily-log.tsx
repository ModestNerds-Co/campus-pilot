import { createFileRoute } from "@tanstack/react-router";

import { DailyLogList } from "@/modules/vehicle-log";
import { ProtectedRoute } from "@/components/protected-route";

export const Route = createFileRoute("/modules/fleet/daily-log")({
  component: () => (
    <ProtectedRoute requiredModule="fleet" requiredPermission="fleet:view">
      <DailyLogList />
    </ProtectedRoute>
  ),
});

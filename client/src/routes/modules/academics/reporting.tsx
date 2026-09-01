import { createFileRoute } from "@tanstack/react-router";

import { ProtectedRoute } from "@/components/protected-route";
import { ReportingWorkspace } from "@/modules/reporting";

export const Route = createFileRoute("/modules/academics/reporting")({
  component: () => <ProtectedRoute requiredModule="academics" requiredPermission="academics:view"><ReportingWorkspace /></ProtectedRoute>,
});

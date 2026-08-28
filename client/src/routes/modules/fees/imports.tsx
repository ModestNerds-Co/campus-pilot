import { createFileRoute } from "@tanstack/react-router";

import { ProtectedRoute } from "@/components/protected-route";
import { FeesImportsWorkspace } from "@/modules/fees";

export const Route = createFileRoute("/modules/fees/imports")({
  component: () => <ProtectedRoute requiredModule="fees" requiredPermission="fees:create"><FeesImportsWorkspace /></ProtectedRoute>,
});

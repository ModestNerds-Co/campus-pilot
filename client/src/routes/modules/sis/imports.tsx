/** Routes authorised SIS users into the staged data-import workspace. */
import { createFileRoute } from "@tanstack/react-router";

import { ProtectedRoute } from "@/components/protected-route";
import { SisImportsWorkspace } from "@/modules/sis";

export const Route = createFileRoute("/modules/sis/imports")({
  component: () => <ProtectedRoute requiredModule="sis" requiredPermission="sis:view"><SisImportsWorkspace /></ProtectedRoute>,
});

/** Routes authorised SIS users into the staged data-import workspace. */
import { createFileRoute } from "@tanstack/react-router";

import { ProtectedRoute } from "@/components/protected-route";
import { SisImportsWorkspace } from "@/modules/sis";
import { SIS_IMPORT_ACCESS_PERMISSIONS } from "@/modules/sis/access";

export const Route = createFileRoute("/modules/sis/imports")({
  component: () => <ProtectedRoute requiredAnyPermissions={SIS_IMPORT_ACCESS_PERMISSIONS} requiredModule="sis" requiredPermission="sis:view"><SisImportsWorkspace /></ProtectedRoute>,
});

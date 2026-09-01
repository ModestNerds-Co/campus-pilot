import { createFileRoute } from "@tanstack/react-router";
import { ProtectedRoute } from "@/components/protected-route";
import { ApplicationsList } from "@/modules/sis";
import { SIS_ADMINISTRATION_PERMISSIONS } from "@/modules/sis/access";

export const Route = createFileRoute("/modules/sis/applications")({
  component: () => <ProtectedRoute requiredAnyPermissions={SIS_ADMINISTRATION_PERMISSIONS} requiredModule="sis" requiredPermission="sis:view"><ApplicationsList /></ProtectedRoute>,
});

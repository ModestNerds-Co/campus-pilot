import { createFileRoute } from "@tanstack/react-router";
import { ProtectedRoute } from "@/components/protected-route";
import { ApplicationsList } from "@/modules/sis";

export const Route = createFileRoute("/modules/sis/applications")({
  component: () => <ProtectedRoute requiredModule="sis" requiredPermission="sis:view"><ApplicationsList /></ProtectedRoute>,
});

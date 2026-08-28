import { createFileRoute } from "@tanstack/react-router";
import { ProtectedRoute } from "@/components/protected-route";
import { GuardianRelationshipsList } from "@/modules/sis";

export const Route = createFileRoute("/modules/sis/guardian-relationships")({
  component: () => <ProtectedRoute requiredModule="sis" requiredPermission="sis:view"><GuardianRelationshipsList /></ProtectedRoute>,
});

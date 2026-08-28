import { createFileRoute } from "@tanstack/react-router";
import { ProtectedRoute } from "@/components/protected-route";
import { EnrolmentsList } from "@/modules/sis";

export const Route = createFileRoute("/modules/sis/enrolments")({
  component: () => <ProtectedRoute requiredModule="sis" requiredPermission="sis:view"><EnrolmentsList /></ProtectedRoute>,
});

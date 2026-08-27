import { createFileRoute } from "@tanstack/react-router";
import { ProtectedRoute } from "@/components/protected-route";
import { TeachingAssignmentsList } from "@/modules/academics";

export const Route = createFileRoute("/modules/academics/teaching-assignments")({
  component: () => <ProtectedRoute requiredModule="academics" requiredPermission="academics:view"><TeachingAssignmentsList /></ProtectedRoute>,
});

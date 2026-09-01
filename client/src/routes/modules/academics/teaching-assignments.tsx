import { createFileRoute } from "@tanstack/react-router";
import { ProtectedRoute } from "@/components/protected-route";
import { TeachingAssignmentsList } from "@/modules/academics";
import { ACADEMIC_ADMINISTRATION_PERMISSIONS } from "@/modules/academics/access";

export const Route = createFileRoute("/modules/academics/teaching-assignments")({
  component: () => <ProtectedRoute requiredAnyPermissions={ACADEMIC_ADMINISTRATION_PERMISSIONS} requiredModule="academics"><TeachingAssignmentsList /></ProtectedRoute>,
});

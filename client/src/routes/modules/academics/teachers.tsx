import { createFileRoute } from "@tanstack/react-router";
import { ProtectedRoute } from "@/components/protected-route";
import { TeachersList } from "@/modules/academics";
import { ACADEMIC_ADMINISTRATION_PERMISSIONS } from "@/modules/academics/access";

export const Route = createFileRoute("/modules/academics/teachers")({
  component: () => <ProtectedRoute requiredAnyPermissions={ACADEMIC_ADMINISTRATION_PERMISSIONS} requiredModule="academics"><TeachersList /></ProtectedRoute>,
});

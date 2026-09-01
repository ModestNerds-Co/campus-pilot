import { createFileRoute } from "@tanstack/react-router";

import { ProtectedRoute } from "@/components/protected-route";
import { ACADEMIC_TEACHING_PERMISSIONS } from "@/modules/academics/access";
import { GradebookWorkspace } from "@/modules/gradebook";

export const Route = createFileRoute("/modules/academics/gradebook")({
  component: () => <ProtectedRoute requiredAnyPermissions={ACADEMIC_TEACHING_PERMISSIONS} requiredModule="academics"><GradebookWorkspace /></ProtectedRoute>,
});

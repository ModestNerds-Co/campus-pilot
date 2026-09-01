import { createFileRoute } from "@tanstack/react-router";

import { ProtectedRoute } from "@/components/protected-route";
import { StudentSupportCasesWorkspace } from "@/modules/student-support";

export const Route = createFileRoute("/modules/student-support/cases")({
  component: () => <ProtectedRoute requiredModule="student_support" requiredPermission="student_support:view"><StudentSupportCasesWorkspace /></ProtectedRoute>,
});

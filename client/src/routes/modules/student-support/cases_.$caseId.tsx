import { createFileRoute } from "@tanstack/react-router";

import { ProtectedRoute } from "@/components/protected-route";
import { StudentSupportCaseWorkspace } from "@/modules/student-support";

export const Route = createFileRoute("/modules/student-support/cases_/$caseId")({ component: CaseRoute });

function CaseRoute() {
  const { caseId } = Route.useParams();
  return <ProtectedRoute requiredModule="student_support" requiredPermission="student_support:view"><StudentSupportCaseWorkspace caseId={caseId} /></ProtectedRoute>;
}

import { createFileRoute } from "@tanstack/react-router";
import { ProtectedRoute } from "@/components/protected-route";
import { AssessmentsList } from "@/modules/academics";

export const Route = createFileRoute("/modules/academics/assessments")({
  component: () => <ProtectedRoute requiredModule="academics" requiredPermission="academics:view"><AssessmentsList /></ProtectedRoute>,
});

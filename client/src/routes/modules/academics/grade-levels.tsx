import { createFileRoute } from "@tanstack/react-router";
import { ProtectedRoute } from "@/components/protected-route";
import { AcademicGradeLevelsList } from "@/modules/academics";

export const Route = createFileRoute("/modules/academics/grade-levels")({
  component: () => <ProtectedRoute requiredModule="academics" requiredPermission="academics:view"><AcademicGradeLevelsList /></ProtectedRoute>,
});

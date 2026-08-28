import { createFileRoute } from "@tanstack/react-router";

import { ProtectedRoute } from "@/components/protected-route";
import { AcademicTermsList } from "@/modules/academics";

export const Route = createFileRoute("/modules/academics/terms")({
  component: () => <ProtectedRoute requiredModule="academics" requiredPermission="academics:view"><AcademicTermsList /></ProtectedRoute>,
});

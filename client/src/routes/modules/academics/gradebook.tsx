import { createFileRoute } from "@tanstack/react-router";

import { ProtectedRoute } from "@/components/protected-route";
import { GradebookWorkspace } from "@/modules/gradebook";

export const Route = createFileRoute("/modules/academics/gradebook")({
  component: () => <ProtectedRoute requiredModule="academics" requiredPermission="academics:view"><GradebookWorkspace /></ProtectedRoute>,
});

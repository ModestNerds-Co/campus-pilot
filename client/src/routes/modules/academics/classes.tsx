import { createFileRoute } from "@tanstack/react-router";
import { ProtectedRoute } from "@/components/protected-route";
import { AcademicDirectoryList } from "@/modules/academics";

export const Route = createFileRoute("/modules/academics/classes")({
  component: () => <ProtectedRoute requiredModule="academics" requiredPermission="academics:view"><AcademicDirectoryList kind="class" /></ProtectedRoute>,
});

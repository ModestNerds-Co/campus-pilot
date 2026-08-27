import { createFileRoute } from "@tanstack/react-router";
import { ProtectedRoute } from "@/components/protected-route";
import { AcademicDirectoryList } from "@/modules/academics";

export const Route = createFileRoute("/modules/academics/academic-years")({
  component: () => <ProtectedRoute requiredModule="academics" requiredPermission="academics:view"><AcademicDirectoryList kind="academic-year" /></ProtectedRoute>,
});

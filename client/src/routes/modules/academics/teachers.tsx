import { createFileRoute } from "@tanstack/react-router";
import { ProtectedRoute } from "@/components/protected-route";
import { TeachersList } from "@/modules/academics";

export const Route = createFileRoute("/modules/academics/teachers")({
  component: () => <ProtectedRoute requiredModule="academics" requiredPermission="academics:view"><TeachersList /></ProtectedRoute>,
});

import { createFileRoute } from "@tanstack/react-router";
import { ProtectedRoute } from "@/components/protected-route";
import { SisPeopleList } from "@/modules/sis";

export const Route = createFileRoute("/modules/sis/learners")({
  component: () => <ProtectedRoute requiredModule="sis" requiredPermission="sis:view"><SisPeopleList kind="learner" /></ProtectedRoute>,
});

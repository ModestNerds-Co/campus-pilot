/** Routes SIS users to learner numbering settings. */
import { createFileRoute } from "@tanstack/react-router";

import { ProtectedRoute } from "@/components/protected-route";
import { LearnerNumberingPolicyPage } from "@/modules/sis";

export const Route = createFileRoute("/modules/sis/settings")({
  component: () => <ProtectedRoute requiredModule="sis" requiredPermission="sis:edit"><LearnerNumberingPolicyPage /></ProtectedRoute>,
});

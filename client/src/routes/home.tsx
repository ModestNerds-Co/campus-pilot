import { createFileRoute } from "@tanstack/react-router";

import { ProtectedRoute } from "@/components/protected-route";
import { CampusHome } from "@/modules/platform/campus-home";

export const Route = createFileRoute("/home")({
  component: () => (
    <ProtectedRoute>
      <CampusHome />
    </ProtectedRoute>
  ),
});

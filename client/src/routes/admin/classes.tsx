import { createFileRoute } from "@tanstack/react-router";
import { School } from "lucide-react";

import { ComingSoon } from "@/components/coming-soon";

export const Route = createFileRoute("/admin/classes")({
  component: () => (
    <ComingSoon
      description="Build the grade, class and homeroom structure that anchors the academic year."
      highlights={["Grade definitions", "Class rosters", "Homeroom assignments"]}
      icon={School}
      title="Grades & classes"
    />
  ),
});

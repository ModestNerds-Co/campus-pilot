import { createFileRoute } from "@tanstack/react-router";
import { UserRoundCog } from "lucide-react";

import { ComingSoon } from "@/components/coming-soon";

export const Route = createFileRoute("/admin/staff")({
  component: () => (
    <ComingSoon
      description="Maintain the staff directory and connect each person to their campus responsibilities."
      highlights={["Employment records", "Department assignments", "Teaching responsibilities"]}
      icon={UserRoundCog}
      title="Staff"
    />
  ),
});

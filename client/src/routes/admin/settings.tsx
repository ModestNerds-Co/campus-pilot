import { createFileRoute } from "@tanstack/react-router";
import { Settings2 } from "lucide-react";

import { ComingSoon } from "@/components/coming-soon";

export const Route = createFileRoute("/admin/settings")({
  component: () => (
    <ComingSoon
      description="Manage the campus identity, academic defaults and platform-wide preferences."
      highlights={["School profile", "Academic calendar defaults", "Notifications and integrations"]}
      icon={Settings2}
      title="Settings"
    />
  ),
});

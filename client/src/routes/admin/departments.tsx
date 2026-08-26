import { createFileRoute } from "@tanstack/react-router";
import { Building2 } from "lucide-react";

import { ComingSoon } from "@/components/coming-soon";

export const Route = createFileRoute("/admin/departments")({
  component: () => (
    <ComingSoon
      description="Organise academic and operational teams around accountable department structures."
      highlights={["Department ownership", "Staff allocation", "Academic reporting lines"]}
      icon={Building2}
      title="Departments"
    />
  ),
});

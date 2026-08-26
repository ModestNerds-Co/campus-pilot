import { createFileRoute } from "@tanstack/react-router";
import { MessageSquare } from "lucide-react";
import { ComingSoon } from "@/components/coming-soon";

function Messaging() {
  return (
    <ComingSoon
      title="Messaging & Communications"
      description="SMS, email, and push announcements to parents and staff — bulk and targeted."
      icon={MessageSquare}
      highlights={["Bulk SMS & email announcements", "Targeted messaging by class or grade", "Delivery tracking"]}
    />
  );
}

export const Route = createFileRoute("/admin/messaging")({
  component: Messaging,
});

import { createFileRoute } from "@tanstack/react-router";
import { HeartPulse } from "lucide-react";
import { ComingSoon } from "@/components/coming-soon";

function HealthClinic() {
  return (
    <ComingSoon
      title="Health & Clinic"
      description="Student health records and incident logs for the school clinic or sick bay."
      icon={HeartPulse}
      highlights={["Student health records", "Incident and visit logs", "Medication and allergy alerts"]}
    />
  );
}

export const Route = createFileRoute("/admin/health")({
  component: HealthClinic,
});

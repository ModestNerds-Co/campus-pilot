import { createFileRoute } from "@tanstack/react-router";
import { Receipt } from "lucide-react";
import { ComingSoon } from "@/components/coming-soon";

function FeesPaymentPlans() {
  return (
    <ComingSoon
      title="Fees & Payment Plans"
      description="Fee structures, installment plans, arrears tracking, and parent statements — usually the most requested feature on day one."
      icon={Receipt}
      highlights={["Term fee structures per grade", "Installment payment plans", "Arrears and statement reminders"]}
    />
  );
}

export const Route = createFileRoute("/admin/fees")({
  component: FeesPaymentPlans,
});

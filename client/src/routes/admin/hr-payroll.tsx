import { createFileRoute } from "@tanstack/react-router";
import { Briefcase } from "lucide-react";
import { ComingSoon } from "@/components/coming-soon";

function HrPayroll() {
  return (
    <ComingSoon
      title="HR & Payroll"
      description="Staff contracts, leave management, and payroll runs — including Zimbabwe-specific PAYE and NSSA deductions."
      icon={Briefcase}
      highlights={["Staff contracts and leave", "Payroll runs with PAYE/NSSA", "Payslip history"]}
    />
  );
}

export const Route = createFileRoute("/admin/hr-payroll")({
  component: HrPayroll,
});

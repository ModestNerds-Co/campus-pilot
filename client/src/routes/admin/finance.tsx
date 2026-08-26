import { createFileRoute } from "@tanstack/react-router";
import { Landmark } from "lucide-react";
import { ComingSoon } from "@/components/coming-soon";

function FinanceAccounting() {
  return (
    <ComingSoon
      title="Finance & Accounting"
      description="Chart of accounts, general ledger, invoicing, receipts, and bank reconciliation — with USD/ZWL multi-currency support built in from the start."
      icon={Landmark}
      highlights={["Chart of accounts & general ledger", "Invoicing and receipting", "Bank reconciliation"]}
    />
  );
}

export const Route = createFileRoute("/admin/finance")({
  component: FinanceAccounting,
});

import { createFileRoute } from "@tanstack/react-router";
import { PackageSearch } from "lucide-react";
import { ComingSoon } from "@/components/coming-soon";

function ProcurementStores() {
  return (
    <ComingSoon
      title="Procurement & Stores"
      description="Suppliers, purchase requisitions, stock levels, and the school's asset register."
      icon={PackageSearch}
      highlights={["Supplier directory", "Purchase requisitions", "Stock and asset register"]}
    />
  );
}

export const Route = createFileRoute("/admin/procurement")({
  component: ProcurementStores,
});

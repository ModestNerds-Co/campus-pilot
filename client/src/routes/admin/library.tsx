import { createFileRoute } from "@tanstack/react-router";
import { Library as LibraryIcon } from "lucide-react";
import { ComingSoon } from "@/components/coming-soon";

function LibraryModule() {
  return (
    <ComingSoon
      title="Library"
      description="Catalog, circulation, and fines — tracking every book from the shelf to the student and back."
      icon={LibraryIcon}
      highlights={["Book catalog", "Borrow / return circulation", "Overdue fines"]}
    />
  );
}

export const Route = createFileRoute("/admin/library")({
  component: LibraryModule,
});

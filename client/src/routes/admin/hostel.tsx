import { createFileRoute } from "@tanstack/react-router";
import { BedDouble } from "lucide-react";
import { ComingSoon } from "@/components/coming-soon";

function HostelBoarding() {
  return (
    <ComingSoon
      title="Hostel & Boarding"
      description="Room allocation, boarder registers, and check-in/out tracking for boarding schools."
      icon={BedDouble}
      highlights={["Room and bed allocation", "Boarder registers", "Check-in / check-out logs"]}
    />
  );
}

export const Route = createFileRoute("/admin/hostel")({
  component: HostelBoarding,
});

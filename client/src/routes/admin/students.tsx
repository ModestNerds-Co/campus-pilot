import { createFileRoute } from "@tanstack/react-router";
import { GraduationCap } from "lucide-react";
import { ComingSoon } from "@/components/coming-soon";

function StudentInformation() {
  return (
    <ComingSoon
      title="Student Information System"
      description="Admissions, enrollment, records, and promotions — the foundation every other module (fees, academics, library) will reference."
      icon={GraduationCap}
      highlights={[
        "Student records with guardian and emergency contact details",
        "Admissions and enrollment workflow",
        "Class and grade promotion history",
      ]}
    />
  );
}

export const Route = createFileRoute("/admin/students")({
  component: StudentInformation,
});

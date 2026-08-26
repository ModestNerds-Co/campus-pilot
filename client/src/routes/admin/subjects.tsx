import { createFileRoute } from "@tanstack/react-router";
import { BookOpen } from "lucide-react";
import { ComingSoon } from "@/components/coming-soon";

function Academics() {
  return (
    <ComingSoon
      title="Academics"
      description="Timetabling, attendance, subjects, exams and results — the day-to-day academic operations of the school."
      icon={BookOpen}
      highlights={["Timetable builder", "Daily attendance registers", "Exam results and report cards"]}
    />
  );
}

export const Route = createFileRoute("/admin/subjects")({
  component: Academics,
});

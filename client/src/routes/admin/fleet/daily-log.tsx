import { createFileRoute } from "@tanstack/react-router";
import { DailyLogList } from "@/modules/vehicle-log";

export const Route = createFileRoute("/admin/fleet/daily-log")({
  component: DailyLogList,
});

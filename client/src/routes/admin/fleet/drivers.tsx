import { createFileRoute } from "@tanstack/react-router";
import { DriversList } from "@/modules/fleet";

export const Route = createFileRoute("/admin/fleet/drivers")({
  component: DriversList,
});

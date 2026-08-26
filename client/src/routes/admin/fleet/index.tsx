import { createFileRoute } from "@tanstack/react-router";
import { VehiclesList } from "@/modules/fleet";

export const Route = createFileRoute("/admin/fleet/")({
  component: VehiclesList,
});

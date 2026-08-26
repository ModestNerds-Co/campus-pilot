import { createFileRoute } from "@tanstack/react-router";

import { ModuleWorkspace } from "@/modules/platform/module-workspace";

export const Route = createFileRoute("/modules/$moduleKey")({
  component: ModuleRoute,
});

function ModuleRoute() {
  const { moduleKey } = Route.useParams();
  return <ModuleWorkspace moduleKey={moduleKey} />;
}

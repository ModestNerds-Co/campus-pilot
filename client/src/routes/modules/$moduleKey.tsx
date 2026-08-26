import { createFileRoute, redirect } from "@tanstack/react-router";

import { ModuleWorkspace } from "@/modules/platform/module-workspace";
import { moduleRouteKey } from "@/modules/platform/module-registry";

export const Route = createFileRoute("/modules/$moduleKey")({
  beforeLoad: ({ params }) => {
    const canonicalKey = moduleRouteKey(params.moduleKey);
    if (canonicalKey !== params.moduleKey) {
      throw redirect({
        params: { moduleKey: canonicalKey },
        replace: true,
        to: "/modules/$moduleKey",
      });
    }
  },
  component: ModuleRoute,
});

function ModuleRoute() {
  const { moduleKey } = Route.useParams();
  return <ModuleWorkspace moduleKey={moduleKey} />;
}

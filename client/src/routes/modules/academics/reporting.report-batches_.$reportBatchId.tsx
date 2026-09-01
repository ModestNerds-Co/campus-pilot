/** Direct report-batch route. The URL is the only source of record identity. */
import { createFileRoute } from "@tanstack/react-router";

import { ProtectedRoute } from "@/components/protected-route";
import { ReportBatchWorkspace } from "@/modules/reporting";

export const Route = createFileRoute("/modules/academics/reporting/report-batches_/$reportBatchId")({
  component: ReportBatchRoute,
});

function ReportBatchRoute() {
  const { reportBatchId } = Route.useParams();
  return <ProtectedRoute requiredModule="academics" requiredPermission="academics:view"><ReportBatchWorkspace reportBatchId={reportBatchId} /></ProtectedRoute>;
}

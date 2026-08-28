import { createFileRoute } from "@tanstack/react-router";

import { InvoicesWorkspace } from "@/modules/fees";

export const Route = createFileRoute("/modules/fees/invoices")({ component: InvoicesWorkspace });

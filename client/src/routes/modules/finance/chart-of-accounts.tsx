import { createFileRoute } from "@tanstack/react-router";

import { AccountsList } from "@/modules/finance";

export const Route = createFileRoute("/modules/finance/chart-of-accounts")({ component: AccountsList });

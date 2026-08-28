import { createFileRoute } from "@tanstack/react-router";

import { AccountingPeriods } from "@/modules/finance";

export const Route = createFileRoute("/modules/finance/accounting-periods")({ component: AccountingPeriods });

import { createFileRoute } from "@tanstack/react-router";

import { BillingAccountsList } from "@/modules/fees";

export const Route = createFileRoute("/modules/fees/billing-accounts")({ component: BillingAccountsList });

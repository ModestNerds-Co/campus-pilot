import { createFileRoute } from "@tanstack/react-router";

import { CurrenciesList } from "@/modules/finance";

export const Route = createFileRoute("/modules/finance/currencies")({ component: CurrenciesList });

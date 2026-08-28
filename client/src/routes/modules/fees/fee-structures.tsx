import { createFileRoute } from "@tanstack/react-router";

import { FeeStructuresList } from "@/modules/fees";

export const Route = createFileRoute("/modules/fees/fee-structures")({ component: FeeStructuresList });

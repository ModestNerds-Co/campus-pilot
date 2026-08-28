import { createFileRoute } from "@tanstack/react-router";

import { JournalsWorkspace } from "@/modules/finance";

export const Route = createFileRoute("/modules/finance/journals")({ component: JournalsWorkspace });

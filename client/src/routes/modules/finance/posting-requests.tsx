import { createFileRoute } from "@tanstack/react-router";

import { PostingRequestsWorkspace } from "@/modules/finance";

export const Route = createFileRoute("/modules/finance/posting-requests")({ component: PostingRequestsWorkspace });

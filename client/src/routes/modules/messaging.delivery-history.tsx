import { createFileRoute } from "@tanstack/react-router";
import { ProtectedRoute } from "@/components/protected-route";
import { DeliveryHistoryWorkspace } from "@/modules/messaging";

export const Route = createFileRoute("/modules/messaging/delivery-history")({ component: () => <ProtectedRoute requiredModule="messaging" requiredPermission="messaging:send"><DeliveryHistoryWorkspace /></ProtectedRoute> });

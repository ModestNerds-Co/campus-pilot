import { createFileRoute } from "@tanstack/react-router";
import { ProtectedRoute } from "@/components/protected-route";
import { CommunicationInboxWorkspace } from "@/modules/messaging";

export const Route = createFileRoute("/modules/messaging/inbox")({ component: () => <ProtectedRoute requiredModule="messaging" requiredPermission="messaging:view"><CommunicationInboxWorkspace /></ProtectedRoute> });

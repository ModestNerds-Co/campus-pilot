import { createFileRoute } from "@tanstack/react-router";
import { ProtectedRoute } from "@/components/protected-route";
import { AnnouncementWorkspace } from "@/modules/messaging";

export const Route = createFileRoute("/modules/messaging/announcements_/$announcementId")({ component: AnnouncementRoute });
function AnnouncementRoute() { const { announcementId } = Route.useParams(); return <ProtectedRoute requiredModule="messaging" requiredPermission="messaging:create"><AnnouncementWorkspace announcementId={announcementId} /></ProtectedRoute>; }

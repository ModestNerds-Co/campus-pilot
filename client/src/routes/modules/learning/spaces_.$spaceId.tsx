import { createFileRoute } from "@tanstack/react-router";
import { ProtectedRoute } from "@/components/protected-route";
import { LearningSpaceWorkspace } from "@/modules/learning";
export const Route=createFileRoute("/modules/learning/spaces_/$spaceId")({component:LearningSpaceRoute});
function LearningSpaceRoute(){const {spaceId}=Route.useParams();return <ProtectedRoute requiredModule="learning" requiredPermission="learning:view"><LearningSpaceWorkspace spaceId={spaceId}/></ProtectedRoute>}

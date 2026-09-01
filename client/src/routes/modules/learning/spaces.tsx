import { createFileRoute } from "@tanstack/react-router";
import { ProtectedRoute } from "@/components/protected-route";
import { LearningSpacesWorkspace } from "@/modules/learning";
export const Route=createFileRoute("/modules/learning/spaces")({component:()=> <ProtectedRoute requiredModule="learning" requiredPermission="learning:view"><LearningSpacesWorkspace/></ProtectedRoute>});

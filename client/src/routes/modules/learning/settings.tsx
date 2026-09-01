import { createFileRoute } from "@tanstack/react-router";
import { ProtectedRoute } from "@/components/protected-route";
import { LearningSettingsWorkspace } from "@/modules/learning";
export const Route=createFileRoute("/modules/learning/settings")({component:()=> <ProtectedRoute requiredModule="learning" requiredPermission="learning:manage"><LearningSettingsWorkspace/></ProtectedRoute>});

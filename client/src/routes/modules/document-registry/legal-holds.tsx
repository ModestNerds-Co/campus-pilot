import { createFileRoute } from "@tanstack/react-router";
import { ProtectedRoute } from "@/components/protected-route";
import { DocumentRegistryLegalHoldsWorkspace } from "@/modules/document-registry";

export const Route = createFileRoute("/modules/document-registry/legal-holds")({
  component: () => (
    <ProtectedRoute
      requiredModule="document_registry"
      requiredPermission="document_registry:manage"
      requiredRecordScope="document_registry.records"
      requiredRecordScopeKind="campus"
    >
      <DocumentRegistryLegalHoldsWorkspace />
    </ProtectedRoute>
  ),
});

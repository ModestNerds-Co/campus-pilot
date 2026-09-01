/**
 * Derives Health personas from effective permissions and record scope.
 * Mutation controls require campus visibility because the API never accepts
 * self-scoped clinical writes.
 */

export type HealthRecordScopes = Record<string, string>;

function hasPermission(permissions: readonly string[], permission: string) {
  return permissions.includes("*") || permissions.includes(permission);
}

export function healthAccessProfile(
  permissions: readonly string[],
  recordScopes: HealthRecordScopes | undefined,
) {
  const hasCampusPatients = recordScopes?.["health.patients"] === "campus";
  const hasCampusCare = recordScopes?.["health.care"] === "campus";
  const canReadReferences = hasCampusPatients && hasPermission(permissions, "health:manage");

  return {
    hasCampusPatients,
    hasCampusCare,
    isSelfService: !hasCampusPatients && !hasCampusCare,
    canAddPatient: canReadReferences && hasPermission(permissions, "health:create"),
    canEditCare: hasCampusCare && hasPermission(permissions, "health:edit"),
    canCreateVisit:
      hasCampusCare && canReadReferences && hasPermission(permissions, "health:create"),
    canManageMedication:
      hasCampusCare && canReadReferences && hasPermission(permissions, "health:medication"),
    canManageFollowUps:
      hasCampusCare && canReadReferences && hasPermission(permissions, "health:follow_up"),
  };
}

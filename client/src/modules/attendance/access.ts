export type AttendanceRecordScopes = Record<string, string> | undefined;

function hasPermission(permissions: readonly string[], permission: string) {
  return permissions.includes("*") || permissions.includes(permission);
}

export function attendanceAccessProfile(
  permissions: readonly string[],
  recordScopes: AttendanceRecordScopes,
) {
  const scope = recordScopes?.["attendance.registers"];
  const hasAssignedScope = scope === "assigned" || scope === "self_and_assigned";
  const hasCampusScope = scope === "campus";
  const hasOperationalScope = hasAssignedScope || hasCampusScope;

  return {
    scope,
    hasAssignedScope,
    hasCampusScope,
    canView: hasOperationalScope && hasPermission(permissions, "attendance:view"),
    canCreate: hasOperationalScope && hasPermission(permissions, "attendance:create"),
    canEdit: hasOperationalScope && hasPermission(permissions, "attendance:edit"),
    canSubmit: hasOperationalScope && hasPermission(permissions, "attendance:submit"),
    canManage: hasCampusScope && hasPermission(permissions, "attendance:manage"),
    canDelete: hasCampusScope && hasPermission(permissions, "attendance:delete"),
  };
}

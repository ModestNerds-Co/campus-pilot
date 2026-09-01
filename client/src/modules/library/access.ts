export interface LibraryAccessProfile {
  canBorrow: boolean;
  canCirculate: boolean;
  canManage: boolean;
  canManageCatalogue: boolean;
  isOperator: boolean;
}

export function libraryAccessProfile(permissions: readonly string[]): LibraryAccessProfile {
  const allowed = (permission: string) =>
    permissions.includes("*") || permissions.includes(permission);
  const canCirculate = allowed("library:circulate");
  const canManage = allowed("library:manage");
  const canManageCatalogue = ["library:create", "library:edit", "library:delete"].some(allowed);
  return {
    canBorrow: allowed("library:borrow"),
    canCirculate,
    canManage,
    canManageCatalogue,
    isOperator: canCirculate || canManage || canManageCatalogue,
  };
}

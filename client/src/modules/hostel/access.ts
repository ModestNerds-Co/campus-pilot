export type HostelRecordScopes = Record<string, string>;

export interface HostelAccessProfile {
  canAllocate: boolean;
  hasCampusOccupancy: boolean;
}

export function hostelAccessProfile(
  permissions: readonly string[],
  recordScopes: HostelRecordScopes | undefined,
): HostelAccessProfile {
  const allowed = (permission: string) =>
    permissions.includes("*") || permissions.includes(permission);
  const hasCampusOccupancy = recordScopes?.["hostel.occupancy"] === "campus";
  const canAllocate = hasCampusOccupancy && allowed("hostel:allocate");

  return {
    canAllocate,
    hasCampusOccupancy,
  };
}

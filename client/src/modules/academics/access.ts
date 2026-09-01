/**
 * Defines the client-side Academics administration boundary.
 * The API remains authoritative; this list prevents teaching-only users from
 * entering setup workspaces that they cannot administer.
 */

export const ACADEMIC_ADMINISTRATION_PERMISSIONS = [
  "academics:create",
  "academics:edit",
  "academics:delete",
  "academics:manage",
] as const;

export const ACADEMIC_TEACHING_PERMISSIONS = [
  "academics:teach",
  "academics:manage",
] as const;

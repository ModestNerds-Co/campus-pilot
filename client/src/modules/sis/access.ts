/**
 * Defines SIS workflow boundaries that are narrower than ordinary record viewing.
 * The API remains authoritative; these values keep navigation and route affordances
 * aligned with the exact operation permissions used by the server catalog.
 */

export const SIS_IMPORT_ACCESS_PERMISSIONS = ["sis:create"] as const;

/**
 * Admissions and SIS configuration are administrative workflows. A user with
 * only `sis:view` may still receive record-scoped learner access (for example,
 * a teacher viewing learners assigned to their classes), but must not be
 * presented with campus-wide admissions or setup screens.
 */
export const SIS_ADMINISTRATION_PERMISSIONS = [
  "sis:create",
  "sis:edit",
  "sis:delete",
] as const;

/**
 * Mirrors role-delegation affordances for Administration screens.
 * The API remains authoritative; these helpers only hide actions it will reject.
 */

import type { ApiEnvelope } from "./types";

export function hasPermission(permissions: string[] | undefined, permission: string) {
  return permissions?.includes("*") || permissions?.includes(permission) || false;
}

export function canDelegatePermissions(
  operatorPermissions: string[] | undefined,
  requestedPermissions: string[],
) {
  if (operatorPermissions?.includes("*")) return true;
  return requestedPermissions.every((permission) => permission !== "*");
}

export function apiErrorMessage(envelope: ApiEnvelope<unknown>, fallback: string) {
  if (envelope.message) return envelope.message;
  const issue = envelope.issues?.[0];
  if (typeof issue === "string") return issue;
  if (issue?.detail) return issue.detail;
  return fallback;
}

import type { ManifestStatus, RunStatus } from "./types";

export function displayValue(value: string) { return value.replace(/_/g, " ").replace(/\b\w/g, (letter) => letter.toUpperCase()); }
export function dateLabel(value: string | null) { if (!value) return "—"; return new Intl.DateTimeFormat(undefined, { day: "numeric", month: "short", year: "numeric" }).format(new Date(`${value}T00:00:00`)); }
export function dateTimeLabel(value: string) { return new Intl.DateTimeFormat(undefined, { day: "numeric", month: "short", year: "numeric", hour: "2-digit", minute: "2-digit" }).format(new Date(value)); }
export function statusTone(status: RunStatus | ManifestStatus | "active" | "inactive" | "ended" | "cancelled"): "neutral" | "info" | "warn" | "success" | "danger" {
  if (status === "active" || status === "completed" || status === "boarded") return "success";
  if (status === "boarding" || status === "expected") return "info";
  if (status === "departed" || status === "no_show") return "warn";
  if (status === "exception") return "danger";
  return "neutral";
}
export function allowed(permissions: string[], permission: string) { return permissions.includes("*") || permissions.includes(permission); }


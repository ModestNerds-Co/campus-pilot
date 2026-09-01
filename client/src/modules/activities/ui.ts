/** Activities labels, time presentation, and authority helpers. */

export function displayValue(value: string) { return value.replace(/_/g, " ").replace(/\b\w/g, (letter) => letter.toUpperCase()); }
export function formatDate(value: string | null) { if (!value) return "—"; return new Intl.DateTimeFormat(undefined, { day: "numeric", month: "short", year: "numeric" }).format(new Date(`${value}T00:00:00`)); }
export function formatDateTime(value: string) { return new Intl.DateTimeFormat(undefined, { day: "numeric", month: "short", year: "numeric", hour: "2-digit", minute: "2-digit" }).format(new Date(value)); }
export function toDateTimeLocal(value: string) { const date = new Date(value); const local = new Date(date.getTime() - date.getTimezoneOffset() * 60000); return local.toISOString().slice(0, 16); }
export function fromDateTimeLocal(value: string) { return new Date(value).toISOString(); }
export function allowed(permissions: string[], permission: string) { return permissions.includes("*") || permissions.includes(permission); }
export function statusTone(status: string): "neutral" | "info" | "warn" | "success" | "danger" {
  if (["active", "completed", "granted", "present"].includes(status)) return "success";
  if (["draft", "scheduled", "pending", "late"].includes(status)) return "warn";
  if (["cancelled", "declined", "withdrawn", "absent"].includes(status)) return "danger";
  if (["closed", "ended", "archived", "excused"].includes(status)) return "neutral";
  return "info";
}

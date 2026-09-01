// Shared presentation helpers for Internal Audit workspaces.

export function label(value: string) {
  return value.replace(/_/g, " ").replace(/\b\w/g, (letter) => letter.toUpperCase());
}

export function dateValue(value: string | null) {
  if (!value) return "—";
  return new Intl.DateTimeFormat(undefined, { dateStyle: "medium" }).format(new Date(`${value}T00:00:00`));
}

export function dateTime(value: string | null) {
  if (!value) return "—";
  return new Intl.DateTimeFormat(undefined, { dateStyle: "medium", timeStyle: "short" }).format(new Date(value));
}

export function tone(value: string): "neutral" | "info" | "success" | "warning" | "danger" {
  if (["approved", "issued", "closed"].includes(value)) return "success";
  if (["fieldwork", "reporting", "moderate"].includes(value)) return "info";
  if (["planned", "draft", "low"].includes(value)) return "neutral";
  if (value === "high") return "warning";
  if (value === "critical") return "danger";
  return "neutral";
}

export function allowed(permissions: string[], permission: string) {
  return permissions.includes("*") || permissions.includes(permission);
}

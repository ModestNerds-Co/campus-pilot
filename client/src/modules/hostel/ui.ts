/** Shared Hostel display formatting and status tones. */

export function displayValue(value: string | null | undefined) {
  if (!value) return "—";
  return value.replace(/_/g, " ").replace(/\b\w/g, (character) => character.toUpperCase());
}
export function statusTone(value: string): "neutral" | "info" | "success" | "warning" | "danger" {
  if (["active", "available", "resolved"].includes(value)) return "success";
  if (["critical", "safeguarding"].includes(value)) return "danger";
  if (["planned", "maintenance", "high"].includes(value)) return "warning";
  if (["wellbeing", "moderate"].includes(value)) return "info";
  return "neutral";
}
export function dateValue(value: string | null) {
  if (!value) return "—";
  return new Intl.DateTimeFormat(undefined, { dateStyle: "medium" }).format(new Date(`${value}T00:00:00`));
}
export function dateTime(value: string) {
  return new Intl.DateTimeFormat(undefined, { dateStyle: "medium", timeStyle: "short" }).format(new Date(value));
}
export function todayValue() {
  const now = new Date();
  const offset = now.getTimezoneOffset() * 60_000;
  return new Date(now.getTime() - offset).toISOString().slice(0, 10);
}
export function localDateTimeValue() {
  const now = new Date();
  const offset = now.getTimezoneOffset() * 60_000;
  return new Date(now.getTime() - offset).toISOString().slice(0, 16);
}

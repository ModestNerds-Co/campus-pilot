export function displayValue(value: string | null | undefined) {
  if (!value) return "—";
  return value.replace(/_/g, " ").replace(/\b\w/g, (character: string) => character.toUpperCase());
}
export function statusTone(value: string): "neutral" | "info" | "success" | "warning" | "danger" {
  if (["active", "open", "given", "returned_to_class", "completed"].includes(value)) return "success";
  if (["critical", "emergency_referral", "missed"].includes(value)) return "danger";
  if (["high", "suspended", "refused", "held"].includes(value)) return "warning";
  if (["moderate", "wellbeing", "follow_up"].includes(value)) return "info";
  return "neutral";
}
export function dateTime(value: string) {
  return new Intl.DateTimeFormat(undefined, { dateStyle: "medium", timeStyle: "short" }).format(new Date(value));
}

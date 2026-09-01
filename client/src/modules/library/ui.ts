export function displayValue(value: string) {
  return value
    .replace(/_/g, " ")
    .replace(/^./, (letter) => letter.toUpperCase());
}
export function optional(value: string) {
  const trimmed = value.trim();
  return trimmed ? trimmed : null;
}
export function statusTone(
  status: string,
): "neutral" | "warning" | "success" | "danger" {
  return ["active", "available", "ready", "submitted_to_fees"].includes(status)
    ? "success"
    : ["lost", "closed", "expired"].includes(status)
      ? "danger"
      : ["waiting", "repair", "suspended", "assessed", "on_loan"].includes(
            status,
          )
        ? "warning"
        : "neutral";
}
export function formatDate(value: string) {
  return new Intl.DateTimeFormat(undefined, {
    day: "numeric",
    month: "short",
    year: "numeric",
    timeZone: "UTC",
  }).format(new Date(`${value}T00:00:00Z`));
}
export function formatDateTime(value: string) {
  return new Intl.DateTimeFormat(undefined, {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(new Date(value));
}
export function formatMinor(
  amount: number,
  currency: string,
  minorUnits: number,
) {
  return new Intl.NumberFormat(undefined, {
    style: "currency",
    currency,
    minimumFractionDigits: minorUnits,
    maximumFractionDigits: minorUnits,
  }).format(amount / 10 ** minorUnits);
}

/** Shared operational labels and presentation helpers for Student Support. */

import type { CaseSeverity, CaseStatus } from "./types";

export function displayValue(value: string) {
  return value.replace(/_/g, " ").replace(/\b\w/g, (letter) => letter.toUpperCase());
}

export function statusTone(status: CaseStatus): "neutral" | "info" | "warn" | "success" | "danger" {
  if (status === "open") return "info";
  if (status === "active") return "warn";
  if (status === "escalated") return "danger";
  if (status === "resolved") return "success";
  return "neutral";
}

export function severityTone(severity: CaseSeverity): "neutral" | "info" | "warn" | "danger" {
  if (severity === "critical") return "danger";
  if (severity === "high") return "warn";
  if (severity === "moderate") return "info";
  return "neutral";
}

export function formatDate(value: string | null) {
  if (!value) return "—";
  return new Intl.DateTimeFormat(undefined, { day: "numeric", month: "short", year: "numeric" }).format(new Date(`${value}T00:00:00`));
}

export function formatDateTime(value: string) {
  return new Intl.DateTimeFormat(undefined, { day: "numeric", month: "short", year: "numeric", hour: "2-digit", minute: "2-digit" }).format(new Date(value));
}

export function localDateTimeValue() {
  const now = new Date();
  const offset = now.getTimezoneOffset() * 60_000;
  return new Date(now.getTime() - offset).toISOString().slice(0, 16);
}

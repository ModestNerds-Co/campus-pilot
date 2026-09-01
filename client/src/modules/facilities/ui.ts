/** Facilities labels and status presentation. */

import type { FacilityPriority, FacilityRequestStatus, FacilityWorkOrderStatus } from "./types";

export function displayValue(value: string) {
  return value.replace(/_/g, " ").replace(/\b\w/g, (letter) => letter.toUpperCase());
}

export function requestTone(status: FacilityRequestStatus): "neutral" | "info" | "warn" | "success" | "danger" {
  if (status === "open") return "info";
  if (status === "assigned") return "warn";
  if (status === "resolved") return "success";
  if (status === "cancelled") return "danger";
  return "neutral";
}

export function workOrderTone(status: FacilityWorkOrderStatus): "neutral" | "info" | "warn" | "success" | "danger" {
  if (status === "assigned") return "info";
  if (status === "in_progress" || status === "ready_for_inspection") return "warn";
  if (status === "completed") return "success";
  if (status === "cancelled") return "danger";
  return "neutral";
}

export function priorityTone(priority: FacilityPriority): "neutral" | "info" | "warn" | "danger" {
  if (priority === "urgent") return "danger";
  if (priority === "high") return "warn";
  if (priority === "normal") return "info";
  return "neutral";
}

export function formatDate(value: string | null) {
  if (!value) return "—";
  return new Intl.DateTimeFormat(undefined, { day: "numeric", month: "short", year: "numeric" }).format(new Date(`${value}T00:00:00`));
}

export function formatDateTime(value: string) {
  return new Intl.DateTimeFormat(undefined, { day: "numeric", month: "short", year: "numeric", hour: "2-digit", minute: "2-digit" }).format(new Date(value));
}

export function allowed(permissions: string[], permission: string) {
  return permissions.includes("*") || permissions.includes(permission);
}

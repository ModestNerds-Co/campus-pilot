import type { RunStatus } from "./types";

export function formatAgentDate(value: string) {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return "Unknown date";
  return new Intl.DateTimeFormat(undefined, {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(date);
}

export function runStatusLabel(status: RunStatus) {
  if (status === "awaiting_approval") return "Awaiting approval";
  return status.charAt(0).toUpperCase() + status.slice(1);
}

export function moduleContextLabel(moduleKey: string) {
  const labels: Record<string, string> = {
    administration: "Administration",
    academics: "Academics",
    agent: "Agent",
    assets_inventory: "Assets and inventory",
    fees: "Fees and billing",
    finance: "Finance",
    fleet: "Fleet",
    home: "All modules",
    hr_payroll: "HR and payroll",
    procurement: "Procurement",
    sis: "People and admissions",
    timetabling: "Timetabling",
    learning: "E-learning",
  };
  return labels[moduleKey] || moduleKey.replace(/[_-]/g, " ");
}

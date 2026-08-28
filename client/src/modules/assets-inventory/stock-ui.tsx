/** Shared operational presentation for the Assets stock ledger. */

import type { ReactNode } from "react";
import { Badge } from "@/components/ui/badge";
import type { StockMovementKind } from "./stock-types";

export function movementKindLabel(kind: StockMovementKind): string {
  return ({
    manual_receipt: "Manual receipt",
    issue: "Issue",
    transfer: "Transfer",
    adjustment: "Adjustment",
    goods_receipt_allocation: "Procurement receipt",
    reversal: "Reversal",
  })[kind];
}

export function MovementKindBadge({ kind }: { kind: StockMovementKind }) {
  const tone = kind === "reversal" ? "warning" : kind === "issue" ? "neutral" : kind === "adjustment" ? "outline" : "info";
  return <Badge tone={tone}>{movementKindLabel(kind)}</Badge>;
}

export function formatOperationalDate(value: string): string {
  const date = new Date(value.length === 10 ? `${value}T00:00:00` : value);
  return Number.isNaN(date.valueOf()) ? value : new Intl.DateTimeFormat(undefined, { day: "numeric", month: "short", year: "numeric" }).format(date);
}

export function formatOperationalDateTime(value: string): string {
  const date = new Date(value);
  return Number.isNaN(date.valueOf()) ? value : new Intl.DateTimeFormat(undefined, { dateStyle: "medium", timeStyle: "short" }).format(date);
}

export function StockFact({ label, value }: { label: string; value: ReactNode }) {
  return <div className="rounded-[var(--radius-lg)] border border-[var(--border)] bg-[var(--surface)] p-4"><p className="text-[11px] font-semibold uppercase tracking-[0.12em] text-[var(--text-subtle)]">{label}</p><div className="mt-2 text-sm font-medium text-[var(--text-strong)]">{value}</div></div>;
}

export function StockNotice({ children, danger = false }: { children: ReactNode; danger?: boolean }) {
  return <div className={`rounded-[var(--radius-lg)] border p-4 text-sm leading-6 ${danger ? "border-[var(--tone-danger-bd)] bg-[var(--tone-danger-bg)] text-[var(--tone-danger)]" : "border-[var(--border)] bg-[var(--surface-muted)] text-[var(--text-muted)]"}`}>{children}</div>;
}

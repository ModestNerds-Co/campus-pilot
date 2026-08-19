//
//  campus-pilot — StatusChip / StatusDot
//

import * as React from "react";
import { cn } from "@/lib/utils";

type StatusTone = "neutral" | "info" | "success" | "warn" | "danger" | "brand" | "pending";

const chipTone: Record<StatusTone, string> = {
  neutral: "bg-[var(--badge-neutral-bg)] text-[var(--badge-neutral-text)] border-[var(--border)]",
  brand:   "bg-[var(--badge-brand-bg)] text-[var(--badge-brand-text)] border-[var(--brand-100)]",
  info:    "bg-[var(--status-info-bg)] text-[var(--status-info-text)] border-[var(--status-info-border)]",
  success: "bg-[var(--status-success-bg)] text-[var(--status-success-text)] border-[var(--status-success-border)]",
  warn:    "bg-[var(--status-warning-bg)] text-[var(--status-warning-text)] border-[var(--status-warning-border)]",
  danger:  "bg-[var(--status-error-bg)] text-[var(--status-error-text)] border-[var(--status-error-border)]",
  pending: "bg-[var(--status-pending-bg)] text-[var(--status-pending-text)] border-[var(--status-pending-border)]",
};

const dotTone: Record<StatusTone, string> = {
  neutral: "bg-[var(--gray-400)]",
  brand:   "bg-[var(--brand)]",
  info:    "bg-[var(--brand)]",
  success: "bg-[var(--tone-success)]",
  warn:    "bg-[var(--tone-warn)]",
  danger:  "bg-[var(--tone-danger)]",
  pending: "bg-[var(--text-subtle)]",
};

export function StatusChip({ tone = "neutral", className, children, dot, ...props }: React.HTMLAttributes<HTMLSpanElement> & { tone?: StatusTone; dot?: boolean }) {
  return (
    <span className={cn("inline-flex items-center gap-1.5 rounded-full border px-2.5 py-0.5 text-xs font-medium", chipTone[tone], className)} {...props}>
      {dot ? <span aria-hidden className={cn("size-1.5 rounded-full", dotTone[tone])} /> : null}
      {children}
    </span>
  );
}

export function StatusDot({ tone = "neutral", className, ...props }: React.HTMLAttributes<HTMLSpanElement> & { tone?: StatusTone }) {
  return <span aria-hidden className={cn("inline-block size-2 rounded-full", dotTone[tone], className)} {...props} />;
}

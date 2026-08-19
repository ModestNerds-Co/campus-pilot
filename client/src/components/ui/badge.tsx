//
//  campus-pilot — Badge
//  Tones: neutral / brand / info / success / warn / danger
//

import * as React from "react";
import { cn } from "@/lib/utils";

type Tone = "neutral" | "brand" | "info" | "success" | "warn" | "warning" | "danger" | "outline";

const toneMap: Record<Tone, string> = {
  neutral: "bg-[var(--badge-neutral-bg)] text-[var(--badge-neutral-text)] border border-[var(--border)]",
  brand:   "bg-[var(--badge-brand-bg)] text-[var(--badge-brand-text)] border border-[var(--brand-100)]",
  info:    "bg-[var(--badge-info-bg)] text-[var(--badge-info-text)] border border-[var(--brand-100)]",
  success: "bg-[var(--badge-success-bg)] text-[var(--badge-success-text)] border border-[var(--tone-success-bd)]",
  warn:    "bg-[var(--badge-warning-bg)] text-[var(--badge-warning-text)] border border-[var(--tone-warn-bd)]",
  warning: "bg-[var(--badge-warning-bg)] text-[var(--badge-warning-text)] border border-[var(--tone-warn-bd)]",
  danger:  "bg-[var(--badge-danger-bg)] text-[var(--badge-danger-text)] border border-[var(--tone-danger-bd)]",
  outline: "bg-transparent text-[var(--text-muted)] border border-[var(--border)]",
};

export interface BadgeProps extends React.HTMLAttributes<HTMLSpanElement> {
  tone?: Tone;
  variant?: Tone;
  dot?: boolean;
}

export function Badge({ className, tone, variant, dot, children, ...props }: BadgeProps) {
  const t = tone ?? variant ?? "neutral";
  return (
    <span
      className={cn(
        "inline-flex items-center gap-1 rounded-full px-2.5 py-0.5 text-xs font-medium leading-none whitespace-nowrap",
        toneMap[t] ?? toneMap.neutral,
        className
      )}
      {...props}
    >
      {dot ? <span aria-hidden className="size-1.5 rounded-full bg-current" /> : null}
      {children}
    </span>
  );
}

export function BadgeGroup({ className, ...props }: React.ComponentProps<"div">) {
  return <div className={cn("inline-flex flex-wrap items-center gap-1.5", className)} {...props} />;
}

export function badgeVariants({ variant }: { variant?: Tone } = {}) {
  return cn("inline-flex items-center rounded-full px-2.5 py-0.5 text-xs font-medium", toneMap[variant ?? "neutral"]);
}

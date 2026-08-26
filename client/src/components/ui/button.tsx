//
//  campus-pilot — Button primitive
//  Token-driven Campus Pilot button variants.
//  Variants: primary / secondary / ghost / outline / destructive
//  Sizes: sm / md / lg / icon
//

import * as React from "react";
import { cn } from "@/lib/utils";

type Variant = "default" | "primary" | "secondary" | "ghost" | "outline" | "destructive" | "link";
type Size = "default" | "sm" | "md" | "lg" | "icon" | "icon-sm" | "icon-lg";

export interface ButtonProps extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: Variant;
  size?: Size;
  asChild?: boolean;
}

const variantStyles: Record<Variant, string> = {
  default:     "bg-[var(--action-primary-bg)] text-[var(--action-primary-fg)] hover:bg-[var(--action-primary-bg-hover)] active:bg-[var(--action-primary-bg-pressed)] shadow-sm border border-transparent",
  primary:     "bg-[var(--action-primary-bg)] text-[var(--action-primary-fg)] hover:bg-[var(--action-primary-bg-hover)] active:bg-[var(--action-primary-bg-pressed)] shadow-sm border border-transparent",
  secondary:   "bg-[var(--surface)] text-[var(--text-strong)] border border-[var(--border)] hover:bg-[var(--surface-muted)] hover:border-[var(--border-strong)] active:bg-[var(--surface-sunken)] shadow-sm",
  outline:     "bg-[var(--surface)] text-[var(--text-strong)] border border-[var(--border)] hover:bg-[var(--surface-muted)] active:bg-[var(--surface-sunken)]",
  ghost:       "bg-transparent text-[var(--text-body)] hover:bg-[var(--button-ghost-hover-bg)] active:bg-[var(--surface-sunken)] border border-transparent",
  destructive: "bg-[var(--tone-danger)] text-[var(--on-brand)] hover:bg-[var(--tone-danger-strong)] active:bg-[var(--tone-danger-strong)] active:brightness-90 shadow-sm border border-transparent",
  link:        "bg-transparent text-[var(--text-link)] underline-offset-4 hover:underline border border-transparent p-0 h-auto",
};

const sizeStyles: Record<Size, string> = {
  default:  "h-[var(--h-control-md)] px-4 py-2 text-[13px]",
  sm:       "h-[var(--h-control-sm)] px-3 text-[12px]",
  md:       "h-[var(--h-control-md)] px-4 py-2 text-[13px]",
  lg:       "h-10 px-6 text-[14px]",
  icon:     "h-[var(--h-control-md)] w-[var(--h-control-md)] p-0",
  "icon-sm":"h-[var(--h-control-sm)] w-[var(--h-control-sm)] p-0",
  "icon-lg":"h-10 w-10 p-0",
};

export function Button({ className, variant = "default", size = "default", asChild, ...props }: ButtonProps) {
  // asChild not needed without Slot — we render a button always
  void asChild;
  return (
    <button
      className={cn(
        "inline-flex items-center justify-center gap-2 rounded-[var(--button-radius)] font-medium whitespace-nowrap",
        "transition-colors duration-200 ease-[var(--motion-ease-default)]",
        "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--focus-ring)] focus-visible:ring-offset-2",
        "disabled:pointer-events-none disabled:opacity-50 disabled:bg-[var(--action-disabled-bg)] disabled:text-[var(--action-disabled-fg)]",
        variantStyles[variant] ?? variantStyles.default,
        sizeStyles[size] ?? sizeStyles.default,
        className
      )}
      {...props}
    />
  );
}

export function buttonVariants(opts: { variant?: Variant; size?: Size } = {}) {
  const v = opts.variant ?? "default";
  const s = opts.size ?? "default";
  return cn("inline-flex items-center justify-center gap-2 rounded-[var(--button-radius)] font-medium", variantStyles[v], sizeStyles[s]);
}

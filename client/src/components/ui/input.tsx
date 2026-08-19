//
//  campus-pilot — Input / Textarea / Select
//  data-slot="input" is load-bearing for themed CSS.
//

import * as React from "react";
import { cn } from "@/lib/utils";

export interface InputProps extends React.InputHTMLAttributes<HTMLInputElement> {
  leadingIcon?: React.ReactNode;
  trailingIcon?: React.ReactNode;
}

export const Input = React.forwardRef<HTMLInputElement, InputProps>(
  ({ className, type = "text", leadingIcon, trailingIcon, ...props }, ref) => {
    if (leadingIcon || trailingIcon) {
      return (
        <div className="relative flex items-center">
          {leadingIcon ? <span className="pointer-events-none absolute left-3 text-[var(--text-muted)] [&_svg]:size-4">{leadingIcon}</span> : null}
          <input
            ref={ref}
            type={type}
            data-slot="input"
            className={cn(
              "flex h-[var(--h-control-md)] w-full rounded-[var(--radius-md)] border border-[var(--input-border)] bg-[var(--input-bg)] px-3 py-2 text-sm",
              "placeholder:text-[var(--text-subtle)] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--focus-ring)] focus-visible:ring-offset-0",
              "disabled:cursor-not-allowed disabled:opacity-50 disabled:bg-[var(--surface-muted)]",
              "aria-[invalid=true]:border-[var(--tone-danger)] aria-[invalid=true]:ring-[var(--tone-danger)]",
              leadingIcon && "pl-9",
              trailingIcon && "pr-9",
              className
            )}
            {...props}
          />
          {trailingIcon ? <span className="absolute right-3 text-[var(--text-muted)] [&_svg]:size-4">{trailingIcon}</span> : null}
        </div>
      );
    }
    return (
      <input
        ref={ref}
        type={type}
        data-slot="input"
        className={cn(
          "flex h-[var(--h-control-md)] w-full rounded-[var(--radius-md)] border border-[var(--input-border)] bg-[var(--input-bg)] px-3 py-2 text-sm",
          "placeholder:text-[var(--text-subtle)] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--focus-ring)] focus-visible:ring-offset-0",
          "disabled:cursor-not-allowed disabled:opacity-50 disabled:bg-[var(--surface-muted)]",
          "aria-[invalid=true]:border-[var(--tone-danger)]",
          className
        )}
        {...props}
      />
    );
  }
);
Input.displayName = "Input";

export const Textarea = React.forwardRef<HTMLTextAreaElement, React.TextareaHTMLAttributes<HTMLTextAreaElement>>(
  ({ className, ...props }, ref) => (
    <textarea
      ref={ref}
      data-slot="input"
      className={cn(
        "flex min-h-[80px] w-full rounded-[var(--radius-md)] border border-[var(--input-border)] bg-[var(--input-bg)] px-3 py-2 text-sm",
        "placeholder:text-[var(--text-subtle)] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--focus-ring)]",
        "disabled:cursor-not-allowed disabled:opacity-50",
        className
      )}
      {...props}
    />
  )
);
Textarea.displayName = "Textarea";

export const Select = React.forwardRef<HTMLSelectElement, React.SelectHTMLAttributes<HTMLSelectElement>>(
  ({ className, children, ...props }, ref) => (
    <select
      ref={ref}
      data-slot="input"
      className={cn(
        "flex h-[var(--h-control-md)] w-full rounded-[var(--radius-md)] border border-[var(--input-border)] bg-[var(--input-bg)] px-3 py-2 text-sm",
        "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--focus-ring)]",
        "disabled:cursor-not-allowed disabled:opacity-50",
        className
      )}
      {...props}
    >
      {children}
    </select>
  )
);
Select.displayName = "Select";

export function Label({ className, ...props }: React.LabelHTMLAttributes<HTMLLabelElement>) {
  return <label className={cn("text-sm font-medium leading-none text-[var(--text-strong)] peer-disabled:cursor-not-allowed peer-disabled:opacity-70", className)} {...props} />;
}

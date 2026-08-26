// campus-pilot — DataTable primitives (token-driven)
// Table chrome uses --table-* tokens; no gray/blue literals.
import * as React from "react";
import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { AlertTriangle, RotateCcw } from "lucide-react";

export function TableWrap({ className, ...props }: React.ComponentProps<"div">) {
  return (
    <div
      className={cn(
        "overflow-hidden rounded-[var(--radius-xl)] border border-[var(--border)] bg-[var(--surface)] shadow-[var(--shadow-card)]",
        className
      )}
      {...props}
    />
  );
}

export function TableScroll({ className, ...props }: React.ComponentProps<"div">) {
  return <div className={cn("overflow-x-auto", className)} {...props} />;
}

export function Table({ className, ...props }: React.ComponentProps<"table">) {
  return <table className={cn("w-full text-sm", className)} {...props} />;
}

export function THead({ className, ...props }: React.ComponentProps<"thead">) {
  return (
    <thead
      className={cn("bg-[var(--table-header-bg)]", className)}
      {...props}
    />
  );
}

export function TH({ className, ...props }: React.ComponentProps<"th">) {
  return (
    <th
      className={cn(
        "px-4 py-3 text-left text-xs font-medium uppercase tracking-wider text-[var(--table-header-text)] sm:px-6",
        className
      )}
      {...props}
    />
  );
}

export function TBody({ className, ...props }: React.ComponentProps<"tbody">) {
  return (
    <tbody
      className={cn("divide-y divide-[var(--table-divider)] bg-[var(--table-row-bg)]", className)}
      {...props}
    />
  );
}

export function TR({ className, ...props }: React.ComponentProps<"tr">) {
  return (
    <tr
      className={cn("hover:bg-[var(--table-row-hover-bg)] transition-colors", className)}
      {...props}
    />
  );
}

export function TD({ className, ...props }: React.ComponentProps<"td">) {
  return <td className={cn("px-4 py-4 sm:px-6", className)} {...props} />;
}

export function TableToolbar({ className, ...props }: React.ComponentProps<"div">) {
  return (
    <div
      className={cn(
        "flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between rounded-[var(--radius-xl)] border border-[var(--border)] bg-[var(--surface)] p-4",
        className
      )}
      {...props}
    />
  );
}

/** Unified DataTable controls row: search+submit,
 * filters, and pagination share one aligned row and one control height —
 * never a detached toolbar plus a separate footer. */
export function TableControlsBar({ className, ...props }: React.ComponentProps<"div">) {
  return (
    <div
      className={cn(
        "flex flex-col gap-3 sm:flex-row sm:items-center rounded-[var(--radius-xl)] border border-[var(--border)] bg-[var(--surface)] p-4 shadow-[var(--shadow-card)]",
        className
      )}
      {...props}
    />
  );
}

export function TableControlsSearch({ className, ...props }: React.ComponentProps<"form">) {
  return <form className={cn("flex min-w-0 flex-1 items-center gap-2", className)} {...props} />;
}

export function TableControlsPagination({
  page,
  totalPages,
  onPrevious,
  onNext,
  className,
}: {
  page: number;
  totalPages: number;
  onPrevious: () => void;
  onNext: () => void;
  className?: string;
}) {
  return (
    <div className={cn("flex items-center gap-3 sm:ml-auto", className)}>
      <span className="whitespace-nowrap text-sm text-[var(--text-muted)]">
        Page {page} of {totalPages}
      </span>
      <div className="flex gap-2">
        <Button variant="secondary" size="default" onClick={onPrevious} disabled={page === 1}>
          Previous
        </Button>
        <Button variant="secondary" size="default" onClick={onNext} disabled={page === totalPages}>
          Next
        </Button>
      </div>
    </div>
  );
}

export function TableEmpty({
  icon,
  title,
  description,
  className,
}: {
  icon?: React.ReactNode;
  title: string;
  description?: string;
  className?: string;
}) {
  return (
    <div className={cn("flex flex-col items-center justify-center gap-2 py-12 text-center", className)}>
      {icon ? <div className="mb-1 text-[var(--text-subtle)] [&_svg]:mx-auto [&_svg]:size-12 [&_svg]:text-[var(--text-subtle)] opacity-80">{icon}</div> : null}
      <p className="text-sm font-medium text-[var(--text-muted)]">{title}</p>
      {description ? <p className="text-sm text-[var(--text-subtle)]">{description}</p> : null}
    </div>
  );
}

export function TableLoading({
  columns = 5,
  rows = 5,
  label = "Loading records…",
}: {
  columns?: number;
  rows?: number;
  label?: string;
}) {
  return (
    <div aria-busy="true" aria-label={label} className="p-4 sm:p-5" role="status">
      <span className="sr-only">{label}</span>
      <div className="mb-4 grid gap-3" style={{ gridTemplateColumns: `repeat(${Math.min(columns, 4)}, minmax(0, 1fr))` }}>
        {Array.from({ length: Math.min(columns, 4) }).map((_, index) => (
          <div className="h-3 animate-pulse rounded-full bg-[var(--surface-sunken)]" key={index} />
        ))}
      </div>
      <div className="space-y-3">
        {Array.from({ length: rows }).map((_, row) => (
          <div className="flex items-center gap-3 border-t border-[var(--border-subtle)] pt-3" key={row}>
            <div className="size-9 shrink-0 animate-pulse rounded-[8px] bg-[var(--surface-sunken)]" />
            <div className="h-3 flex-1 animate-pulse rounded-full bg-[var(--surface-muted)]" />
            <div className="hidden h-3 w-28 animate-pulse rounded-full bg-[var(--surface-muted)] sm:block" />
            <div className="h-7 w-16 animate-pulse rounded-full bg-[var(--surface-sunken)]" />
          </div>
        ))}
      </div>
    </div>
  );
}

export function TableError({
  description,
  onRetry,
  title = "Records could not be loaded",
}: {
  description: string;
  onRetry: () => void;
  title?: string;
}) {
  return (
    <div className="flex flex-col items-start gap-4 p-6 sm:flex-row sm:items-center sm:p-8" role="alert">
      <span className="flex size-10 shrink-0 items-center justify-center rounded-[9px] bg-[var(--tone-danger-bg)] text-[var(--tone-danger)]">
        <AlertTriangle className="size-5" />
      </span>
      <div className="min-w-0 flex-1">
        <p className="text-sm font-semibold text-[var(--text-strong)]">{title}</p>
        <p className="mt-1 max-w-xl text-sm leading-5 text-[var(--text-muted)]">{description}</p>
      </div>
      <Button onClick={onRetry} type="button" variant="secondary">
        <RotateCcw className="size-4" />
        Try again
      </Button>
    </div>
  );
}

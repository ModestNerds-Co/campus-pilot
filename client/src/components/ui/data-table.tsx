// campus-pilot — DataTable primitives (token-driven)
// Table chrome uses --table-* tokens; no gray/blue literals.
import * as React from "react";
import { cn } from "@/lib/utils";

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

export function TablePagination({ className, ...props }: React.ComponentProps<"div">) {
  return (
    <div
      className={cn("flex items-center justify-between border-t border-[var(--border)] bg-[var(--surface)] px-6 py-4", className)}
      {...props}
    />
  );
}

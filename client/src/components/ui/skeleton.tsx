import * as React from "react";
import { cn } from "@/lib/utils";

export function Skeleton({ className, ...props }: React.HTMLAttributes<HTMLDivElement>) {
  return <div className={cn("animate-pulse rounded-[var(--radius-sm)] bg-[var(--surface-muted)]", className)} {...props} />;
}

export function Empty({
  icon,
  title,
  description,
  action,
  className,
}: {
  icon?: React.ReactNode;
  title: string;
  description?: string;
  action?: React.ReactNode;
  className?: string;
}) {
  return (
    <div className={cn("flex flex-col items-center justify-center gap-3 rounded-[var(--radius-xl)] border border-dashed border-[var(--border)] bg-[var(--surface)] px-8 py-12 text-center", className)}>
      {icon ? <div className="flex size-10 items-center justify-center rounded-full bg-[var(--surface-muted)] text-[var(--text-muted)] [&_svg]:size-5">{icon}</div> : null}
      <div>
        <p className="text-sm font-medium text-[var(--text-strong)]">{title}</p>
        {description ? <p className="mt-1 max-w-sm text-sm text-[var(--text-muted)]">{description}</p> : null}
      </div>
      {action ? <div className="mt-2">{action}</div> : null}
    </div>
  );
}

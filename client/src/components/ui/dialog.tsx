// campus-pilot — Dialog / Sheet primitives (token-driven)
// Backdrop uses --surface-overlay; panel uses --surface / --border / --shadow-modal.
import * as React from "react";
import { cn } from "@/lib/utils";
import { X } from "lucide-react";

export function DialogOverlay({ className, ...props }: React.ComponentProps<"div">) {
  return (
    <div
      className={cn("fixed inset-0 bg-[var(--surface-overlay)] backdrop-blur-[1px]", className)}
      {...props}
    />
  );
}

export function DialogPanel({
  className,
  children,
  ...props
}: React.ComponentProps<"div"> & { children: React.ReactNode }) {
  return (
    <div
      role="dialog"
      aria-modal="true"
      className={cn(
        "relative w-full max-w-2xl overflow-hidden rounded-[var(--radius-xl)] border border-[var(--border)] bg-[var(--surface)] shadow-[var(--shadow-modal)]",
        className
      )}
      {...props}
    >
      {children}
    </div>
  );
}

export function DialogHeader({ className, title, onClose, ...props }: React.ComponentProps<"div"> & { title: string; onClose?: () => void }) {
  return (
    <div
      className={cn(
        "flex items-center justify-between gap-4 border-b border-[var(--border)] px-6 py-4",
        className
      )}
      {...props}
    >
      <h2 className="text-base font-semibold leading-tight text-[var(--text-strong)]">{title}</h2>
      {onClose ? (
        <button
          type="button"
          onClick={onClose}
          aria-label="Close dialog"
          className="inline-flex size-8 items-center justify-center rounded-[var(--radius-md)] text-[var(--text-muted)] hover:bg-[var(--surface-muted)] hover:text-[var(--text-strong)] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--focus-ring)]"
        >
          <X className="size-4" />
        </button>
      ) : null}
    </div>
  );
}

export function DialogBody({ className, ...props }: React.ComponentProps<"div">) {
  return <div className={cn("p-6", className)} {...props} />;
}

export function DialogFooter({ className, ...props }: React.ComponentProps<"div">) {
  return (
    <div
      className={cn(
        "flex items-center justify-end gap-3 border-t border-[var(--border)] bg-[var(--surface-muted)] px-6 py-4",
        className
      )}
      {...props}
    />
  );
}

export function DialogShell({
  open,
  onClose,
  children,
}: {
  open: boolean;
  onClose: () => void;
  children: React.ReactNode;
}) {
  if (!open) return null;
  return (
    <div className="fixed inset-0 z-[var(--z-overlay)] overflow-y-auto">
      <div className="flex min-h-screen items-center justify-center p-4">
        <DialogOverlay onClick={onClose} />
        <DialogPanel>{children}</DialogPanel>
      </div>
    </div>
  );
}

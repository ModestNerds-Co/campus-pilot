// campus-pilot — right-side drawer primitives (token-driven)
import * as React from "react";
import { createPortal } from "react-dom";
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

export const DialogPanel = React.forwardRef<
  HTMLDivElement,
  React.ComponentProps<"div"> & { children: React.ReactNode }
>(({ className, children, ...props }, ref) => (
  <div
    aria-labelledby="dialog-title"
    aria-modal="true"
    className={cn(
      "cp-drawer-panel relative ml-auto flex h-[100dvh] min-w-0 w-full max-w-full flex-col overflow-hidden border-l border-[var(--border)] bg-[var(--surface)] shadow-[var(--shadow-modal)] sm:max-w-[640px] [&>form]:flex [&>form]:min-h-0 [&>form]:min-w-0 [&>form]:flex-1 [&>form]:flex-col",
      className
    )}
    ref={ref}
    role="dialog"
    tabIndex={-1}
    {...props}
  >
    {children}
  </div>
));
DialogPanel.displayName = "DialogPanel";

export function DialogHeader({ className, title, onClose, ...props }: React.ComponentProps<"div"> & { title: string; onClose?: () => void }) {
  return (
    <div
      className={cn(
        "flex shrink-0 items-center justify-between gap-4 border-b border-[var(--border)] px-5 py-4 sm:px-6 sm:py-5",
        className
      )}
      {...props}
    >
      <h2 className="text-base font-semibold leading-tight text-[var(--text-strong)]" id="dialog-title">{title}</h2>
      {onClose ? (
        <button
          type="button"
          onClick={onClose}
          aria-label="Close drawer"
          className="inline-flex size-10 items-center justify-center rounded-[var(--radius-md)] text-[var(--text-muted)] hover:bg-[var(--surface-muted)] hover:text-[var(--text-strong)] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--focus-ring)]"
        >
          <X className="size-4" />
        </button>
      ) : null}
    </div>
  );
}

export function DialogBody({ className, ...props }: React.ComponentProps<"div">) {
  return <div className={cn("min-h-0 min-w-0 flex-1 overflow-x-hidden overflow-y-auto overscroll-contain p-5 sm:p-6", className)} data-drawer-body {...props} />;
}

export function DialogFooter({ className, ...props }: React.ComponentProps<"div">) {
  return (
    <div
      className={cn(
        "flex shrink-0 flex-wrap items-center justify-end gap-3 border-t border-[var(--border)] bg-[var(--surface-muted)] px-5 py-4 sm:px-6",
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
  panelClassName,
}: {
  open: boolean;
  onClose: () => void;
  children: React.ReactNode;
  panelClassName?: string;
}) {
  const panelRef = React.useRef<HTMLDivElement>(null);
  const onCloseRef = React.useRef(onClose);

  React.useEffect(() => {
    onCloseRef.current = onClose;
  }, [onClose]);

  React.useEffect(() => {
    if (!open) return;
    const previousFocus = document.activeElement as HTMLElement | null;
    const previousScrollY = window.scrollY;
    const previousOverflow = document.body.style.overflow;
    const previousPosition = document.body.style.position;
    const previousTop = document.body.style.top;
    const previousWidth = document.body.style.width;
    const root = document.documentElement;
    const previousRootOverflow = root.style.overflow;
    document.body.style.overflow = "hidden";
    document.body.style.position = "fixed";
    document.body.style.top = `-${previousScrollY}px`;
    document.body.style.width = "100%";
    root.style.overflow = "hidden";

    const panel = panelRef.current;
    const focusableSelector =
      'button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), a[href], [tabindex]:not([tabindex="-1"])';
    const focusable = panel?.querySelectorAll<HTMLElement>(focusableSelector);
    const preferredFocus =
      panel?.querySelector<HTMLElement>('[data-autofocus="true"]') ||
      panel?.querySelector<HTMLElement>("input:not([disabled]), textarea:not([disabled]), select:not([disabled])");
    const focusTarget = preferredFocus || focusable?.[0] || panel;
    const focusFrame = window.requestAnimationFrame(() => focusTarget?.focus({ preventScroll: true }));

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        onCloseRef.current();
        return;
      }
      if (event.key !== "Tab" || !panel) return;
      const items = Array.from(panel.querySelectorAll<HTMLElement>(focusableSelector));
      if (items.length === 0) {
        event.preventDefault();
        panel.focus();
        return;
      }
      const first = items[0];
      const last = items[items.length - 1];
      if (event.shiftKey && document.activeElement === first) {
        event.preventDefault();
        last.focus();
      } else if (!event.shiftKey && document.activeElement === last) {
        event.preventDefault();
        first.focus();
      }
    };

    document.addEventListener("keydown", handleKeyDown);
    return () => {
      window.cancelAnimationFrame(focusFrame);
      document.body.style.overflow = previousOverflow;
      document.body.style.position = previousPosition;
      document.body.style.top = previousTop;
      document.body.style.width = previousWidth;
      root.style.overflow = previousRootOverflow;
      window.scrollTo(0, previousScrollY);
      document.removeEventListener("keydown", handleKeyDown);
      previousFocus?.focus({ preventScroll: true });
    };
  }, [open]);

  if (!open || typeof document === "undefined") return null;
  return createPortal(
    <div className="fixed inset-0 z-[var(--z-overlay)] overflow-hidden">
      <div className="flex min-h-[100dvh] items-stretch justify-end">
        <DialogOverlay onClick={onClose} />
        <DialogPanel className={panelClassName} ref={panelRef}>{children}</DialogPanel>
      </div>
    </div>,
    document.body,
  );
}

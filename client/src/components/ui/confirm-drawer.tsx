import { Loader2, TriangleAlert } from "lucide-react";

import { Button } from "@/components/ui/button";
import { DialogBody, DialogFooter, DialogHeader, DialogShell } from "@/components/ui/dialog";

export function ConfirmDrawer({
  cancelLabel = "Keep record",
  confirmLabel,
  description,
  isPending = false,
  onClose,
  onConfirm,
  open,
  title,
}: {
  cancelLabel?: string;
  confirmLabel: string;
  description: string;
  isPending?: boolean;
  onClose: () => void;
  onConfirm: () => void;
  open: boolean;
  title: string;
}) {
  return (
    <DialogShell onClose={isPending ? () => undefined : onClose} open={open}>
      <DialogHeader onClose={isPending ? undefined : onClose} title={title} />
      <DialogBody>
        <div className="flex gap-4">
          <span className="flex size-10 shrink-0 items-center justify-center rounded-[9px] bg-[var(--tone-danger-bg)] text-[var(--tone-danger)]">
            <TriangleAlert className="size-5" />
          </span>
          <p className="max-w-lg text-sm leading-6 text-[var(--text-muted)]">{description}</p>
        </div>
      </DialogBody>
      <DialogFooter>
        <Button data-autofocus="true" disabled={isPending} onClick={onClose} type="button" variant="secondary">
          {cancelLabel}
        </Button>
        <Button disabled={isPending} onClick={onConfirm} type="button" variant="destructive">
          {isPending ? <Loader2 className="size-4 animate-spin" /> : null}
          {isPending ? "Deleting…" : confirmLabel}
        </Button>
      </DialogFooter>
    </DialogShell>
  );
}

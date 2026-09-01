import { useEffect, useState } from "react";

import { ConfirmDrawer } from "@/components/ui/confirm-drawer";
import { DialogShell } from "@/components/ui/dialog";

export function GuardedDrawer({
  children,
  dirty,
  discardDescription,
  onClose,
  open,
  panelClassName,
  pending = false,
}: {
  children: (requestClose: () => void) => React.ReactNode;
  dirty: boolean;
  discardDescription: string;
  onClose: () => void;
  open: boolean;
  panelClassName?: string;
  pending?: boolean;
}) {
  const [discardOpen, setDiscardOpen] = useState(false);

  useEffect(() => {
    if (!open) setDiscardOpen(false);
  }, [open]);

  useEffect(() => {
    if (!open || !dirty) return;
    const warnBeforeUnload = (event: BeforeUnloadEvent) => {
      event.preventDefault();
      event.returnValue = "";
    };
    window.addEventListener("beforeunload", warnBeforeUnload);
    return () => window.removeEventListener("beforeunload", warnBeforeUnload);
  }, [dirty, open]);

  const requestClose = () => {
    if (pending) return;
    if (dirty) {
      setDiscardOpen(true);
      return;
    }
    onClose();
  };

  return (
    <>
      <DialogShell
        onClose={requestClose}
        open={open && !discardOpen}
        panelClassName={panelClassName}
      >
        {children(requestClose)}
      </DialogShell>
      <ConfirmDrawer
        cancelLabel="Keep editing"
        confirmLabel="Discard changes"
        description={discardDescription}
        onClose={() => setDiscardOpen(false)}
        onConfirm={() => {
          setDiscardOpen(false);
          onClose();
        }}
        open={open && discardOpen}
        title="Discard changes?"
      />
    </>
  );
}

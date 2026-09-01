// Communication announcement record, lifecycle actions, and delivery evidence.
import { useCallback, useEffect, useRef, useState } from "react";
import { Link, useNavigate } from "@tanstack/react-router";
import {
  ArrowLeft,
  Loader2,
  Pencil,
  RotateCcw,
  Send,
  Trash2,
  TriangleAlert,
  XCircle,
} from "lucide-react";
import toast from "react-hot-toast";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { ConfirmDrawer } from "@/components/ui/confirm-drawer";
import {
  Table,
  TableEmpty,
  TableError,
  TableLoading,
  TableScroll,
  TableWrap,
  TBody,
  TD,
  TH,
  THead,
  TR,
} from "@/components/ui/data-table";
import {
  DialogBody,
  DialogFooter,
  DialogHeader,
  DialogShell,
} from "@/components/ui/dialog";
import { Label, Textarea } from "@/components/ui/input";
import { usePageChrome } from "@/modules/admin/layouts/page-chrome";
import { useAuthStore } from "@/stores/auth-store";

import { AnnouncementDrawer } from "./announcement-drawer";
import {
  displayValue,
  formatDateTime,
  priorityTone,
  statusTone,
} from "./announcements-workspace";
import { communicationService, responseMessage } from "./service";
import type {
  AnnouncementDetail,
  AudiencePreview,
  CommunicationReferenceData,
  DeliveryRecord,
  MessagingListSearch,
} from "./types";

type Action = "submit" | "reopen" | "publish" | "cancel" | null;

export function AnnouncementWorkspace({
  announcementId,
  listSearch,
}: {
  announcementId: string;
  listSearch: MessagingListSearch;
}) {
  const navigate = useNavigate();
  const permissions = useAuthStore((state) => state.user?.permissions ?? []);
  const has = (permission: string) =>
    permissions.includes("*") || permissions.includes(permission);
  const canEdit = has("messaging:edit");
  const canSend = has("messaging:send");
  const [announcement, setAnnouncement] = useState<AnnouncementDetail | null>(null);
  const [references, setReferences] = useState<CommunicationReferenceData | null>(null);
  const [referencesLoading, setReferencesLoading] = useState(false);
  const [referencesError, setReferencesError] = useState<string | null>(null);
  const [deliveries, setDeliveries] = useState<DeliveryRecord[]>([]);
  const [deliveriesError, setDeliveriesError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [editOpen, setEditOpen] = useState(false);
  const [action, setAction] = useState<Action>(null);
  const [deleteOpen, setDeleteOpen] = useState(false);
  const [saving, setSaving] = useState(false);
  const referenceRequestRef = useRef(0);

  const loadDeliveries = useCallback(async () => {
    if (!canSend) return;
    setDeliveriesError(null);
    try {
      const history = await communicationService.deliveries(announcementId);
      if (!history.success || !history.data) {
        throw new Error(responseMessage(history, "Delivery history could not be loaded"));
      }
      setDeliveries(history.data);
    } catch (historyError) {
      setDeliveriesError(
        historyError instanceof Error
          ? historyError.message
          : "Delivery history could not be loaded",
      );
    }
  }, [announcementId, canSend]);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const response = await communicationService.readAnnouncement(announcementId);
      if (!response.success || !response.data) {
        throw new Error(responseMessage(response, "Announcement could not be loaded"));
      }
      setAnnouncement(response.data);
      await loadDeliveries();
    } catch (loadError) {
      setError(
        loadError instanceof Error ? loadError.message : "Announcement could not be loaded",
      );
    } finally {
      setLoading(false);
    }
  }, [announcementId, loadDeliveries]);

  const loadReferences = useCallback(async () => {
    if (!canEdit) return;
    const requestId = ++referenceRequestRef.current;
    setReferencesLoading(true);
    setReferencesError(null);
    try {
      const response = await communicationService.references();
      if (!response.success || !response.data) {
        throw new Error(responseMessage(response, "Audiences could not be loaded"));
      }
      if (requestId === referenceRequestRef.current) setReferences(response.data);
    } catch (referenceError) {
      if (requestId !== referenceRequestRef.current) return;
      setReferencesError(
        referenceError instanceof Error
          ? referenceError.message
          : "Audiences could not be loaded",
      );
    } finally {
      if (requestId === referenceRequestRef.current) setReferencesLoading(false);
    }
  }, [canEdit]);

  useEffect(() => {
    void load();
  }, [load]);
  useEffect(() => {
    void loadReferences();
  }, [loadReferences]);

  const actions = announcement ? (
    <div className="flex flex-wrap gap-2">
      {announcement.status === "draft" && canEdit ? (
        <>
          <Button onClick={() => setEditOpen(true)} variant="secondary">
            <Pencil className="size-4" /> Edit
          </Button>
          <Button onClick={() => setAction("submit")}>
            <Send className="size-4" /> Review recipients
          </Button>
        </>
      ) : null}
      {announcement.status === "submitted" && canSend ? (
        <Button onClick={() => setAction("publish")}>
          <Send className="size-4" /> Publish
        </Button>
      ) : null}
      {announcement.status === "submitted" && has("messaging:manage") ? (
        <Button onClick={() => setAction("reopen")} variant="secondary">
          <RotateCcw className="size-4" /> Reopen
        </Button>
      ) : null}
      {announcement.status === "published" && has("messaging:manage") ? (
        <Button onClick={() => setAction("cancel")} variant="secondary">
          <XCircle className="size-4" /> Cancel
        </Button>
      ) : null}
      {announcement.status === "draft" && has("messaging:delete") ? (
        <Button onClick={() => setDeleteOpen(true)} variant="ghost">
          <Trash2 className="size-4" /> Delete
        </Button>
      ) : null}
    </div>
  ) : null;
  usePageChrome(announcement?.title ?? "Announcement", actions);

  const deleteRecord = async () => {
    if (!announcement || saving) return;
    setSaving(true);
    try {
      const response = await communicationService.deleteAnnouncement(
        announcement.id,
        announcement.version,
      );
      if (!response.success) {
        throw new Error(responseMessage(response, "Announcement could not be deleted"));
      }
      toast.success("Announcement deleted");
      void navigate({ search: listSearch, to: "/modules/messaging" });
    } catch (deleteError) {
      toast.error(
        deleteError instanceof Error ? deleteError.message : "Announcement could not be deleted",
      );
      setSaving(false);
    }
  };

  if (loading) {
    return (
      <TableWrap>
        <TableLoading columns={4} label="Loading announcement…" />
      </TableWrap>
    );
  }
  if (error || !announcement) {
    return (
      <Unavailable
        description={error || "Announcement could not be loaded."}
        onRetry={() => void load()}
      />
    );
  }

  return (
    <div className="space-y-6">
      <Link
        className="inline-flex items-center gap-2 text-sm font-medium text-[var(--text-muted)] hover:text-[var(--brand-strong)]"
        search={listSearch}
        to="/modules/messaging"
      >
        <ArrowLeft className="size-4" /> Announcements
      </Link>
      <section className="border border-[var(--border)] bg-[var(--surface)]">
        <div className="flex flex-wrap items-center gap-2 border-b border-[var(--border)] p-5 sm:p-6">
          <Badge tone={statusTone(announcement.status)}>
            {displayValue(announcement.status)}
          </Badge>
          <Badge tone={priorityTone(announcement.priority)}>
            {displayValue(announcement.priority)}
          </Badge>
          <span className="text-xs text-[var(--text-muted)]">
            {announcement.creator_name} · updated {formatDateTime(announcement.updated_at)}
          </span>
        </div>
        <div className="p-5 sm:p-6">
          <p className="whitespace-pre-wrap text-sm leading-7 text-[var(--text-strong)]">
            {announcement.body}
          </p>
        </div>
      </section>
      <section>
        <h2 className="text-sm font-semibold text-[var(--text-strong)]">Audience</h2>
        <div className="mt-3 grid gap-3 sm:grid-cols-2">
          {announcement.targets.map((target) => (
            <div
              className="border border-[var(--border)] bg-[var(--surface)] p-4"
              key={target.id}
            >
              <p className="font-medium text-[var(--text-strong)]">{target.label}</p>
              <p className="mt-1 text-xs text-[var(--text-muted)]">
                {displayValue(target.kind)}
              </p>
            </div>
          ))}
        </div>
      </section>
      {announcement.reopen_reason ? (
        <OperationalNote
          label={`Reopened ${announcement.reopened_at ? formatDateTime(announcement.reopened_at) : ""}`}
          value={announcement.reopen_reason}
        />
      ) : null}
      {announcement.cancellation_reason ? (
        <OperationalNote
          label={`Cancelled ${announcement.cancelled_at ? formatDateTime(announcement.cancelled_at) : ""}`}
          value={announcement.cancellation_reason}
        />
      ) : null}
      {canSend ? (
        <DeliveryTable
          announcement={announcement}
          deliveries={deliveries}
          error={deliveriesError}
          onRetry={() => void loadDeliveries()}
        />
      ) : null}
      <AnnouncementDrawer
        announcement={announcement}
        onClose={() => setEditOpen(false)}
        onRetryReferences={() => void loadReferences()}
        onSaved={(value) => {
          setAnnouncement(value);
          setEditOpen(false);
        }}
        open={editOpen}
        references={references}
        referencesError={referencesError}
        referencesLoading={referencesLoading}
      />
      <TransitionDrawer
        action={action}
        announcement={announcement}
        onClose={() => setAction(null)}
        onCompleted={(value) => {
          setAnnouncement(value);
          setAction(null);
          void load();
        }}
      />
      <ConfirmDrawer
        confirmLabel="Delete draft"
        description="This removes the draft and its audience selections. Published communication is not deleted."
        isPending={saving}
        onClose={() => setDeleteOpen(false)}
        onConfirm={() => void deleteRecord()}
        open={deleteOpen}
        title="Delete announcement draft?"
      />
    </div>
  );
}

function TransitionDrawer({
  action,
  announcement,
  onClose,
  onCompleted,
}: {
  action: Action;
  announcement: AnnouncementDetail;
  onClose: () => void;
  onCompleted: (value: AnnouncementDetail) => void;
}) {
  const [preview, setPreview] = useState<AudiencePreview | null>(null);
  const [previewError, setPreviewError] = useState<string | null>(null);
  const [reason, setReason] = useState("");
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const previewRequestRef = useRef(0);
  const needsPreview = action === "submit" || action === "publish";

  const loadPreview = useCallback(async () => {
    if (!needsPreview) return;
    const requestId = ++previewRequestRef.current;
    setLoading(true);
    setPreview(null);
    setPreviewError(null);
    try {
      const response = await communicationService.audiencePreview(announcement.id);
      if (!response.success || !response.data) {
        throw new Error(responseMessage(response, "Recipient preview could not be loaded"));
      }
      if (requestId === previewRequestRef.current) setPreview(response.data);
    } catch (previewLoadError) {
      if (requestId !== previewRequestRef.current) return;
      setPreviewError(
        previewLoadError instanceof Error
          ? previewLoadError.message
          : "Recipient preview could not be loaded",
      );
    } finally {
      if (requestId === previewRequestRef.current) setLoading(false);
    }
  }, [announcement.id, needsPreview]);

  useEffect(() => {
    if (!action) return;
    setReason("");
    setPreview(null);
    setPreviewError(null);
    void loadPreview();
  }, [action, loadPreview]);

  const submit = async () => {
    if (!action || saving) return;
    setSaving(true);
    try {
      const response =
        action === "submit"
          ? await communicationService.submitAnnouncement(
              announcement.id,
              announcement.version,
            )
          : action === "publish"
            ? await communicationService.publishAnnouncement(
                announcement.id,
                announcement.version,
              )
            : action === "reopen"
              ? await communicationService.reopenAnnouncement(
                  announcement.id,
                  announcement.version,
                  reason.trim(),
                )
              : await communicationService.cancelAnnouncement(
                  announcement.id,
                  announcement.version,
                  reason.trim(),
                );
      if (!response.success || !response.data) {
        throw new Error(responseMessage(response, "Announcement could not be updated"));
      }
      toast.success(
        action === "submit"
          ? "Recipients fixed for review"
          : action === "publish"
            ? "Announcement published"
            : action === "reopen"
              ? "Announcement reopened"
              : "Announcement cancelled",
      );
      onCompleted(response.data);
    } catch (actionError) {
      toast.error(
        actionError instanceof Error ? actionError.message : "Announcement could not be updated",
      );
    } finally {
      setSaving(false);
    }
  };

  const needsReason = action === "reopen" || action === "cancel";
  const title =
    action === "submit"
      ? "Review recipients"
      : action === "publish"
        ? "Publish announcement"
        : action === "reopen"
          ? "Reopen announcement"
          : "Cancel announcement";
  const shownRecipients = preview?.recipients.slice(0, 20) ?? [];

  return (
    <DialogShell onClose={saving ? () => undefined : onClose} open={Boolean(action)}>
      <DialogHeader onClose={saving ? undefined : onClose} title={title} />
      <DialogBody className="space-y-5">
        {loading ? (
          <div className="flex items-center gap-2 text-sm text-[var(--text-muted)]">
            <Loader2 className="size-4 animate-spin" /> Resolving recipients…
          </div>
        ) : previewError ? (
          <div className="border border-[var(--tone-danger-bd)] bg-[var(--tone-danger-bg)] p-4">
            <div className="flex gap-3">
              <TriangleAlert className="mt-0.5 size-4 shrink-0 text-[var(--tone-danger)]" />
              <p className="text-sm text-[var(--tone-danger)]">{previewError}</p>
            </div>
            <Button
              className="mt-3"
              onClick={() => void loadPreview()}
              type="button"
              variant="secondary"
            >
              Try again
            </Button>
          </div>
        ) : preview ? (
          <div>
            <p className="text-3xl font-semibold text-[var(--text-strong)]">
              {preview.recipient_count}
            </p>
            <p className="mt-1 text-sm text-[var(--text-muted)]">
              active linked recipient{preview.recipient_count === 1 ? "" : "s"}
            </p>
            {shownRecipients.length > 0 ? (
              <div className="mt-4 border border-[var(--border)]">
                {shownRecipients.map((recipient) => (
                  <p
                    className="border-b border-[var(--border)] px-3 py-2 text-sm last:border-0"
                    key={recipient.id}
                  >
                    {recipient.full_name}
                  </p>
                ))}
              </div>
            ) : null}
            {preview.recipient_count > shownRecipients.length ? (
              <p className="mt-2 text-xs text-[var(--text-muted)]">
                {shownRecipients.length} shown · {preview.recipient_count - shownRecipients.length}{" "}
                more
              </p>
            ) : null}
          </div>
        ) : null}
        {action === "submit" ? (
          <p className="text-sm leading-6 text-[var(--text-muted)]">
            Submitting fixes this recipient list for publication. Later roster changes do not alter
            it.
          </p>
        ) : null}
        {action === "publish" ? (
          <p className="text-sm leading-6 text-[var(--text-muted)]">
            Publication delivers the reviewed announcement to each recipient's Campus Pilot inbox.
          </p>
        ) : null}
        {needsReason ? (
          <div>
            <Label htmlFor="transition-reason">Reason</Label>
            <Textarea
              className="mt-1.5"
              data-autofocus="true"
              id="transition-reason"
              maxLength={1000}
              onChange={(event) => setReason(event.target.value)}
              required
              value={reason}
            />
          </div>
        ) : null}
      </DialogBody>
      <DialogFooter>
        <Button disabled={saving} onClick={onClose} type="button" variant="secondary">
          Back
        </Button>
        <Button
          disabled={
            saving ||
            loading ||
            Boolean(previewError) ||
            (needsPreview && (!preview || preview.recipient_count === 0)) ||
            (needsReason && !reason.trim())
          }
          onClick={() => void submit()}
          type="button"
        >
          {saving ? <Loader2 className="size-4 animate-spin" /> : null}
          {action === "submit"
            ? "Fix recipients"
            : action === "publish"
              ? "Publish"
              : action === "reopen"
                ? "Reopen"
                : "Cancel announcement"}
        </Button>
      </DialogFooter>
    </DialogShell>
  );
}

function DeliveryTable({
  announcement,
  deliveries,
  error,
  onRetry,
}: {
  announcement: AnnouncementDetail;
  deliveries: DeliveryRecord[];
  error: string | null;
  onRetry: () => void;
}) {
  return (
    <section>
      <div className="flex flex-wrap items-baseline justify-between gap-3">
        <h2 className="text-sm font-semibold text-[var(--text-strong)]">Delivery history</h2>
        <span className="text-xs text-[var(--text-muted)]">
          In-app · {announcement.read_count} of {announcement.recipient_count} read
        </span>
      </div>
      <TableWrap className="mt-3">
        {error ? (
          <TableError description={error} onRetry={onRetry} />
        ) : deliveries.length === 0 ? (
          <TableEmpty
            description={
              announcement.status === "draft"
                ? "Recipient delivery records are created after review."
                : "No delivery records are available."
            }
            title="No deliveries"
          />
        ) : (
          <TableScroll>
            <Table>
              <THead>
                <tr>
                  <TH>Recipient</TH>
                  <TH>Channel</TH>
                  <TH>Status</TH>
                  <TH>Delivered</TH>
                  <TH>Read</TH>
                </tr>
              </THead>
              <TBody>
                {deliveries.map((delivery) => (
                  <TR key={delivery.id}>
                    <TD className="font-medium">{delivery.recipient_name}</TD>
                    <TD>In-app</TD>
                    <TD>
                      <Badge tone={delivery.status === "delivered" ? "success" : "warning"}>
                        {displayValue(delivery.status)}
                      </Badge>
                    </TD>
                    <TD>
                      {delivery.delivered_at ? formatDateTime(delivery.delivered_at) : "—"}
                    </TD>
                    <TD>{delivery.read_at ? formatDateTime(delivery.read_at) : "—"}</TD>
                  </TR>
                ))}
              </TBody>
            </Table>
          </TableScroll>
        )}
      </TableWrap>
    </section>
  );
}

function OperationalNote({ label, value }: { label: string; value: string }) {
  return (
    <div className="border-l-4 border-[var(--brand-strong)] bg-[var(--surface-muted)] p-4">
      <p className="text-xs font-semibold uppercase tracking-[0.12em] text-[var(--text-muted)]">
        {label}
      </p>
      <p className="mt-2 text-sm text-[var(--text-strong)]">{value}</p>
    </div>
  );
}

function Unavailable({ description, onRetry }: { description: string; onRetry: () => void }) {
  return (
    <div className="border border-[var(--border)] bg-[var(--surface)] p-8 text-center">
      <h1 className="text-lg font-semibold text-[var(--text-strong)]">
        Announcement unavailable
      </h1>
      <p className="mx-auto mt-2 max-w-lg text-sm text-[var(--text-muted)]">{description}</p>
      <Button className="mt-5" onClick={onRetry} variant="secondary">
        Retry
      </Button>
    </div>
  );
}

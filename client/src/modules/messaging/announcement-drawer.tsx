// Communication draft editor with interruption-safe dismissal.
import { useEffect, useMemo, useRef, useState } from "react";
import { Loader2, Plus, TriangleAlert, Trash2 } from "lucide-react";
import toast from "react-hot-toast";

import { Button } from "@/components/ui/button";
import { ConfirmDrawer } from "@/components/ui/confirm-drawer";
import {
  DialogBody,
  DialogFooter,
  DialogHeader,
  DialogShell,
} from "@/components/ui/dialog";
import { Input, Label, Select, Textarea } from "@/components/ui/input";

import { communicationService, responseMessage } from "./service";
import type {
  AnnouncementDetail,
  AnnouncementPayload,
  AnnouncementPriority,
  AudienceKind,
  AudienceTargetInput,
  CommunicationReferenceData,
} from "./types";

interface AnnouncementDrawerProps {
  announcement?: AnnouncementDetail | null;
  onClose: () => void;
  onRetryReferences: () => void;
  onSaved: (value: AnnouncementDetail) => void;
  open: boolean;
  references: CommunicationReferenceData | null;
  referencesError: string | null;
  referencesLoading: boolean;
}

export function AnnouncementDrawer({
  announcement,
  onClose,
  onRetryReferences,
  onSaved,
  open,
  references,
  referencesError,
  referencesLoading,
}: AnnouncementDrawerProps) {
  const [title, setTitle] = useState("");
  const [body, setBody] = useState("");
  const [priority, setPriority] = useState<AnnouncementPriority>("normal");
  const [targets, setTargets] = useState<AudienceTargetInput[]>([]);
  const [kind, setKind] = useState<AudienceKind>("class_group");
  const [targetValue, setTargetValue] = useState("");
  const [saving, setSaving] = useState(false);
  const [discardOpen, setDiscardOpen] = useState(false);
  const editorSessionRef = useRef<string | null>(null);

  const initialTargets = useMemo(
    () =>
      announcement?.targets.map(({ kind: itemKind, target_id, target_key, label }) => ({
        kind: itemKind,
        target_id,
        target_key,
        label,
      })) ?? [],
    [announcement],
  );
  const editorKey = announcement?.id ?? "new";

  useEffect(() => {
    if (!open) {
      editorSessionRef.current = null;
      setDiscardOpen(false);
      return;
    }
    if (editorSessionRef.current === editorKey) return;
    editorSessionRef.current = editorKey;
    setTitle(announcement?.title ?? "");
    setBody(announcement?.body ?? "");
    setPriority(announcement?.priority ?? "normal");
    setTargets(initialTargets);
    setKind(defaultAudienceKind(references));
    setTargetValue("");
    setDiscardOpen(false);
  }, [announcement, editorKey, initialTargets, open, references]);

  useEffect(() => {
    if (!open || !references || audienceKindAvailable(kind, references)) return;
    setKind(defaultAudienceKind(references));
    setTargetValue("");
  }, [kind, open, references]);

  const options = useMemo(() => audienceOptions(kind, references), [kind, references]);
  const dirty =
    title !== (announcement?.title ?? "") ||
    body !== (announcement?.body ?? "") ||
    priority !== (announcement?.priority ?? "normal") ||
    !targetsEqual(targets, initialTargets);

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
    if (saving) return;
    if (dirty) {
      setDiscardOpen(true);
      return;
    }
    onClose();
  };

  const addTarget = () => {
    if (!references) return;
    const option =
      kind === "campus"
        ? { value: "campus", label: "Entire campus" }
        : options.find((item) => item.value === targetValue);
    if (!option) {
      toast.error("Choose an audience");
      return;
    }
    const next: AudienceTargetInput = {
      kind,
      target_id: ["class_group", "department", "individual"].includes(kind)
        ? option.value
        : null,
      target_key: kind === "role" ? option.value : null,
      label: option.label,
    };
    const identity = targetIdentity(next);
    if (targets.some((item) => targetIdentity(item) === identity)) {
      toast.error("That audience is already selected");
      return;
    }
    setTargets((current) => [...current, next]);
    setTargetValue("");
  };

  const submit = async (event: React.FormEvent) => {
    event.preventDefault();
    if (saving || targets.length === 0) return;
    setSaving(true);
    const payload: AnnouncementPayload = {
      title: title.trim(),
      body: body.trim(),
      priority,
      targets,
    };
    try {
      const response = announcement
        ? await communicationService.updateAnnouncement(
            announcement.id,
            announcement.version,
            payload,
          )
        : await communicationService.createAnnouncement(payload);
      if (!response.success || !response.data) {
        throw new Error(responseMessage(response, "Announcement could not be saved"));
      }
      toast.success(announcement ? "Announcement updated" : "Draft created");
      onSaved(response.data);
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "Announcement could not be saved");
    } finally {
      setSaving(false);
    }
  };

  return (
    <>
      <DialogShell
        onClose={requestClose}
        open={open && !discardOpen}
        panelClassName="sm:max-w-[720px]"
      >
        <DialogHeader
          onClose={saving ? undefined : requestClose}
          title={announcement ? "Edit announcement" : "New announcement"}
        />
        <form onSubmit={submit}>
          <DialogBody className="space-y-6">
            <div>
              <Label htmlFor="announcement-title">Title</Label>
              <Input
                className="mt-1.5"
                data-autofocus="true"
                id="announcement-title"
                maxLength={180}
                onChange={(event) => setTitle(event.target.value)}
                required
                value={title}
              />
            </div>
            <div>
              <Label htmlFor="announcement-message">Message</Label>
              <Textarea
                className="mt-1.5 min-h-48 resize-y"
                id="announcement-message"
                maxLength={10000}
                onChange={(event) => setBody(event.target.value)}
                required
                value={body}
              />
            </div>
            <div>
              <Label htmlFor="announcement-priority">Priority</Label>
              <Select
                className="mt-1.5"
                id="announcement-priority"
                onChange={(event) =>
                  setPriority(event.target.value as AnnouncementPriority)
                }
                value={priority}
              >
                <option value="normal">Normal</option>
                <option value="important">Important</option>
                <option value="urgent">Urgent</option>
              </Select>
            </div>
            <section className="space-y-3 border-t border-[var(--border)] pt-5">
              <div>
                <h3 className="text-sm font-semibold text-[var(--text-strong)]">Audience</h3>
                <p className="mt-1 text-xs leading-5 text-[var(--text-muted)]">
                  Recipients are fixed when the draft is submitted for review.
                </p>
              </div>

              {referencesLoading && !references ? (
                <div className="flex items-center gap-2 text-sm text-[var(--text-muted)]">
                  <Loader2 className="size-4 animate-spin" /> Loading audiences…
                </div>
              ) : referencesError && !references ? (
                <div
                  className="border border-[var(--tone-danger-bd)] bg-[var(--tone-danger-bg)] p-4"
                  role="status"
                >
                  <div className="flex gap-3">
                    <TriangleAlert className="mt-0.5 size-4 shrink-0 text-[var(--tone-danger)]" />
                    <p className="text-sm text-[var(--tone-danger)]">{referencesError}</p>
                  </div>
                  <Button
                    className="mt-3"
                    onClick={onRetryReferences}
                    type="button"
                    variant="secondary"
                  >
                    Try again
                  </Button>
                </div>
              ) : references ? (
                <div className="grid gap-3 sm:grid-cols-[180px_1fr_auto]">
                  <Select
                    aria-label="Audience type"
                    onChange={(event) => {
                      setKind(event.target.value as AudienceKind);
                      setTargetValue("");
                    }}
                    value={kind}
                  >
                    {references.campus_allowed ? (
                      <option value="campus">Entire campus</option>
                    ) : null}
                    <option value="class_group">Class</option>
                    {references.campus_allowed ? (
                      <>
                        <option value="role">Role</option>
                        <option value="department">Department</option>
                        <option value="individual">Individual</option>
                      </>
                    ) : null}
                  </Select>
                  {kind === "campus" ? (
                    <div className="flex h-[var(--h-control-md)] items-center rounded-[var(--radius-md)] border border-[var(--border)] px-3 text-sm text-[var(--text-muted)]">
                      All active campus accounts
                    </div>
                  ) : (
                    <Select
                      aria-label="Audience"
                      onChange={(event) => setTargetValue(event.target.value)}
                      value={targetValue}
                    >
                      <option value="">Choose {audienceLabel(kind).toLowerCase()}</option>
                      {options.map((option) => (
                        <option key={option.value} value={option.value}>
                          {option.label}
                        </option>
                      ))}
                    </Select>
                  )}
                  <Button onClick={addTarget} type="button" variant="secondary">
                    <Plus className="size-4" /> Add
                  </Button>
                </div>
              ) : null}

              <div className="space-y-2">
                {targets.length === 0 ? (
                  <p className="border border-dashed border-[var(--border)] p-4 text-sm text-[var(--text-muted)]">
                    No audience selected.
                  </p>
                ) : (
                  targets.map((target) => (
                    <div
                      className="flex items-center justify-between gap-3 border border-[var(--border)] bg-[var(--surface-muted)] px-3 py-2.5"
                      key={targetIdentity(target)}
                    >
                      <div className="min-w-0">
                        <p className="truncate text-sm font-medium text-[var(--text-strong)]">
                          {target.label}
                        </p>
                        <p className="mt-0.5 text-xs text-[var(--text-muted)]">
                          {audienceLabel(target.kind)}
                        </p>
                      </div>
                      <Button
                        aria-label={`Remove ${target.label}`}
                        onClick={() =>
                          setTargets((current) =>
                            current.filter(
                              (item) => targetIdentity(item) !== targetIdentity(target),
                            ),
                          )
                        }
                        size="icon-sm"
                        type="button"
                        variant="ghost"
                      >
                        <Trash2 className="size-4" />
                      </Button>
                    </div>
                  ))
                )}
              </div>
            </section>
          </DialogBody>
          <DialogFooter>
            <Button disabled={saving} onClick={requestClose} type="button" variant="secondary">
              Cancel
            </Button>
            <Button
              disabled={saving || !title.trim() || !body.trim() || targets.length === 0}
              type="submit"
            >
              {saving ? <Loader2 className="size-4 animate-spin" /> : null}
              {saving ? "Saving…" : announcement ? "Save changes" : "Create draft"}
            </Button>
          </DialogFooter>
        </form>
      </DialogShell>

      <ConfirmDrawer
        cancelLabel="Keep editing"
        confirmLabel="Discard changes"
        description="The unsaved title, message, priority, and audience changes will be lost."
        onClose={() => setDiscardOpen(false)}
        onConfirm={() => {
          setDiscardOpen(false);
          onClose();
        }}
        open={open && discardOpen}
        title="Discard announcement changes?"
      />
    </>
  );
}

function audienceOptions(kind: AudienceKind, references: CommunicationReferenceData | null) {
  if (!references) return [];
  if (kind === "class_group") {
    return references.classes.map((item) => ({
      value: item.id,
      label: `${item.name} · ${item.code}`,
    }));
  }
  if (kind === "department") {
    return references.departments.map((item) => ({
      value: item.id,
      label: `${item.name} · ${item.code}`,
    }));
  }
  if (kind === "role") {
    return references.roles.map((item) => ({ value: item.key, label: item.name }));
  }
  if (kind === "individual") {
    return references.users.map((item) => ({
      value: item.id,
      label: `${item.full_name} · ${item.email}`,
    }));
  }
  return [];
}

function defaultAudienceKind(references: CommunicationReferenceData | null): AudienceKind {
  if (references?.classes.length) return "class_group";
  if (references?.campus_allowed) return "campus";
  return "class_group";
}

function audienceKindAvailable(
  kind: AudienceKind,
  references: CommunicationReferenceData,
) {
  return kind === "class_group" || references.campus_allowed;
}

function targetsEqual(left: AudienceTargetInput[], right: AudienceTargetInput[]) {
  if (left.length !== right.length) return false;
  return left.every(
    (target, index) =>
      targetIdentity(target) === targetIdentity(right[index]) && target.label === right[index].label,
  );
}

function targetIdentity(target: AudienceTargetInput) {
  return `${target.kind}:${target.target_id ?? ""}:${target.target_key ?? ""}`;
}

function audienceLabel(kind: AudienceKind) {
  return kind === "class_group"
    ? "Class"
    : kind === "department"
      ? "Department"
      : kind === "individual"
        ? "Individual"
        : kind === "role"
          ? "Role"
          : "Campus";
}

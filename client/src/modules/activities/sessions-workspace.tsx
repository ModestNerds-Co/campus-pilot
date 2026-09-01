/** Scheduled activity sessions, participation, completion, and cancellation. */

import { useCallback, useEffect, useMemo, useState } from "react";
import { CalendarClock, Plus, Search, UsersRound } from "lucide-react";
import toast from "react-hot-toast";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Table,
  TableControlsBar,
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
import { Input, Label, Select, Textarea } from "@/components/ui/input";
import { usePageChrome } from "@/modules/admin/layouts/page-chrome";
import { useAuthStore } from "@/stores/auth-store";

import { activitiesService, responseMessage } from "./service";
import type {
  ActivityGroupSummary,
  ActivityParticipation,
  ActivityParticipationMark,
  ActivitySessionRecord,
  ActivitySessionStatus,
  ActivitySessionSummary,
  SessionPayload,
} from "./types";
import {
  allowed,
  displayValue,
  formatDateTime,
  statusTone,
} from "./ui";

type SessionForm = Omit<SessionPayload, "starts_at" | "ends_at"> & {
  starts_at: string;
  ends_at: string;
};

type SessionAction = "edit" | "participation" | "complete" | "cancel";

const participationMarks: ActivityParticipationMark[] = [
  "present",
  "absent",
  "late",
  "excused",
  "not_required",
];

export function ActivitiesSessionsWorkspace() {
  const permissions = useAuthStore((state) => state.user?.permissions ?? []);
  const canOperate =
    allowed(permissions, "activities:operate") ||
    allowed(permissions, "activities:manage");
  const [records, setRecords] = useState<ActivitySessionSummary[]>([]);
  const [search, setSearch] = useState("");
  const [status, setStatus] = useState<ActivitySessionStatus | "all">(
    "scheduled",
  );
  const [selected, setSelected] = useState<ActivitySessionSummary | null>(null);
  const [createOpen, setCreateOpen] = useState(false);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const response = await activitiesService.sessions({
        page: 1,
        per_page: 100,
        search: search.trim() || undefined,
        status: status === "all" ? undefined : status,
      });
      if (!response.success || !response.data) {
        throw new Error(
          responseMessage(response, "Activity sessions could not be loaded"),
        );
      }
      setRecords(response.data);
    } catch (loadError) {
      setError(
        loadError instanceof Error
          ? loadError.message
          : "Activity sessions could not be loaded",
      );
    } finally {
      setLoading(false);
    }
  }, [search, status]);

  useEffect(() => {
    void load();
  }, [load]);

  usePageChrome(
    "Sessions",
    canOperate ? (
      <Button onClick={() => setCreateOpen(true)}>
        <Plus className="size-4" />
        Schedule session
      </Button>
    ) : null,
  );

  return (
    <div className="space-y-6">
      <TableControlsBar>
        <Input
          aria-label="Search activity sessions"
          className="sm:w-72"
          leadingIcon={<Search />}
          onChange={(event) => setSearch(event.target.value)}
          placeholder="Search session or group"
          value={search}
        />
        <Select
          aria-label="Session status"
          className="sm:w-44"
          onChange={(event) =>
            setStatus(event.target.value as ActivitySessionStatus | "all")
          }
          value={status}
        >
          <option value="scheduled">Scheduled</option>
          <option value="completed">Completed</option>
          <option value="cancelled">Cancelled</option>
          <option value="all">All statuses</option>
        </Select>
      </TableControlsBar>
      <TableWrap>
        {loading ? (
          <TableLoading columns={6} label="Loading activity sessions…" />
        ) : error ? (
          <TableError description={error} onRetry={() => void load()} />
        ) : records.length === 0 ? (
          <TableEmpty
            description={
              search || status !== "scheduled"
                ? "Change the current filters."
                : canOperate
                  ? "Schedule the first activity session."
                  : "No activity sessions are available to you."
            }
            icon={<CalendarClock />}
            title={
              search || status !== "scheduled"
                ? "No sessions match"
                : "No activity sessions"
            }
          />
        ) : (
          <TableScroll>
            <Table className="min-w-[980px]">
              <THead>
                <tr>
                  <TH>Session</TH>
                  <TH>Group</TH>
                  <TH>Time</TH>
                  <TH>Location</TH>
                  <TH>Participation</TH>
                  <TH>Status</TH>
                </tr>
              </THead>
              <TBody>
                {records.map((record) => (
                  <TR
                    className="cursor-pointer"
                    key={record.id}
                    onClick={() => setSelected(record)}
                  >
                    <TD>
                      <p className="font-medium text-[var(--text-strong)]">
                        {record.title}
                      </p>
                      <p className="mt-1 font-tabular text-xs text-[var(--text-muted)]">
                        {record.reference}
                      </p>
                    </TD>
                    <TD className="text-[var(--text-muted)]">
                      {record.group_name}
                    </TD>
                    <TD className="whitespace-nowrap text-[var(--text-muted)]">
                      {formatDateTime(record.starts_at)}
                    </TD>
                    <TD className="text-[var(--text-muted)]">
                      {record.location_note || "—"}
                    </TD>
                    <TD className="text-[var(--text-muted)]">
                      {record.marked_count}/{record.roster_count} marked
                    </TD>
                    <TD>
                      <Badge tone={statusTone(record.status)}>
                        {displayValue(record.status)}
                      </Badge>
                    </TD>
                  </TR>
                ))}
              </TBody>
            </Table>
          </TableScroll>
        )}
      </TableWrap>
      {canOperate ? (
        <SessionCreateDrawer
          onClose={() => setCreateOpen(false)}
          onSaved={() => {
            setCreateOpen(false);
            void load();
          }}
          open={createOpen}
        />
      ) : null}
      <SessionRecordDrawer
        canOperate={canOperate}
        onClose={() => setSelected(null)}
        onSaved={() => {
          setSelected(null);
          void load();
        }}
        open={selected !== null}
        summary={selected}
      />
    </div>
  );
}

function SessionCreateDrawer({
  onClose,
  onSaved,
  open,
}: {
  onClose: () => void;
  onSaved: () => void;
  open: boolean;
}) {
  const [groups, setGroups] = useState<ActivityGroupSummary[]>([]);
  const [form, setForm] = useState<SessionForm>(emptySessionForm());
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    if (!open) return;
    setForm(emptySessionForm());
    setLoading(true);
    void activitiesService
      .groups({ page: 1, per_page: 100, status: "active" })
      .then((response) => {
        if (response.success && response.data) setGroups(response.data);
        else
          toast.error(
            responseMessage(response, "Activity groups could not be loaded"),
          );
      })
      .finally(() => setLoading(false));
  }, [open]);

  const save = async (event: React.FormEvent) => {
    event.preventDefault();
    setSaving(true);
    try {
      const response = await activitiesService.createSession(toPayload(form));
      if (!response.success) {
        throw new Error(
          responseMessage(response, "Activity session could not be scheduled"),
        );
      }
      toast.success("Activity session scheduled");
      onSaved();
    } catch (saveError) {
      toast.error(
        saveError instanceof Error
          ? saveError.message
          : "Activity session could not be scheduled",
      );
    } finally {
      setSaving(false);
    }
  };

  return (
    <DialogShell onClose={onClose} open={open}>
      <DialogHeader
        onClose={saving ? undefined : onClose}
        title="Schedule activity session"
      />
      <form
        className="flex min-h-0 flex-1 flex-col"
        onSubmit={(event) => void save(event)}
      >
        <DialogBody>
          <SessionFields
            disabled={loading}
            form={form}
            groups={groups}
            setForm={setForm}
          />
        </DialogBody>
        <DialogFooter>
          <Button onClick={onClose} type="button" variant="secondary">
            Cancel
          </Button>
          <Button
            disabled={saving || loading || !sessionFormReady(form)}
            type="submit"
          >
            {saving ? "Scheduling…" : "Schedule session"}
          </Button>
        </DialogFooter>
      </form>
    </DialogShell>
  );
}

function SessionRecordDrawer({
  canOperate,
  onClose,
  onSaved,
  open,
  summary,
}: {
  canOperate: boolean;
  onClose: () => void;
  onSaved: () => void;
  open: boolean;
  summary: ActivitySessionSummary | null;
}) {
  const [record, setRecord] = useState<ActivitySessionRecord | null>(null);
  const [form, setForm] = useState<SessionForm>(emptySessionForm());
  const [action, setAction] = useState<SessionAction | null>(null);
  const [selectedParticipation, setSelectedParticipation] =
    useState<ActivityParticipation | null>(null);
  const [mark, setMark] =
    useState<ActivityParticipationMark>("present");
  const [participationNotes, setParticipationNotes] = useState("");
  const [reason, setReason] = useState("");
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);

  const loadRecord = useCallback(async () => {
    if (!summary) return;
    setLoading(true);
    try {
      const response = await activitiesService.session(summary.id);
      if (!response.success || !response.data) {
        throw new Error(
          responseMessage(response, "Activity session could not be loaded"),
        );
      }
      setRecord(response.data);
      setForm(sessionForm(response.data));
    } catch (loadError) {
      toast.error(
        loadError instanceof Error
          ? loadError.message
          : "Activity session could not be loaded",
      );
    } finally {
      setLoading(false);
    }
  }, [summary]);

  useEffect(() => {
    if (!open) return;
    setAction(null);
    setSelectedParticipation(null);
    setReason("");
    void loadRecord();
  }, [loadRecord, open]);

  const run = async () => {
    if (!record || !action) return;
    setSaving(true);
    try {
      let response;
      if (action === "edit") {
        const payload = toPayload(form);
        response = await activitiesService.updateSession(record, {
          title: payload.title,
          starts_at: payload.starts_at,
          ends_at: payload.ends_at,
          location_note: payload.location_note,
          notes: payload.notes,
        });
      } else if (action === "participation" && selectedParticipation) {
        response = await activitiesService.markParticipation(
          record.id,
          selectedParticipation,
          mark,
          participationNotes.trim() || null,
        );
      } else if (action === "complete") {
        response = await activitiesService.completeSession(record, reason.trim());
      } else if (action === "cancel") {
        response = await activitiesService.cancelSession(record, reason.trim());
      } else {
        return;
      }
      if (!response.success || !response.data) {
        throw new Error(
          responseMessage(response, "Activity session could not be updated"),
        );
      }
      toast.success(sessionActionMessage(action));
      if (action === "participation") {
        setRecord(response.data);
        setAction(null);
        setSelectedParticipation(null);
      } else {
        onSaved();
      }
    } catch (actionError) {
      toast.error(
        actionError instanceof Error
          ? actionError.message
          : "Activity session could not be updated",
      );
    } finally {
      setSaving(false);
    }
  };

  const beginParticipation = (participation: ActivityParticipation) => {
    setSelectedParticipation(participation);
    setMark(participation.mark ?? "present");
    setParticipationNotes(participation.notes ?? "");
    setAction("participation");
  };

  const unmarkedCount = useMemo(
    () => record?.participation.filter((item) => !item.mark).length ?? 0,
    [record],
  );

  return (
    <DialogShell
      onClose={onClose}
      open={open}
      panelClassName="sm:max-w-[800px]"
    >
      <DialogHeader
        onClose={saving ? undefined : onClose}
        title={summary?.title ?? "Activity session"}
      />
      {action && record ? (
        <div className="flex min-h-0 flex-1 flex-col">
          <DialogBody>
            {action === "edit" ? (
              <SessionFields form={form} setForm={setForm} />
            ) : action === "participation" ? (
              <div className="space-y-5">
                <p className="text-sm font-medium text-[var(--text-strong)]">
                  {selectedParticipation?.learner_name}
                </p>
                <Field label="Participation">
                  <Select
                    data-autofocus="true"
                    onChange={(event) =>
                      setMark(event.target.value as ActivityParticipationMark)
                    }
                    value={mark}
                  >
                    {participationMarks.map((value) => (
                      <option key={value} value={value}>
                        {displayValue(value)}
                      </option>
                    ))}
                  </Select>
                </Field>
                <Field label="Notes">
                  <Textarea
                    maxLength={2000}
                    onChange={(event) =>
                      setParticipationNotes(event.target.value)
                    }
                    rows={6}
                    value={participationNotes}
                  />
                </Field>
              </div>
            ) : (
              <div className="space-y-5">
                {action === "complete" && unmarkedCount > 0 ? (
                  <div className="border border-[var(--status-warning-border)] bg-[var(--status-warning-soft)] p-4 text-sm text-[var(--text-body)]">
                    {unmarkedCount} learner{unmarkedCount === 1 ? " is" : "s are"}{" "}
                    still unmarked.
                  </div>
                ) : null}
                <Field
                  label={
                    action === "complete"
                      ? "Completion summary"
                      : "Cancellation reason"
                  }
                >
                  <Textarea
                    data-autofocus="true"
                    maxLength={action === "complete" ? 3000 : 2000}
                    onChange={(event) => setReason(event.target.value)}
                    rows={7}
                    value={reason}
                  />
                </Field>
              </div>
            )}
          </DialogBody>
          <DialogFooter>
            <Button
              onClick={() => setAction(null)}
              type="button"
              variant="secondary"
            >
              Back
            </Button>
            <Button
              disabled={
                saving ||
                (action === "edit"
                  ? !sessionFormReady(form)
                  : action === "complete" || action === "cancel"
                    ? !reason.trim() ||
                      (action === "complete" && unmarkedCount > 0)
                    : false)
              }
              onClick={() => void run()}
              type="button"
              variant={action === "cancel" ? "destructive" : "default"}
            >
              {saving ? "Saving…" : sessionActionButton(action)}
            </Button>
          </DialogFooter>
        </div>
      ) : (
        <div className="flex min-h-0 flex-1 flex-col">
          <DialogBody>
            {loading || !record ? (
              <div className="flex min-h-48 items-center justify-center text-sm text-[var(--text-muted)]">
                {loading ? "Loading session…" : "Session unavailable"}
              </div>
            ) : (
              <div className="space-y-7">
                <div>
                  <div className="flex flex-wrap gap-2">
                    <Badge tone={statusTone(record.status)}>
                      {displayValue(record.status)}
                    </Badge>
                    <Badge tone="neutral">{record.reference}</Badge>
                  </div>
                  <p className="mt-3 text-sm text-[var(--text-muted)]">
                    {record.group_name} · {formatDateTime(record.starts_at)} –{" "}
                    {formatDateTime(record.ends_at)}
                  </p>
                  {record.location_note ? (
                    <p className="mt-2 text-sm text-[var(--text-body)]">
                      {record.location_note}
                    </p>
                  ) : null}
                  {record.notes ? (
                    <p className="mt-3 whitespace-pre-wrap text-sm text-[var(--text-muted)]">
                      {record.notes}
                    </p>
                  ) : null}
                  {record.completion_summary ? (
                    <p className="mt-3 text-sm text-[var(--text-body)]">
                      {record.completion_summary}
                    </p>
                  ) : null}
                  {record.cancellation_reason ? (
                    <p className="mt-3 text-sm text-[var(--status-danger)]">
                      {record.cancellation_reason}
                    </p>
                  ) : null}
                </div>
                <Section label="Participation">
                  {record.participation.length ? (
                    <div className="divide-y divide-[var(--border)] border border-[var(--border)]">
                      {record.participation.map((participation) => (
                        <button
                          className="flex w-full items-center justify-between gap-4 p-4 text-left disabled:cursor-default"
                          disabled={!canOperate || record.status !== "scheduled"}
                          key={participation.membership_id}
                          onClick={() => beginParticipation(participation)}
                          type="button"
                        >
                          <span>
                            <span className="block font-medium text-[var(--text-strong)]">
                              {participation.learner_name}
                            </span>
                            <span className="mt-1 block text-xs text-[var(--text-muted)]">
                              {participation.learner_number}
                              {participation.notes
                                ? ` · ${participation.notes}`
                                : ""}
                            </span>
                          </span>
                          <Badge
                            tone={
                              participation.mark
                                ? statusTone(participation.mark)
                                : "neutral"
                            }
                          >
                            {participation.mark
                              ? displayValue(participation.mark)
                              : "Unmarked"}
                          </Badge>
                        </button>
                      ))}
                    </div>
                  ) : (
                    <div className="flex min-h-32 flex-col items-center justify-center border border-[var(--border)] text-center">
                      <UsersRound className="size-5 text-[var(--text-muted)]" />
                      <p className="mt-2 text-sm text-[var(--text-muted)]">
                        No learners were on this session roster.
                      </p>
                    </div>
                  )}
                </Section>
                <Section label="History">
                  {record.history.length ? (
                    <div className="space-y-3">
                      {record.history.map((event) => (
                        <div
                          className="border-l-2 border-[var(--border-strong)] pl-4"
                          key={event.id}
                        >
                          <p className="text-sm font-medium text-[var(--text-strong)]">
                            {displayValue(
                              event.event_type.replace("activities.session.", ""),
                            )}
                          </p>
                          <p className="mt-1 text-xs text-[var(--text-muted)]">
                            {event.actor_name} · {formatDateTime(event.created_at)}
                          </p>
                        </div>
                      ))}
                    </div>
                  ) : (
                    <p className="text-sm text-[var(--text-muted)]">
                      No lifecycle events recorded.
                    </p>
                  )}
                </Section>
              </div>
            )}
          </DialogBody>
          <DialogFooter>
            {canOperate && record?.status === "scheduled" ? (
              <>
                <Button
                  className="mr-auto"
                  onClick={() => {
                    setReason("");
                    setAction("cancel");
                  }}
                  type="button"
                  variant="ghost"
                >
                  Cancel session
                </Button>
                <Button
                  onClick={() => setAction("edit")}
                  type="button"
                  variant="secondary"
                >
                  Edit
                </Button>
                <Button
                  onClick={() => {
                    setReason("");
                    setAction("complete");
                  }}
                  type="button"
                >
                  Complete session
                </Button>
              </>
            ) : null}
            <Button onClick={onClose} type="button" variant="secondary">
              Close
            </Button>
          </DialogFooter>
        </div>
      )}
    </DialogShell>
  );
}

function SessionFields({
  disabled,
  form,
  groups,
  setForm,
}: {
  disabled?: boolean;
  form: SessionForm;
  groups?: ActivityGroupSummary[];
  setForm: (value: SessionForm) => void;
}) {
  return (
    <div className="space-y-5">
      {groups ? (
        <Field label="Activity group">
          <Select
            data-autofocus="true"
            disabled={disabled}
            onChange={(event) =>
              setForm({ ...form, group_id: event.target.value })
            }
            required
            value={form.group_id}
          >
            <option value="">Select an active group</option>
            {groups.map((group) => (
              <option key={group.id} value={group.id}>
                {group.name} · {group.code}
              </option>
            ))}
          </Select>
        </Field>
      ) : null}
      <Field label="Title">
        <Input
          data-autofocus={groups ? undefined : "true"}
          maxLength={180}
          onChange={(event) => setForm({ ...form, title: event.target.value })}
          required
          value={form.title}
        />
      </Field>
      <div className="grid gap-4 sm:grid-cols-2">
        <Field label="Starts at">
          <Input
            onChange={(event) =>
              setForm({ ...form, starts_at: event.target.value })
            }
            required
            type="datetime-local"
            value={form.starts_at}
          />
        </Field>
        <Field label="Ends at">
          <Input
            min={form.starts_at}
            onChange={(event) =>
              setForm({ ...form, ends_at: event.target.value })
            }
            required
            type="datetime-local"
            value={form.ends_at}
          />
        </Field>
      </div>
      <Field label="Location">
        <Input
          maxLength={500}
          onChange={(event) =>
            setForm({ ...form, location_note: event.target.value || null })
          }
          value={form.location_note ?? ""}
        />
      </Field>
      <Field label="Notes">
        <Textarea
          maxLength={4000}
          onChange={(event) =>
            setForm({ ...form, notes: event.target.value || null })
          }
          rows={7}
          value={form.notes ?? ""}
        />
      </Field>
    </div>
  );
}

function Section({
  children,
  label,
}: {
  children: React.ReactNode;
  label: string;
}) {
  return (
    <section className="border-t border-[var(--border)] pt-5">
      <h4 className="mb-3 text-xs font-semibold uppercase tracking-[0.14em] text-[var(--text-muted)]">
        {label}
      </h4>
      {children}
    </section>
  );
}

function Field({ children, label }: { children: React.ReactNode; label: string }) {
  return (
    <div className="space-y-2">
      <Label>{label}</Label>
      {children}
    </div>
  );
}

function emptySessionForm(): SessionForm {
  const starts = new Date();
  starts.setMinutes(Math.ceil(starts.getMinutes() / 15) * 15, 0, 0);
  const ends = new Date(starts.getTime() + 60 * 60 * 1000);
  return {
    group_id: "",
    title: "",
    starts_at: toLocalInput(starts.toISOString()),
    ends_at: toLocalInput(ends.toISOString()),
    location_note: null,
    notes: null,
  };
}

function sessionForm(record: ActivitySessionRecord): SessionForm {
  return {
    group_id: record.group_id,
    title: record.title,
    starts_at: toLocalInput(record.starts_at),
    ends_at: toLocalInput(record.ends_at),
    location_note: record.location_note,
    notes: record.notes,
  };
}

function toLocalInput(value: string) {
  const date = new Date(value);
  const offset = date.getTimezoneOffset() * 60_000;
  return new Date(date.getTime() - offset).toISOString().slice(0, 16);
}

function toPayload(form: SessionForm): SessionPayload {
  return {
    group_id: form.group_id,
    title: form.title.trim(),
    starts_at: new Date(form.starts_at).toISOString(),
    ends_at: new Date(form.ends_at).toISOString(),
    location_note: form.location_note?.trim() || null,
    notes: form.notes?.trim() || null,
  };
}

function sessionFormReady(form: SessionForm) {
  if (!form.group_id || !form.title.trim() || !form.starts_at || !form.ends_at) {
    return false;
  }
  return new Date(form.ends_at).getTime() > new Date(form.starts_at).getTime();
}

function sessionActionButton(action: SessionAction) {
  return {
    edit: "Save changes",
    participation: "Save participation",
    complete: "Complete session",
    cancel: "Cancel session",
  }[action];
}

function sessionActionMessage(action: SessionAction) {
  return {
    edit: "Activity session updated",
    participation: "Participation saved",
    complete: "Activity session completed",
    cancel: "Activity session cancelled",
  }[action];
}

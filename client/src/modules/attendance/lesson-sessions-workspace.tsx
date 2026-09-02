import { useCallback, useEffect, useState } from "react";
import { Link, useNavigate } from "@tanstack/react-router";
import { CalendarClock, Loader2, RefreshCw } from "lucide-react";
import toast from "react-hot-toast";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Table, TableControlsBar, TableControlsPagination, TableEmpty, TableError,
  TableLoading, TableScroll, TableWrap, TBody, TD, TH, THead, TR,
} from "@/components/ui/data-table";
import { DialogBody, DialogFooter, DialogHeader, DialogShell } from "@/components/ui/dialog";
import { Input, Label, Select, Textarea } from "@/components/ui/input";
import { usePageChrome } from "@/modules/admin/layouts/page-chrome";
import { useAuthStore } from "@/stores/auth-store";

import { attendanceAccessProfile } from "./access";
import { attendanceService, responseMessage } from "./service";
import type {
  AttendanceLessonSession,
  AttendanceLessonSessionsSearch,
  AttendanceReferenceData,
} from "./types";

interface Props {
  search: AttendanceLessonSessionsSearch;
  onSearchChange: (next: AttendanceLessonSessionsSearch, options?: { replace?: boolean }) => void;
}

export function AttendanceLessonSessionsWorkspace({ search, onSearchChange }: Props) {
  const navigate = useNavigate();
  const user = useAuthStore((state) => state.user);
  const access = attendanceAccessProfile(user?.permissions ?? [], user?.record_scopes);
  const [sessions, setSessions] = useState<AttendanceLessonSession[]>([]);
  const [references, setReferences] = useState<AttendanceReferenceData | null>(null);
  const [totalPages, setTotalPages] = useState(1);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [syncOpen, setSyncOpen] = useState(false);
  const [action, setAction] = useState<{ kind: "open" | "cancel"; session: AttendanceLessonSession } | null>(null);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const response = await attendanceService.listLessonSessions({
        page: search.page,
        per_page: 25,
        date_from: search.date_from || undefined,
        date_to: search.date_to || undefined,
        class_group_id: search.class_group_id === "all" ? undefined : search.class_group_id,
        status: search.status === "all" ? undefined : search.status,
      });
      if (!response.success || !response.data) {
        throw new Error(responseMessage(response, "Lesson sessions could not be loaded"));
      }
      setSessions(response.data.sessions);
      setTotalPages(response.pagination?.total_pages ?? 1);
    } catch (loadError) {
      setError(loadError instanceof Error ? loadError.message : "Lesson sessions could not be loaded");
    } finally {
      setLoading(false);
    }
  }, [search]);

  useEffect(() => { void load(); }, [load]);
  useEffect(() => {
    void attendanceService.references().then((response) => {
      if (response.success) setReferences(response.data ?? null);
    });
  }, []);

  usePageChrome(
    "Lesson sessions",
    access.canManage ? <Button onClick={() => setSyncOpen(true)}><RefreshCw className="size-4" />Sync timetable</Button> : null,
  );

  const updateSearch = (patch: Partial<AttendanceLessonSessionsSearch>) => {
    onSearchChange({ ...search, ...patch }, { replace: true });
  };
  const filtered = Boolean(
    search.date_from || search.date_to || search.class_group_id !== "all" || search.status !== "all",
  );

  return <div className="space-y-6">
    <p className="text-sm text-[var(--text-muted)]">Open registers from the published timetable for lessons assigned to you.</p>
    <TableControlsBar>
      <Input aria-label="From date" className="sm:w-40" max={search.date_to || undefined} onChange={(event) => updateSearch({ date_from: event.target.value, page: 1 })} type="date" value={search.date_from} />
      <Input aria-label="To date" className="sm:w-40" min={search.date_from || undefined} onChange={(event) => updateSearch({ date_to: event.target.value, page: 1 })} type="date" value={search.date_to} />
      <Select aria-label="Class filter" className="sm:w-52" onChange={(event) => updateSearch({ class_group_id: event.target.value, page: 1 })} value={search.class_group_id}>
        <option value="all">All classes</option>
        {references?.classes.map((classGroup) => <option key={classGroup.id} value={classGroup.id}>{classGroup.name} · {classGroup.code}</option>)}
      </Select>
      <Select aria-label="Status filter" className="sm:w-44" onChange={(event) => updateSearch({ status: event.target.value as AttendanceLessonSessionsSearch["status"], page: 1 })} value={search.status}>
        <option value="all">All statuses</option><option value="scheduled">Scheduled</option><option value="open">Open</option><option value="completed">Completed</option><option value="cancelled">Cancelled</option>
      </Select>
      {!loading && sessions.length > 0 ? <TableControlsPagination onNext={() => updateSearch({ page: Math.min(totalPages, search.page + 1) })} onPrevious={() => updateSearch({ page: Math.max(1, search.page - 1) })} page={search.page} totalPages={totalPages} /> : null}
    </TableControlsBar>

    <TableWrap>
      {loading ? <TableLoading columns={6} label="Loading lesson sessions…" /> : error ? <TableError description={error} onRetry={() => void load()} /> : sessions.length === 0 ? (
        <TableEmpty description={filtered ? "Change the current filters." : access.canManage ? "Sync a date range from the published timetable." : "No assigned lesson sessions are available."} icon={<CalendarClock />} title={filtered ? "No sessions match these filters" : "No lesson sessions yet"} />
      ) : <TableScroll><Table className="min-w-[920px]"><THead><tr><TH>Date</TH><TH>Class</TH><TH>Subject</TH><TH>Teacher</TH><TH>Period</TH><TH>Status</TH><TH className="w-40">Action</TH></tr></THead><TBody>
        {sessions.map((session) => <TR key={session.id}>
          <TD><p className="font-medium text-[var(--text-strong)]">{formatDate(session.session_date)}</p><p className="mt-1 text-xs text-[var(--text-muted)]">{displayValue(session.day_key)}</p></TD>
          <TD>{session.class_group_name}</TD><TD>{session.subject_name}</TD><TD className="text-[var(--text-muted)]">{session.teacher_name}</TD>
          <TD className="font-tabular text-[var(--text-muted)]">{displayPeriod(session.period_key)}</TD>
          <TD><Badge tone={session.status === "completed" ? "success" : session.status === "open" ? "warning" : session.status === "cancelled" ? "neutral" : "info"}>{displayValue(session.status)}</Badge></TD>
          <TD>{session.register_id ? <Link className="text-sm font-semibold text-[var(--brand-strong)] hover:underline" params={{ registerId: session.register_id }} to="/modules/attendance/registers/$registerId">Open register</Link> : session.status === "scheduled" && access.canCreate ? <div className="flex gap-2"><Button onClick={() => setAction({ kind: "open", session })} size="sm">Open</Button>{access.canManage ? <Button onClick={() => setAction({ kind: "cancel", session })} size="sm" variant="ghost">Cancel</Button> : null}</div> : "—"}</TD>
        </TR>)}
      </TBody></Table></TableScroll>}
    </TableWrap>

    <SyncSessionsDrawer onClose={() => setSyncOpen(false)} onSynced={() => { setSyncOpen(false); void load(); }} open={syncOpen} references={references} />
    <SessionActionDrawer action={action} onClose={() => setAction(null)} onCompleted={(session) => {
      setAction(null);
      if (session.register_id) void navigate({ to: "/modules/attendance/registers/$registerId", params: { registerId: session.register_id } });
      else void load();
    }} />
  </div>;
}

function SyncSessionsDrawer({ onClose, onSynced, open, references }: { onClose: () => void; onSynced: () => void; open: boolean; references: AttendanceReferenceData | null }) {
  const [dateFrom, setDateFrom] = useState("");
  const [dateTo, setDateTo] = useState("");
  const [saving, setSaving] = useState(false);
  useEffect(() => {
    if (!open) return;
    const start = clampDate(today(), references?.term.starts_on, references?.term.ends_on);
    setDateFrom(start);
    setDateTo(addDays(start, 6, references?.term.ends_on));
  }, [open, references]);

  const submit = async (event: React.FormEvent) => {
    event.preventDefault();
    if (saving) return;
    setSaving(true);
    try {
      const response = await attendanceService.syncLessonSessions(dateFrom, dateTo);
      if (!response.success || !response.data) throw new Error(responseMessage(response, "Lesson sessions could not be synced"));
      toast.success(`${response.data.created_count} lesson session${response.data.created_count === 1 ? "" : "s"} added`);
      onSynced();
    } catch (syncError) {
      toast.error(syncError instanceof Error ? syncError.message : "Lesson sessions could not be synced");
    } finally { setSaving(false); }
  };

  return <DialogShell onClose={saving ? () => undefined : onClose} open={open}><DialogHeader onClose={saving ? undefined : onClose} title="Sync lesson sessions" />
    <form onSubmit={submit}><DialogBody className="space-y-5">
      <p className="text-sm leading-6 text-[var(--text-muted)]">Create operational sessions from the current published timetable. Existing sessions are left unchanged.</p>
      {!references ? <p className="border border-[var(--border)] bg-[var(--surface-muted)] p-4 text-sm text-[var(--text-muted)]">An active academic term is required.</p> : <>
        <div><Label htmlFor="session-sync-from">From</Label><Input className="mt-1.5" data-autofocus="true" id="session-sync-from" max={dateTo || references.term.ends_on} min={references.term.starts_on} onChange={(event) => { const value = event.target.value; setDateFrom(value); if (dateTo < value) setDateTo(value); }} required type="date" value={dateFrom} /></div>
        <div><Label htmlFor="session-sync-to">To</Label><Input className="mt-1.5" id="session-sync-to" max={minDate(references.term.ends_on, addDays(dateFrom, 30))} min={dateFrom || references.term.starts_on} onChange={(event) => setDateTo(event.target.value)} required type="date" value={dateTo} /></div>
        <p className="text-xs text-[var(--text-muted)]">Maximum 31 days per sync.</p>
      </>}
    </DialogBody><DialogFooter><Button disabled={saving} onClick={onClose} type="button" variant="secondary">Cancel</Button><Button disabled={saving || !references || !dateFrom || !dateTo} type="submit">{saving ? <><Loader2 className="size-4 animate-spin" />Syncing…</> : "Sync sessions"}</Button></DialogFooter></form>
  </DialogShell>;
}

function SessionActionDrawer({ action, onClose, onCompleted }: { action: { kind: "open" | "cancel"; session: AttendanceLessonSession } | null; onClose: () => void; onCompleted: (session: AttendanceLessonSession) => void }) {
  const [reason, setReason] = useState("");
  const [saving, setSaving] = useState(false);
  useEffect(() => { if (action) setReason(""); }, [action]);
  if (!action) return null;
  const submit = async (event: React.FormEvent) => {
    event.preventDefault();
    if (saving) return;
    setSaving(true);
    try {
      const response = action.kind === "open"
        ? await attendanceService.openLessonSession(action.session.id, action.session.version)
        : await attendanceService.cancelLessonSession(action.session.id, action.session.version, reason.trim());
      if (!response.success || !response.data) throw new Error(responseMessage(response, "Lesson session could not be updated"));
      toast.success(action.kind === "open" ? "Lesson register opened" : "Lesson session cancelled");
      onCompleted(response.data);
    } catch (actionError) {
      toast.error(actionError instanceof Error ? actionError.message : "Lesson session could not be updated");
    } finally { setSaving(false); }
  };
  return <DialogShell onClose={saving ? () => undefined : onClose} open><DialogHeader onClose={saving ? undefined : onClose} title={action.kind === "open" ? "Open lesson register?" : "Cancel lesson session?"} />
    <form onSubmit={submit}><DialogBody className="space-y-5"><div className="border border-[var(--border)] bg-[var(--surface-muted)] p-4"><p className="font-medium text-[var(--text-strong)]">{action.session.class_group_name} · {action.session.subject_name}</p><p className="mt-1 text-sm text-[var(--text-muted)]">{formatDate(action.session.session_date)} · {displayPeriod(action.session.period_key)}</p></div>
      {action.kind === "open" ? <p className="text-sm leading-6 text-[var(--text-muted)]">The current class roster will be frozen into a draft register.</p> : <div><Label htmlFor="session-cancel-reason">Reason</Label><Textarea className="mt-1.5 min-h-28" data-autofocus="true" id="session-cancel-reason" maxLength={1000} onChange={(event) => setReason(event.target.value)} required value={reason} /></div>}
    </DialogBody><DialogFooter><Button disabled={saving} onClick={onClose} type="button" variant="secondary">Back</Button><Button disabled={saving || (action.kind === "cancel" && !reason.trim())} type="submit" variant={action.kind === "cancel" ? "destructive" : "primary"}>{saving ? <><Loader2 className="size-4 animate-spin" />Saving…</> : action.kind === "open" ? "Open register" : "Cancel session"}</Button></DialogFooter></form>
  </DialogShell>;
}

function today() { return new Date().toISOString().slice(0, 10); }
function clampDate(value: string, minimum?: string, maximum?: string) { return minimum && value < minimum ? minimum : maximum && value > maximum ? maximum : value; }
function addDays(value: string, count: number, maximum?: string) { if (!value) return ""; const date = new Date(`${value}T00:00:00Z`); date.setUTCDate(date.getUTCDate() + count); return minDate(date.toISOString().slice(0, 10), maximum); }
function minDate(left: string, right?: string) { return right && right < left ? right : left; }
function displayValue(value: string) { return value.replace(/[_-]/g, " ").replace(/^./, (letter) => letter.toUpperCase()); }
function displayPeriod(value: string) { const match = value.match(/(\d+)$/); return match ? `Period ${match[1]}` : displayValue(value); }
function formatDate(value: string) { return new Intl.DateTimeFormat(undefined, { day: "numeric", month: "short", year: "numeric", timeZone: "UTC" }).format(new Date(`${value}T00:00:00Z`)); }

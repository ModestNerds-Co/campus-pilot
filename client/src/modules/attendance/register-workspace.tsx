import { useCallback, useEffect, useMemo, useState } from "react";
import { Link, useNavigate } from "@tanstack/react-router";
import { ArrowLeft, CheckCircle2, Loader2, RotateCcw, Save, Trash2, UserCheck } from "lucide-react";
import toast from "react-hot-toast";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { ConfirmDrawer } from "@/components/ui/confirm-drawer";
import { Table, TableScroll, TableWrap, TBody, TD, TH, THead, TR } from "@/components/ui/data-table";
import { DialogBody, DialogFooter, DialogHeader, DialogShell } from "@/components/ui/dialog";
import { Input, Label, Select, Textarea } from "@/components/ui/input";
import { usePageChrome } from "@/modules/admin/layouts/page-chrome";
import { useAuthStore } from "@/stores/auth-store";

import { attendanceService, responseMessage } from "./service";
import type { AttendanceMark, AttendanceMarkInput, AttendanceMarkStatus, AttendanceRegister } from "./types";

type RegisterAction = "submit" | "reopen" | "delete" | null;

export function AttendanceRegisterWorkspace({ registerId }: { registerId: string }) {
  const navigate = useNavigate();
  const permissions = useAuthStore((state) => state.user?.permissions ?? []);
  const canEdit = permissions.includes("*") || permissions.includes("attendance:edit");
  const canSubmit = permissions.includes("*") || permissions.includes("attendance:submit");
  const canManage = permissions.includes("*") || permissions.includes("attendance:manage");
  const canDelete = permissions.includes("*") || permissions.includes("attendance:delete");
  const [register, setRegister] = useState<AttendanceRegister | null>(null);
  const [marks, setMarks] = useState<AttendanceMark[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [notFound, setNotFound] = useState(false);
  const [saving, setSaving] = useState(false);
  const [action, setAction] = useState<RegisterAction>(null);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    setNotFound(false);
    try {
      const response = await attendanceService.readRegister(registerId);
      if (!response.success || !response.data) {
        if (response.issues?.some((issue) => (typeof issue === "string" ? issue : issue.detail)?.toLowerCase().includes("not found"))) setNotFound(true);
        else throw new Error(responseMessage(response, "Attendance register could not be loaded"));
        return;
      }
      setRegister(response.data);
      setMarks(response.data.marks);
    } catch (loadError) {
      setError(loadError instanceof Error ? loadError.message : "Attendance register could not be loaded");
    } finally {
      setLoading(false);
    }
  }, [registerId]);

  useEffect(() => { void load(); }, [load]);

  const dirty = useMemo(() => register ? markFingerprint(marks) !== markFingerprint(register.marks) : false, [marks, register]);
  const counts = useMemo(() => countMarks(marks), [marks]);
  const editable = register?.status === "draft" && canEdit;
  const allMarked = marks.length > 0 && counts.unmarked === 0;

  const save = async () => {
    if (!register || !dirty || saving) return;
    setSaving(true);
    try {
      const response = await attendanceService.updateMarks(register.id, register.version, marks.map(markPayload));
      if (!response.success || !response.data) throw new Error(responseMessage(response, "Attendance marks could not be saved"));
      setRegister(response.data);
      setMarks(response.data.marks);
      toast.success("Attendance saved");
    } catch (saveError) {
      toast.error(saveError instanceof Error ? saveError.message : "Attendance marks could not be saved");
    } finally {
      setSaving(false);
    }
  };

  const markAllPresent = () => setMarks((current) => current.map((mark) => ({ ...mark, mark: "present", minutes_late: null, note: null })));

  usePageChrome("Attendance register", register ? <div className="flex flex-wrap items-center gap-2">
    {register.status === "draft" && editable ? <Button disabled={!dirty || saving} onClick={() => void save()} variant="secondary"><Save className="size-4" />{saving ? "Saving…" : "Save"}</Button> : null}
    {register.status === "draft" && canSubmit ? <Button disabled={dirty || !allMarked || saving} onClick={() => setAction("submit")}><CheckCircle2 className="size-4" />Submit</Button> : null}
    {register.status === "submitted" && canManage ? <Button onClick={() => setAction("reopen")} variant="secondary"><RotateCcw className="size-4" />Reopen</Button> : null}
    {register.status === "draft" && canDelete ? <Button aria-label="Delete register" onClick={() => setAction("delete")} size="icon" variant="ghost"><Trash2 className="size-4" /></Button> : null}
  </div> : null);

  if (loading) return <div aria-label="Loading attendance register" className="flex min-h-64 items-center justify-center border border-[var(--border)] bg-[var(--surface)]" role="status"><Loader2 className="size-6 animate-spin text-[var(--brand-strong)]" /></div>;
  if (notFound) return <Unavailable description="This attendance register does not exist or is no longer available." title="Register not found" />;
  if (error || !register) return <Unavailable description={error || "Attendance register could not be loaded."} onRetry={() => void load()} title="Register unavailable" />;

  return (
    <div className="space-y-6">
      <Link className="inline-flex items-center gap-2 text-sm font-medium text-[var(--text-muted)] hover:text-[var(--text-strong)]" to="/modules/attendance/registers"><ArrowLeft className="size-4" />All registers</Link>

      <section className="border border-[var(--border)] bg-[var(--surface)]">
        <div className="flex flex-col gap-4 border-b border-[var(--border)] p-5 sm:flex-row sm:items-start sm:justify-between sm:p-6">
          <div><p className="text-xs font-semibold uppercase tracking-[0.14em] text-[var(--brand-strong)]">{displayValue(register.period)}</p><h1 className="mt-2 text-2xl font-semibold tracking-[-0.03em] text-[var(--text-strong)]">{register.class_group_name}</h1><p className="mt-2 text-sm text-[var(--text-muted)]">{formatDate(register.attendance_date)} · {register.academic_term_name}</p></div>
          <Badge tone={register.status === "submitted" ? "success" : "warning"}>{displayValue(register.status)}</Badge>
        </div>
        <div className="grid grid-cols-2 md:grid-cols-5">
          <Fact label="Learners" value={String(marks.length)} />
          <Fact label="Present" value={String(counts.present)} />
          <Fact label="Absent" value={String(counts.absent)} />
          <Fact label="Late" value={String(counts.late)} />
          <Fact label="Unmarked" value={String(counts.unmarked)} />
        </div>
      </section>

      {register.reopen_reason ? <section className="border border-[var(--tone-warn-bd)] bg-[var(--badge-warning-bg)] p-4 text-sm text-[var(--badge-warning-text)]"><span className="font-semibold">Reopened:</span> {register.reopen_reason}</section> : null}
      {dirty ? <section className="border border-[var(--brand-100)] bg-[var(--badge-info-bg)] p-4 text-sm text-[var(--badge-info-text)]">Save the changed marks before submitting the register.</section> : null}

      <div className="flex flex-wrap items-center justify-between gap-3">
        <div><h2 className="text-lg font-semibold text-[var(--text-strong)]">Class roster</h2><p className="mt-1 text-sm text-[var(--text-muted)]">{register.status === "submitted" ? "Submitted marks are read-only." : "Mark every learner before submission."}</p></div>
        {editable ? <Button disabled={saving} onClick={markAllPresent} variant="secondary"><UserCheck className="size-4" />Mark all present</Button> : null}
      </div>

      <TableWrap>
        <TableScroll><Table className="min-w-[920px]"><THead><tr><TH>Learner</TH><TH className="w-44">Mark</TH><TH className="w-36">Minutes late</TH><TH>Note</TH></tr></THead><TBody>
          {marks.map((mark) => <TR key={mark.id}>
            <TD><Link className="font-medium text-[var(--text-strong)] hover:text-[var(--brand-strong)] hover:underline" params={{ learnerId: mark.learner_id }} to="/modules/attendance/learners/$learnerId">{mark.learner_name}</Link><p className="mt-1 font-tabular text-xs text-[var(--text-muted)]">{mark.learner_number}</p></TD>
            <TD><Select aria-label={`Mark for ${mark.learner_name}`} disabled={!editable || saving} onChange={(event) => updateMark(setMarks, mark.id, { mark: event.target.value as AttendanceMarkStatus })} value={mark.mark}><option value="unmarked">Unmarked</option><option value="present">Present</option><option value="absent">Absent</option><option value="late">Late</option><option value="excused">Excused</option></Select></TD>
            <TD><Input aria-label={`Minutes late for ${mark.learner_name}`} disabled={!editable || saving || mark.mark !== "late"} max={1440} min={0} onChange={(event) => updateMark(setMarks, mark.id, { minutes_late: event.target.value === "" ? null : Number(event.target.value) })} type="number" value={mark.minutes_late ?? ""} /></TD>
            <TD><Input aria-label={`Note for ${mark.learner_name}`} disabled={!editable || saving || mark.mark === "unmarked"} maxLength={1000} onChange={(event) => updateMark(setMarks, mark.id, { note: event.target.value || null })} placeholder={mark.mark === "unmarked" ? "Choose a mark first" : "Optional"} value={mark.note ?? ""} /></TD>
          </TR>)}
        </TBody></Table></TableScroll>
      </TableWrap>

      {register.status === "draft" && counts.unmarked > 0 ? <p className="text-sm text-[var(--text-muted)]">{counts.unmarked} {counts.unmarked === 1 ? "learner is" : "learners are"} still unmarked.</p> : null}

      <RegisterActionDrawer action={action === "submit" || action === "reopen" ? action : null} onClose={() => setAction(null)} onCompleted={(updated) => { setRegister(updated); setMarks(updated.marks); setAction(null); }} register={register} />
      <ConfirmDrawer confirmLabel="Delete register" description={`Delete the draft attendance register for ${register.class_group_name} on ${formatDate(register.attendance_date)}?`} isPending={saving} onClose={() => setAction(null)} onConfirm={() => void (async () => {
        setSaving(true);
        try {
          const response = await attendanceService.deleteRegister(register.id, register.version);
          if (!response.success) throw new Error(responseMessage(response, "Attendance register could not be deleted"));
          toast.success("Attendance register deleted");
          void navigate({ to: "/modules/attendance/registers" });
        } catch (deleteError) { toast.error(deleteError instanceof Error ? deleteError.message : "Attendance register could not be deleted"); setSaving(false); }
      })()} open={action === "delete"} title="Delete attendance register?" />
    </div>
  );
}

function RegisterActionDrawer({ action, onClose, onCompleted, register }: { action: "submit" | "reopen" | null; onClose: () => void; onCompleted: (register: AttendanceRegister) => void; register: AttendanceRegister }) {
  const [reason, setReason] = useState("");
  const [pending, setPending] = useState(false);
  useEffect(() => { if (action) setReason(""); }, [action]);
  if (!action) return null;

  const run = async () => {
    if (action === "reopen" && !reason.trim()) { toast.error("Enter a reason for reopening the register"); return; }
    setPending(true);
    try {
      const response = action === "submit"
        ? await attendanceService.submitRegister(register.id, register.version)
        : await attendanceService.reopenRegister(register.id, register.version, reason.trim());
      if (!response.success || !response.data) throw new Error(responseMessage(response, `Attendance register could not be ${action === "submit" ? "submitted" : "reopened"}`));
      toast.success(action === "submit" ? "Attendance register submitted" : "Attendance register reopened");
      onCompleted(response.data);
    } catch (actionError) {
      toast.error(actionError instanceof Error ? actionError.message : "Attendance register could not be updated");
    } finally {
      setPending(false);
    }
  };

  return <DialogShell onClose={pending ? () => undefined : onClose} open><DialogHeader onClose={pending ? undefined : onClose} title={action === "submit" ? "Submit attendance register?" : "Reopen attendance register?"} /><DialogBody className="space-y-5">
    <div className="flex gap-4"><span className="flex size-10 shrink-0 items-center justify-center rounded-[9px] bg-[var(--badge-info-bg)] text-[var(--badge-info-text)]"><CheckCircle2 className="size-5" /></span><p className="text-sm leading-6 text-[var(--text-muted)]">{action === "submit" ? "Submitting locks the current marks. A manager must reopen the register before they can be changed." : "Reopening returns this register to draft so its marks can be corrected."}</p></div>
    {action === "reopen" ? <div><Label htmlFor="attendance-reopen-reason">Reason</Label><Textarea className="mt-1.5 min-h-28" data-autofocus="true" id="attendance-reopen-reason" maxLength={1000} onChange={(event) => setReason(event.target.value)} required value={reason} /></div> : null}
  </DialogBody><DialogFooter><Button disabled={pending} onClick={onClose} type="button" variant="secondary">Cancel</Button><Button disabled={pending || (action === "reopen" && !reason.trim())} onClick={() => void run()} type="button">{pending ? <Loader2 className="size-4 animate-spin" /> : null}{pending ? "Updating…" : action === "submit" ? "Submit register" : "Reopen register"}</Button></DialogFooter></DialogShell>;
}

function updateMark(setMarks: React.Dispatch<React.SetStateAction<AttendanceMark[]>>, id: string, patch: Partial<AttendanceMark>) {
  setMarks((current) => current.map((mark) => {
    if (mark.id !== id) return mark;
    const updated = { ...mark, ...patch };
    if (patch.mark && patch.mark !== "late") updated.minutes_late = null;
    if (patch.mark === "unmarked") updated.note = null;
    return updated;
  }));
}

function markPayload(mark: AttendanceMark): AttendanceMarkInput { return { learner_id: mark.learner_id, mark: mark.mark, minutes_late: mark.mark === "late" ? mark.minutes_late : null, note: mark.mark === "unmarked" ? null : mark.note?.trim() || null }; }
function markFingerprint(marks: AttendanceMark[]) { return JSON.stringify(marks.map(markPayload).sort((left, right) => left.learner_id.localeCompare(right.learner_id))); }
function countMarks(marks: AttendanceMark[]) { return marks.reduce((counts, mark) => ({ ...counts, [mark.mark]: counts[mark.mark] + 1 }), { unmarked: 0, present: 0, absent: 0, late: 0, excused: 0 } as Record<AttendanceMarkStatus, number>); }
function Fact({ label, value }: { label: string; value: string }) { return <div className="border-b border-r border-[var(--border)] p-4 last:border-r-0 md:border-b-0"><p className="text-xs font-medium uppercase tracking-[0.12em] text-[var(--text-subtle)]">{label}</p><p className="mt-2 font-tabular text-lg font-semibold text-[var(--text-strong)]">{value}</p></div>; }
function Unavailable({ description, onRetry, title }: { description: string; onRetry?: () => void; title: string }) { return <div className="border border-[var(--border)] bg-[var(--surface)] p-8 text-center"><h1 className="text-lg font-semibold text-[var(--text-strong)]">{title}</h1><p className="mx-auto mt-2 max-w-lg text-sm text-[var(--text-muted)]">{description}</p>{onRetry ? <Button className="mt-5" onClick={onRetry} variant="secondary">Retry</Button> : <Link className="mt-5 inline-flex text-sm font-semibold text-[var(--brand-strong)] hover:underline" to="/modules/attendance/registers">Back to registers</Link>}</div>; }
function displayValue(value: string) { return value.replace(/_/g, " ").replace(/^./, (letter) => letter.toUpperCase()); }
function formatDate(value: string) { return new Intl.DateTimeFormat(undefined, { day: "numeric", month: "short", year: "numeric", timeZone: "UTC" }).format(new Date(`${value}T00:00:00Z`)); }

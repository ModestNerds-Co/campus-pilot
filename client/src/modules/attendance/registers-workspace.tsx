import { useCallback, useEffect, useState } from "react";
import { Link, useNavigate } from "@tanstack/react-router";
import { CalendarCheck2, Loader2, Plus } from "lucide-react";
import toast from "react-hot-toast";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Table, TableControlsBar, TableControlsPagination, TableEmpty, TableError, TableLoading,
  TableScroll, TableWrap, TBody, TD, TH, THead, TR,
} from "@/components/ui/data-table";
import { DialogBody, DialogFooter, DialogHeader, DialogShell } from "@/components/ui/dialog";
import { Input, Label, Select } from "@/components/ui/input";
import { usePageChrome } from "@/modules/admin/layouts/page-chrome";
import { useAuthStore } from "@/stores/auth-store";

import { attendanceService, responseMessage } from "./service";
import type {
  AttendancePeriod, AttendanceReferenceData, AttendanceRegisterStatus,
  AttendanceRegisterSummary,
} from "./types";

export function AttendanceRegistersWorkspace() {
  const navigate = useNavigate();
  const permissions = useAuthStore((state) => state.user?.permissions ?? []);
  const canCreate = permissions.includes("*") || permissions.includes("attendance:create");
  const [registers, setRegisters] = useState<AttendanceRegisterSummary[]>([]);
  const [references, setReferences] = useState<AttendanceReferenceData | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [page, setPage] = useState(1);
  const [totalPages, setTotalPages] = useState(1);
  const [dateFrom, setDateFrom] = useState("");
  const [dateTo, setDateTo] = useState("");
  const [classId, setClassId] = useState("all");
  const [period, setPeriod] = useState("all");
  const [status, setStatus] = useState("all");
  const [createOpen, setCreateOpen] = useState(false);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const response = await attendanceService.listRegisters({
        page,
        per_page: 25,
        date_from: dateFrom || undefined,
        date_to: dateTo || undefined,
        class_group_id: classId === "all" ? undefined : classId,
        period: period === "all" ? undefined : period as AttendancePeriod,
        status: status === "all" ? undefined : status as AttendanceRegisterStatus,
      });
      if (!response.success || !response.data) throw new Error(responseMessage(response, "Attendance registers could not be loaded"));
      setRegisters(response.data.registers);
      setTotalPages(response.pagination?.total_pages ?? 1);
    } catch (loadError) {
      setError(loadError instanceof Error ? loadError.message : "Attendance registers could not be loaded");
    } finally {
      setLoading(false);
    }
  }, [classId, dateFrom, dateTo, page, period, status]);

  useEffect(() => { void load(); }, [load]);
  useEffect(() => {
    void attendanceService.references().then((response) => {
      if (response.success) setReferences(response.data ?? null);
    });
  }, []);

  usePageChrome("Attendance registers", canCreate ? <Button onClick={() => setCreateOpen(true)}><Plus className="size-4" />New register</Button> : null);
  const filtered = Boolean(dateFrom || dateTo || classId !== "all" || period !== "all" || status !== "all");

  return (
    <div className="space-y-6">
      <p className="text-sm text-[var(--text-muted)]">Record daily learner attendance by class.</p>
      <TableControlsBar>
        <Input aria-label="From date" className="sm:w-40" max={dateTo || undefined} onChange={(event) => { setPage(1); setDateFrom(event.target.value); }} type="date" value={dateFrom} />
        <Input aria-label="To date" className="sm:w-40" min={dateFrom || undefined} onChange={(event) => { setPage(1); setDateTo(event.target.value); }} type="date" value={dateTo} />
        <Select aria-label="Class filter" className="sm:w-52" onChange={(event) => { setPage(1); setClassId(event.target.value); }} value={classId}>
          <option value="all">All classes</option>
          {references?.classes.map((classGroup) => <option key={classGroup.id} value={classGroup.id}>{classGroup.name} · {classGroup.code}</option>)}
        </Select>
        <Select aria-label="Period filter" className="sm:w-40" onChange={(event) => { setPage(1); setPeriod(event.target.value); }} value={period}>
          <option value="all">All periods</option><option value="full_day">Full day</option><option value="morning">Morning</option><option value="afternoon">Afternoon</option>
        </Select>
        <Select aria-label="Status filter" className="sm:w-40" onChange={(event) => { setPage(1); setStatus(event.target.value); }} value={status}>
          <option value="all">All statuses</option><option value="draft">Draft</option><option value="submitted">Submitted</option>
        </Select>
        {!loading && registers.length > 0 ? <TableControlsPagination onNext={() => setPage((value) => Math.min(totalPages, value + 1))} onPrevious={() => setPage((value) => Math.max(1, value - 1))} page={page} totalPages={totalPages} /> : null}
      </TableControlsBar>

      <TableWrap>
        {loading ? <TableLoading columns={6} label="Loading attendance registers…" /> : error ? <TableError description={error} onRetry={() => void load()} /> : registers.length === 0 ? (
          <TableEmpty description={filtered ? "Change the current filters." : canCreate ? "Create the first register for a class." : "No attendance registers are available."} icon={<CalendarCheck2 />} title={filtered ? "No registers match these filters" : "No attendance registers yet"} />
        ) : <TableScroll><Table><THead><tr><TH>Date</TH><TH>Class</TH><TH>Period</TH><TH>Status</TH><TH>Marked</TH><TH>Exceptions</TH></tr></THead><TBody>
          {registers.map((register) => <TR key={register.id}>
            <TD><Link className="font-medium text-[var(--text-strong)] hover:text-[var(--brand-strong)] hover:underline" params={{ registerId: register.id }} to="/modules/attendance/registers/$registerId">{formatDate(register.attendance_date)}</Link><p className="mt-1 text-xs text-[var(--text-muted)]">{register.academic_term_name}</p></TD>
            <TD><span className="font-medium text-[var(--text-strong)]">{register.class_group_name}</span></TD>
            <TD className="text-[var(--text-muted)]">{displayValue(register.period)}</TD>
            <TD><Badge tone={register.status === "submitted" ? "success" : "warning"}>{displayValue(register.status)}</Badge></TD>
            <TD className="font-tabular text-[var(--text-muted)]">{register.learner_count - register.unmarked_count} / {register.learner_count}</TD>
            <TD className="font-tabular text-[var(--text-muted)]">{register.absent_count + register.late_count + register.excused_count}</TD>
          </TR>)}
        </TBody></Table></TableScroll>}
      </TableWrap>

      <CreateRegisterDrawer onClose={() => setCreateOpen(false)} onCreated={(registerId) => { setCreateOpen(false); void navigate({ to: "/modules/attendance/registers/$registerId", params: { registerId } }); }} open={createOpen} references={references} />
    </div>
  );
}

function CreateRegisterDrawer({ onClose, onCreated, open, references }: { onClose: () => void; onCreated: (registerId: string) => void; open: boolean; references: AttendanceReferenceData | null }) {
  const [classId, setClassId] = useState("");
  const [attendanceDate, setAttendanceDate] = useState(today());
  const [period, setPeriod] = useState<AttendancePeriod>("full_day");
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    if (!open) return;
    setClassId(references?.classes[0]?.id ?? "");
    setAttendanceDate(clampDate(today(), references?.term.starts_on, references?.term.ends_on));
    setPeriod("full_day");
  }, [open, references]);

  const submit = async (event: React.FormEvent) => {
    event.preventDefault();
    if (!references || saving) return;
    setSaving(true);
    try {
      const response = await attendanceService.createRegister({
        academic_term_id: references.term.id,
        class_group_id: classId,
        attendance_date: attendanceDate,
        period,
        idempotency_key: crypto.randomUUID(),
      });
      if (!response.success || !response.data) throw new Error(responseMessage(response, "Attendance register could not be created"));
      toast.success("Attendance register created");
      onCreated(response.data.id);
    } catch (saveError) {
      toast.error(saveError instanceof Error ? saveError.message : "Attendance register could not be created");
    } finally {
      setSaving(false);
    }
  };

  return <DialogShell onClose={onClose} open={open}><DialogHeader onClose={saving ? undefined : onClose} title="New attendance register" />
    {!references ? <DialogBody><p className="text-sm leading-6 text-[var(--text-muted)]">An active academic year, term, and class are required before a register can be created.</p></DialogBody> : <form onSubmit={submit}><DialogBody className="space-y-5">
      <div><Label>Academic term</Label><p className="mt-2 text-sm font-medium text-[var(--text-strong)]">{references.term.name}</p><p className="mt-1 text-xs text-[var(--text-muted)]">{references.term.academic_year_name} · {formatDate(references.term.starts_on)} to {formatDate(references.term.ends_on)}</p></div>
      <div><Label>Class</Label><Select className="mt-1.5" data-autofocus="true" onChange={(event) => setClassId(event.target.value)} required value={classId}><option value="">Choose a class</option>{references.classes.map((classGroup) => <option key={classGroup.id} value={classGroup.id}>{classGroup.name} · {classGroup.code}</option>)}</Select>{references.classes.length === 0 ? <p className="mt-2 text-xs text-[var(--tone-danger)]">No active classes are available.</p> : null}</div>
      <div><Label>Date</Label><Input className="mt-1.5" max={references.term.ends_on} min={references.term.starts_on} onChange={(event) => setAttendanceDate(event.target.value)} required type="date" value={attendanceDate} /></div>
      <div><Label>Period</Label><Select className="mt-1.5" onChange={(event) => setPeriod(event.target.value as AttendancePeriod)} value={period}><option value="full_day">Full day</option><option value="morning">Morning</option><option value="afternoon">Afternoon</option></Select></div>
    </DialogBody><DialogFooter><Button disabled={saving} onClick={onClose} type="button" variant="secondary">Cancel</Button><Button disabled={saving || !classId || references.classes.length === 0} type="submit">{saving ? <><Loader2 className="size-4 animate-spin" />Creating…</> : "Create register"}</Button></DialogFooter></form>}
  </DialogShell>;
}

function today() { return new Date().toISOString().slice(0, 10); }
function clampDate(value: string, minimum?: string, maximum?: string) { return minimum && value < minimum ? minimum : maximum && value > maximum ? maximum : value; }
function displayValue(value: string) { return value.replace(/_/g, " ").replace(/^./, (letter) => letter.toUpperCase()); }
function formatDate(value: string) { return new Intl.DateTimeFormat(undefined, { day: "numeric", month: "short", year: "numeric", timeZone: "UTC" }).format(new Date(`${value}T00:00:00Z`)); }

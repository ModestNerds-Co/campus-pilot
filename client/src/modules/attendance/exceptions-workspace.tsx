import { useCallback, useEffect, useState } from "react";
import { Link } from "@tanstack/react-router";
import { AlertTriangle } from "lucide-react";

import { Badge } from "@/components/ui/badge";
import {
  Table, TableControlsBar, TableControlsPagination, TableEmpty, TableError,
  TableLoading, TableScroll, TableWrap, TBody, TD, TH, THead, TR,
} from "@/components/ui/data-table";
import { Input, Select } from "@/components/ui/input";
import { usePageChrome } from "@/modules/admin/layouts/page-chrome";

import { attendanceService, responseMessage } from "./service";
import type {
  AttendanceException,
  AttendanceExceptionsSearch,
  AttendanceReferenceData,
} from "./types";

interface Props {
  search: AttendanceExceptionsSearch;
  onSearchChange: (next: AttendanceExceptionsSearch, options?: { replace?: boolean }) => void;
}

export function AttendanceExceptionsWorkspace({ search, onSearchChange }: Props) {
  const [exceptions, setExceptions] = useState<AttendanceException[]>([]);
  const [references, setReferences] = useState<AttendanceReferenceData | null>(null);
  const [totalPages, setTotalPages] = useState(1);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const response = await attendanceService.listExceptions({
        page: search.page,
        per_page: 25,
        date_from: search.date_from || undefined,
        date_to: search.date_to || undefined,
        class_group_id: search.class_group_id === "all" ? undefined : search.class_group_id,
        status: search.status === "all" ? undefined : search.status,
        mark: search.mark === "all" ? undefined : search.mark,
      });
      if (!response.success || !response.data) throw new Error(responseMessage(response, "Attendance exceptions could not be loaded"));
      setExceptions(response.data.exceptions);
      setTotalPages(response.pagination?.total_pages ?? 1);
    } catch (loadError) {
      setError(loadError instanceof Error ? loadError.message : "Attendance exceptions could not be loaded");
    } finally { setLoading(false); }
  }, [search]);

  useEffect(() => { void load(); }, [load]);
  useEffect(() => { void attendanceService.references().then((response) => { if (response.success) setReferences(response.data ?? null); }); }, []);
  usePageChrome("Absence follow-up");

  const updateSearch = (patch: Partial<AttendanceExceptionsSearch>) => onSearchChange({ ...search, ...patch }, { replace: true });
  const filtered = Boolean(search.date_from || search.date_to || search.class_group_id !== "all" || search.status !== "all" || search.mark !== "all");

  return <div className="space-y-6">
    <p className="text-sm text-[var(--text-muted)]">Review absences, late arrivals, and excused marks from submitted registers.</p>
    <TableControlsBar>
      <Input aria-label="From date" className="sm:w-40" max={search.date_to || undefined} onChange={(event) => updateSearch({ date_from: event.target.value, page: 1 })} type="date" value={search.date_from} />
      <Input aria-label="To date" className="sm:w-40" min={search.date_from || undefined} onChange={(event) => updateSearch({ date_to: event.target.value, page: 1 })} type="date" value={search.date_to} />
      <Select aria-label="Class filter" className="sm:w-52" onChange={(event) => updateSearch({ class_group_id: event.target.value, page: 1 })} value={search.class_group_id}><option value="all">All classes</option>{references?.classes.map((classGroup) => <option key={classGroup.id} value={classGroup.id}>{classGroup.name} · {classGroup.code}</option>)}</Select>
      <Select aria-label="Mark filter" className="sm:w-40" onChange={(event) => updateSearch({ mark: event.target.value as AttendanceExceptionsSearch["mark"], page: 1 })} value={search.mark}><option value="all">All marks</option><option value="absent">Absent</option><option value="late">Late</option><option value="excused">Excused</option></Select>
      <Select aria-label="Status filter" className="sm:w-44" onChange={(event) => updateSearch({ status: event.target.value as AttendanceExceptionsSearch["status"], page: 1 })} value={search.status}><option value="all">All statuses</option><option value="open">Open</option><option value="acknowledged">Acknowledged</option><option value="resolved">Resolved</option></Select>
      {!loading && exceptions.length > 0 ? <TableControlsPagination onNext={() => updateSearch({ page: Math.min(totalPages, search.page + 1) })} onPrevious={() => updateSearch({ page: Math.max(1, search.page - 1) })} page={search.page} totalPages={totalPages} /> : null}
    </TableControlsBar>
    <TableWrap>{loading ? <TableLoading columns={6} label="Loading Attendance exceptions…" /> : error ? <TableError description={error} onRetry={() => void load()} /> : exceptions.length === 0 ? <TableEmpty description={filtered ? "Change the current filters." : "Exceptions appear here when a register with an absent, late, or excused mark is submitted."} icon={<AlertTriangle />} title={filtered ? "No exceptions match these filters" : "No Attendance exceptions"} /> : <TableScroll><Table className="min-w-[900px]"><THead><tr><TH>Learner</TH><TH>Date</TH><TH>Class</TH><TH>Mark</TH><TH>Status</TH><TH>Follow-up</TH></tr></THead><TBody>{exceptions.map((exception) => <TR key={exception.id}>
      <TD><Link className="font-medium text-[var(--text-strong)] hover:text-[var(--brand-strong)] hover:underline" params={{ exceptionId: exception.id }} to="/modules/attendance/exceptions/$exceptionId">{exception.learner_name}</Link><p className="mt-1 font-tabular text-xs text-[var(--text-muted)]">{exception.learner_number}</p></TD>
      <TD><p>{formatDate(exception.attendance_date)}</p><p className="mt-1 text-xs text-[var(--text-muted)]">{displayPeriod(exception.period)}</p></TD><TD>{exception.class_group_name}</TD>
      <TD><Badge tone={exception.mark === "absent" ? "danger" : exception.mark === "late" ? "warning" : "neutral"}>{displayValue(exception.mark)}</Badge>{exception.mark === "late" && exception.minutes_late !== null ? <p className="mt-1 text-xs text-[var(--text-muted)]">{exception.minutes_late} min</p> : null}</TD>
      <TD><Badge tone={exception.status === "resolved" ? "success" : exception.status === "acknowledged" ? "info" : "warning"}>{displayValue(exception.status)}</Badge></TD>
      <TD className="max-w-60 text-sm text-[var(--text-muted)]">{exception.resolution || exception.acknowledgement_note || exception.attendance_note || "Not recorded"}</TD>
    </TR>)}</TBody></Table></TableScroll>}</TableWrap>
  </div>;
}

function displayValue(value: string) { return value.replace(/[_-]/g, " ").replace(/^./, (letter) => letter.toUpperCase()); }
function displayPeriod(value: string) { if (!value.startsWith("lesson:")) return displayValue(value); const match = value.match(/(\d+)$/); return match ? `Lesson · Period ${match[1]}` : `Lesson · ${displayValue(value.slice(7))}`; }
function formatDate(value: string) { return new Intl.DateTimeFormat(undefined, { day: "numeric", month: "short", year: "numeric", timeZone: "UTC" }).format(new Date(`${value}T00:00:00Z`)); }

/** Read-only learner Attendance history over accepted submitted registers. */

import { useCallback, useEffect, useState } from "react";
import { Link } from "@tanstack/react-router";
import { ArrowLeft, CalendarDays, Loader2 } from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Table, TableScroll, TableWrap, TBody, TD, TH, THead, TR } from "@/components/ui/data-table";
import { Input, Label } from "@/components/ui/input";
import { usePageChrome } from "@/modules/admin/layouts/page-chrome";

import { attendanceService, responseMessage } from "./service";
import type { LearnerAttendanceHistory, PaginationMeta } from "./types";

export function LearnerAttendanceHistoryWorkspace({ learnerId }: { learnerId: string }) {
  const [history, setHistory] = useState<LearnerAttendanceHistory | null>(null);
  const [pagination, setPagination] = useState<PaginationMeta | null>(null);
  const [page, setPage] = useState(1);
  const [dateFrom, setDateFrom] = useState("");
  const [dateTo, setDateTo] = useState("");
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  usePageChrome("Learner attendance");

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const response = await attendanceService.learnerHistory(learnerId, {
        page,
        per_page: 25,
        date_from: dateFrom || undefined,
        date_to: dateTo || undefined,
      });
      if (!response.success || !response.data) {
        throw new Error(responseMessage(response, "Learner attendance could not be loaded"));
      }
      setHistory(response.data);
      setPagination(response.pagination);
    } catch (loadError) {
      setError(loadError instanceof Error ? loadError.message : "Learner attendance could not be loaded");
    } finally {
      setLoading(false);
    }
  }, [dateFrom, dateTo, learnerId, page]);

  useEffect(() => { void load(); }, [load]);

  return <div className="space-y-6">
    <Link className="inline-flex items-center gap-2 text-sm font-medium text-[var(--text-muted)] hover:text-[var(--text-strong)]" to="/modules/attendance/registers"><ArrowLeft className="size-4" />Attendance registers</Link>

    {loading && !history ? <div aria-label="Loading learner attendance" className="flex min-h-64 items-center justify-center border border-[var(--border)] bg-[var(--surface)]" role="status"><Loader2 className="size-6 animate-spin text-[var(--brand-strong)]" /></div> : error || !history ? <Unavailable description={error || "Learner attendance could not be loaded."} onRetry={() => void load()} /> : <>
      <section className="border border-[var(--border)] bg-[var(--surface)]">
        <div className="border-b border-[var(--border)] p-5 sm:p-6"><p className="text-xs font-semibold uppercase tracking-[0.14em] text-[var(--brand-strong)]">Attendance history</p><h1 className="mt-2 text-2xl font-semibold tracking-[-0.03em] text-[var(--text-strong)]">{history.learner_name}</h1><p className="mt-2 font-tabular text-sm text-[var(--text-muted)]">{history.learner_number}</p></div>
        <div className="grid grid-cols-2 md:grid-cols-4"><Fact label="Present" value={history.present_count} /><Fact label="Absent" value={history.absent_count} /><Fact label="Late" value={history.late_count} /><Fact label="Excused" value={history.excused_count} /></div>
      </section>

      <section className="grid gap-4 border border-[var(--border)] bg-[var(--surface)] p-4 sm:grid-cols-[minmax(0,1fr)_minmax(0,1fr)_auto] sm:items-end">
        <div><Label htmlFor="attendance-history-from">From</Label><Input className="mt-1.5" id="attendance-history-from" onChange={(event) => { setDateFrom(event.target.value); setPage(1); }} type="date" value={dateFrom} /></div>
        <div><Label htmlFor="attendance-history-to">To</Label><Input className="mt-1.5" id="attendance-history-to" onChange={(event) => { setDateTo(event.target.value); setPage(1); }} type="date" value={dateTo} /></div>
        <Button disabled={!dateFrom && !dateTo} onClick={() => { setDateFrom(""); setDateTo(""); setPage(1); }} variant="secondary">Clear dates</Button>
      </section>

      {history.entries.length === 0 ? <div className="border border-[var(--border)] bg-[var(--surface)] p-10 text-center"><CalendarDays className="mx-auto size-7 text-[var(--text-subtle)]" /><h2 className="mt-3 font-semibold text-[var(--text-strong)]">No submitted attendance</h2><p className="mt-2 text-sm text-[var(--text-muted)]">No accepted marks match the selected dates.</p></div> : <TableWrap><TableScroll><Table className="min-w-[820px]"><THead><tr><TH>Date</TH><TH>Class</TH><TH>Period</TH><TH>Mark</TH><TH>Details</TH></tr></THead><TBody>{history.entries.map((entry) => <TR key={`${entry.register_id}-${entry.attendance_date}`}><TD><Link className="font-medium text-[var(--text-strong)] hover:text-[var(--brand-strong)] hover:underline" params={{ registerId: entry.register_id }} to="/modules/attendance/registers/$registerId">{formatDate(entry.attendance_date)}</Link></TD><TD>{entry.class_group_name}</TD><TD>{displayValue(entry.period)}</TD><TD><Badge tone={entry.mark === "present" ? "success" : entry.mark === "late" ? "warning" : entry.mark === "absent" ? "danger" : "neutral"}>{displayValue(entry.mark)}</Badge></TD><TD className="text-[var(--text-muted)]">{entry.mark === "late" && entry.minutes_late !== null ? `${entry.minutes_late} min late` : entry.note || "—"}</TD></TR>)}</TBody></Table></TableScroll></TableWrap>}

      {pagination && pagination.total_pages > 1 ? <div className="flex items-center justify-between gap-4"><p className="text-sm text-[var(--text-muted)]">Page {pagination.current_page} of {pagination.total_pages}</p><div className="flex gap-2"><Button disabled={!pagination.has_prev || loading} onClick={() => setPage((current) => Math.max(1, current - 1))} variant="secondary">Previous</Button><Button disabled={!pagination.has_next || loading} onClick={() => setPage((current) => current + 1)} variant="secondary">Next</Button></div></div> : null}
    </>}
  </div>;
}

function Fact({ label, value }: { label: string; value: number }) { return <div className="border-b border-r border-[var(--border)] p-4 last:border-r-0 md:border-b-0"><p className="text-xs font-medium uppercase tracking-[0.12em] text-[var(--text-subtle)]">{label}</p><p className="mt-2 font-tabular text-lg font-semibold text-[var(--text-strong)]">{value}</p></div>; }
function Unavailable({ description, onRetry }: { description: string; onRetry: () => void }) { return <div className="border border-[var(--border)] bg-[var(--surface)] p-8 text-center"><h1 className="text-lg font-semibold text-[var(--text-strong)]">Attendance history unavailable</h1><p className="mx-auto mt-2 max-w-lg text-sm text-[var(--text-muted)]">{description}</p><Button className="mt-5" onClick={onRetry} variant="secondary">Retry</Button></div>; }
function displayValue(value: string) { return value.replace(/_/g, " ").replace(/^./, (letter) => letter.toUpperCase()); }
function formatDate(value: string) { return new Intl.DateTimeFormat(undefined, { day: "numeric", month: "short", year: "numeric", timeZone: "UTC" }).format(new Date(`${value}T00:00:00Z`)); }


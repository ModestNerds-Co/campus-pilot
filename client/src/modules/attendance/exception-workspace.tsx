import { useCallback, useEffect, useState } from "react";
import { Link } from "@tanstack/react-router";
import { ArrowLeft, Loader2 } from "lucide-react";
import toast from "react-hot-toast";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { DialogBody, DialogFooter, DialogHeader, DialogShell } from "@/components/ui/dialog";
import { Label, Textarea } from "@/components/ui/input";
import { usePageChrome } from "@/modules/admin/layouts/page-chrome";

import { attendanceService, responseMessage } from "./service";
import type { AttendanceException } from "./types";

type Action = "acknowledge" | "resolve" | "reopen";

export function AttendanceExceptionWorkspace({ exceptionId }: { exceptionId: string }) {
  const [exception, setException] = useState<AttendanceException | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [action, setAction] = useState<Action | null>(null);

  const load = useCallback(async () => {
    setLoading(true); setError(null);
    try {
      const response = await attendanceService.readException(exceptionId);
      if (!response.success || !response.data) throw new Error(responseMessage(response, "Attendance exception could not be loaded"));
      setException(response.data);
    } catch (loadError) { setError(loadError instanceof Error ? loadError.message : "Attendance exception could not be loaded"); }
    finally { setLoading(false); }
  }, [exceptionId]);

  useEffect(() => { void load(); }, [load]);
  usePageChrome("Attendance exception", exception ? <div className="flex flex-wrap gap-2">{exception.status === "open" ? <Button onClick={() => setAction("acknowledge")} variant="secondary">Acknowledge</Button> : null}{exception.status !== "resolved" ? <Button onClick={() => setAction("resolve")}>Resolve</Button> : <Button onClick={() => setAction("reopen")} variant="secondary">Reopen</Button>}</div> : null);

  if (loading) return <div aria-label="Loading Attendance exception" className="flex min-h-64 items-center justify-center border border-[var(--border)] bg-[var(--surface)]" role="status"><Loader2 className="size-6 animate-spin text-[var(--brand-strong)]" /></div>;
  if (error || !exception) return <div className="border border-[var(--border)] bg-[var(--surface)] p-8 text-center"><h1 className="text-lg font-semibold text-[var(--text-strong)]">Attendance exception unavailable</h1><p className="mx-auto mt-2 max-w-lg text-sm text-[var(--text-muted)]">{error || "The Attendance exception could not be loaded."}</p><Button className="mt-5" onClick={() => void load()} variant="secondary">Retry</Button></div>;

  return <div className="space-y-6">
    <Link className="inline-flex items-center gap-2 text-sm font-medium text-[var(--text-muted)] hover:text-[var(--text-strong)]" search={{ page: 1, date_from: "", date_to: "", class_group_id: "all", status: "all", mark: "all" }} to="/modules/attendance/exceptions"><ArrowLeft className="size-4" />Absence follow-up</Link>
    <section className="border border-[var(--border)] bg-[var(--surface)]">
      <div className="flex flex-col gap-4 border-b border-[var(--border)] p-5 sm:flex-row sm:items-start sm:justify-between sm:p-6"><div><p className="text-xs font-semibold uppercase tracking-[0.14em] text-[var(--brand-strong)]">{displayValue(exception.mark)}</p><h1 className="mt-2 text-2xl font-semibold tracking-[-0.03em] text-[var(--text-strong)]">{exception.learner_name}</h1><p className="mt-2 font-tabular text-sm text-[var(--text-muted)]">{exception.learner_number}</p></div><Badge tone={exception.status === "resolved" ? "success" : exception.status === "acknowledged" ? "info" : "warning"}>{displayValue(exception.status)}</Badge></div>
      <div className="grid border-b border-[var(--border)] sm:grid-cols-2 lg:grid-cols-4"><Fact label="Date" value={formatDate(exception.attendance_date)} /><Fact label="Class" value={exception.class_group_name} /><Fact label="Period" value={displayPeriod(exception.period)} /><Fact label="Submitted" value={formatDateTime(exception.source_submitted_at)} /></div>
      <div className="grid gap-0 lg:grid-cols-2"><div className="border-b border-[var(--border)] p-5 lg:border-b-0 lg:border-r sm:p-6"><h2 className="text-sm font-semibold text-[var(--text-strong)]">Attendance evidence</h2><dl className="mt-4 space-y-4"><Detail label="Mark" value={displayValue(exception.mark)} /><Detail label="Minutes late" value={exception.minutes_late === null ? "Not applicable" : `${exception.minutes_late} minutes`} /><Detail label="Register note" value={exception.attendance_note || "Not recorded"} /><Detail label="Register version" value={String(exception.source_register_version)} /></dl><Link className="mt-5 inline-flex text-sm font-semibold text-[var(--brand-strong)] hover:underline" params={{ registerId: exception.register_id }} to="/modules/attendance/registers/$registerId">Open submitted register</Link></div>
        <div className="p-5 sm:p-6"><h2 className="text-sm font-semibold text-[var(--text-strong)]">Follow-up</h2><dl className="mt-4 space-y-4"><Detail label="Acknowledged" value={exception.acknowledged_at ? formatDateTime(exception.acknowledged_at) : "Not yet"} /><Detail label="Acknowledgement" value={exception.acknowledgement_note || "Not recorded"} /><Detail label="Resolved" value={exception.resolved_at ? formatDateTime(exception.resolved_at) : "Not yet"} /><Detail label="Resolution" value={exception.resolution || "Not recorded"} />{exception.reopened_at ? <Detail label="Reopened" value={`${formatDateTime(exception.reopened_at)} · ${exception.reopen_reason || "Reason not recorded"}`} /> : null}</dl></div>
      </div>
    </section>
    <ExceptionActionDrawer action={action} exception={exception} onClose={() => setAction(null)} onCompleted={(updated) => { setAction(null); setException(updated); }} />
  </div>;
}

function ExceptionActionDrawer({ action, exception, onClose, onCompleted }: { action: Action | null; exception: AttendanceException; onClose: () => void; onCompleted: (updated: AttendanceException) => void }) {
  const [text, setText] = useState(""); const [saving, setSaving] = useState(false);
  useEffect(() => { if (action) setText(""); }, [action]);
  if (!action) return null;
  const labels = action === "acknowledge" ? { title: "Acknowledge exception", field: "Follow-up note", submit: "Acknowledge" } : action === "resolve" ? { title: "Resolve exception", field: "Resolution", submit: "Resolve" } : { title: "Reopen exception", field: "Reason", submit: "Reopen" };
  const submit = async (event: React.FormEvent) => { event.preventDefault(); if (saving || !text.trim()) return; setSaving(true); try { const response = action === "acknowledge" ? await attendanceService.acknowledgeException(exception.id, exception.version, text.trim()) : action === "resolve" ? await attendanceService.resolveException(exception.id, exception.version, text.trim()) : await attendanceService.reopenException(exception.id, exception.version, text.trim()); if (!response.success || !response.data) throw new Error(responseMessage(response, "Attendance exception could not be updated")); toast.success(`Attendance exception ${action === "acknowledge" ? "acknowledged" : action === "resolve" ? "resolved" : "reopened"}`); onCompleted(response.data); } catch (actionError) { toast.error(actionError instanceof Error ? actionError.message : "Attendance exception could not be updated"); } finally { setSaving(false); } };
  return <DialogShell onClose={saving ? () => undefined : onClose} open><DialogHeader onClose={saving ? undefined : onClose} title={labels.title} /><form onSubmit={submit}><DialogBody><Label htmlFor="attendance-exception-action">{labels.field}</Label><Textarea className="mt-1.5 min-h-32" data-autofocus="true" id="attendance-exception-action" maxLength={action === "resolve" ? 2000 : 1000} onChange={(event) => setText(event.target.value)} required value={text} /></DialogBody><DialogFooter><Button disabled={saving} onClick={onClose} type="button" variant="secondary">Cancel</Button><Button disabled={saving || !text.trim()} type="submit">{saving ? <><Loader2 className="size-4 animate-spin" />Saving…</> : labels.submit}</Button></DialogFooter></form></DialogShell>;
}

function Fact({ label, value }: { label: string; value: string }) { return <div className="border-b border-[var(--border)] p-4 last:border-b-0 sm:border-b-0 sm:border-r sm:last:border-r-0"><p className="text-[11px] font-semibold uppercase tracking-[0.12em] text-[var(--text-subtle)]">{label}</p><p className="mt-2 text-sm font-medium text-[var(--text-strong)]">{value}</p></div>; }
function Detail({ label, value }: { label: string; value: string }) { return <div><dt className="text-xs font-medium text-[var(--text-subtle)]">{label}</dt><dd className="mt-1 text-sm leading-6 text-[var(--text-body)]">{value}</dd></div>; }
function displayValue(value: string) { return value.replace(/[_-]/g, " ").replace(/^./, (letter) => letter.toUpperCase()); }
function displayPeriod(value: string) { if (!value.startsWith("lesson:")) return displayValue(value); const match = value.match(/(\d+)$/); return match ? `Lesson · Period ${match[1]}` : `Lesson · ${displayValue(value.slice(7))}`; }
function formatDate(value: string) { return new Intl.DateTimeFormat(undefined, { day: "numeric", month: "short", year: "numeric", timeZone: "UTC" }).format(new Date(`${value}T00:00:00Z`)); }
function formatDateTime(value: string) { return new Intl.DateTimeFormat(undefined, { dateStyle: "medium", timeStyle: "short" }).format(new Date(value)); }

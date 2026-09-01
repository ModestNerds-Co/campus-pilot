import { useCallback, useEffect, useMemo, useState } from "react";
import { Link, useNavigate } from "@tanstack/react-router";
import { ArrowLeft, CheckCircle2, Loader2, RotateCcw, Save, Send, Trash2 } from "lucide-react";
import toast from "react-hot-toast";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { ConfirmDrawer } from "@/components/ui/confirm-drawer";
import { Table, TableScroll, TableWrap, TBody, TD, TH, THead, TR } from "@/components/ui/data-table";
import { DialogBody, DialogFooter, DialogHeader, DialogShell } from "@/components/ui/dialog";
import { Input, Label, Select, Textarea } from "@/components/ui/input";
import { usePageChrome } from "@/modules/admin/layouts/page-chrome";
import { useAuthStore } from "@/stores/auth-store";

import { gradebookService, responseMessage } from "./service";
import type { GradebookMark, GradebookMarkInput, GradebookMarkStatus, GradebookSheet } from "./types";

type MarkSheetAction = "submit" | "publish" | "reopen" | "delete" | null;

export function MarkSheetWorkspace({ markSheetId }: { markSheetId: string }) {
  const navigate = useNavigate();
  const permissions = useAuthStore((state) => state.user?.permissions ?? []);
  const canEdit = permissions.includes("*") || permissions.includes("academics:edit");
  const canManage = permissions.includes("*") || permissions.includes("academics:manage");
  const canDelete = permissions.includes("*") || permissions.includes("academics:delete");
  const [sheet, setSheet] = useState<GradebookSheet | null>(null);
  const [marks, setMarks] = useState<GradebookMark[]>([]);
  const [rawScores, setRawScores] = useState<Record<string, string>>({});
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [notFound, setNotFound] = useState(false);
  const [saving, setSaving] = useState(false);
  const [action, setAction] = useState<MarkSheetAction>(null);

  const applySheet = useCallback((next: GradebookSheet) => {
    setSheet(next);
    setMarks(next.marks);
    setRawScores(Object.fromEntries(next.marks.map((mark) => [mark.id, mark.marks_awarded_hundredths === null ? "" : formatHundredths(mark.marks_awarded_hundredths)])));
  }, []);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    setNotFound(false);
    try {
      const response = await gradebookService.readMarkSheet(markSheetId);
      if (!response.success || !response.data) {
        if (response.issues?.some((issue) => (typeof issue === "string" ? issue : issue.detail)?.toLowerCase().includes("not found"))) setNotFound(true);
        else throw new Error(responseMessage(response, "Mark sheet could not be loaded"));
        return;
      }
      applySheet(response.data);
    } catch (loadError) {
      setError(loadError instanceof Error ? loadError.message : "Mark sheet could not be loaded");
    } finally {
      setLoading(false);
    }
  }, [applySheet, markSheetId]);

  useEffect(() => { void load(); }, [load]);

  const payload = useMemo(() => marks.map((mark) => markPayload(mark, rawScores[mark.id] ?? "")), [marks, rawScores]);
  const invalidScores = useMemo(() => payload.some((mark) => mark.mark_status === "scored" && mark.marks_awarded_hundredths === null), [payload]);
  const dirty = useMemo(() => sheet ? markFingerprint(payload) !== markFingerprint(sheet.marks.map((mark) => markPayload(mark, formatHundredths(mark.marks_awarded_hundredths)))) : false, [payload, sheet]);
  const counts = useMemo(() => countMarks(marks), [marks]);
  const editable = sheet?.status === "draft" && canEdit;
  const allMarked = marks.length > 0 && counts.unmarked === 0 && !invalidScores;

  const save = async () => {
    if (!sheet || !dirty || invalidScores || saving) return;
    const overMaximum = payload.some((mark) => mark.marks_awarded_hundredths !== null && mark.marks_awarded_hundredths > sheet.maximum_marks * 100);
    if (overMaximum) { toast.error(`Marks cannot exceed ${sheet.maximum_marks}`); return; }
    setSaving(true);
    try {
      const response = await gradebookService.updateMarks(sheet.id, sheet.version, payload);
      if (!response.success || !response.data) throw new Error(responseMessage(response, "Marks could not be saved"));
      applySheet(response.data);
      toast.success("Marks saved");
    } catch (saveError) {
      toast.error(saveError instanceof Error ? saveError.message : "Marks could not be saved");
    } finally {
      setSaving(false);
    }
  };

  usePageChrome("Mark sheet", sheet ? <div className="flex flex-wrap items-center gap-2">
    {sheet.status === "draft" && editable ? <Button disabled={!dirty || invalidScores || saving} onClick={() => void save()} variant="secondary"><Save className="size-4" />{saving ? "Saving…" : "Save"}</Button> : null}
    {sheet.status === "draft" && canEdit ? <Button disabled={dirty || !allMarked || saving} onClick={() => setAction("submit")}><CheckCircle2 className="size-4" />Submit</Button> : null}
    {sheet.status === "submitted" && canManage ? <Button onClick={() => setAction("publish")}><Send className="size-4" />Publish</Button> : null}
    {(sheet.status === "submitted" || sheet.status === "published") && canManage ? <Button onClick={() => setAction("reopen")} variant="secondary"><RotateCcw className="size-4" />Reopen</Button> : null}
    {sheet.status === "draft" && canDelete ? <Button aria-label="Delete mark sheet" onClick={() => setAction("delete")} size="icon" variant="ghost"><Trash2 className="size-4" /></Button> : null}
  </div> : null);

  if (loading) return <div aria-label="Loading mark sheet" className="flex min-h-64 items-center justify-center border border-[var(--border)] bg-[var(--surface)]" role="status"><Loader2 className="size-6 animate-spin text-[var(--brand-strong)]" /></div>;
  if (notFound) return <Unavailable description="This mark sheet does not exist or is no longer available." title="Mark sheet not found" />;
  if (error || !sheet) return <Unavailable description={error || "Mark sheet could not be loaded."} onRetry={() => void load()} title="Mark sheet unavailable" />;

  return <div className="space-y-6">
    <Link className="inline-flex items-center gap-2 text-sm font-medium text-[var(--text-muted)] hover:text-[var(--text-strong)]" to="/modules/academics/gradebook"><ArrowLeft className="size-4" />Gradebook</Link>

    <section className="border border-[var(--border)] bg-[var(--surface)]">
      <div className="flex flex-col gap-4 border-b border-[var(--border)] p-5 sm:flex-row sm:items-start sm:justify-between sm:p-6">
        <div><p className="text-xs font-semibold uppercase tracking-[0.14em] text-[var(--brand-strong)]">{sheet.subject_name} · {sheet.class_group_name}</p><h1 className="mt-2 text-2xl font-semibold tracking-[-0.03em] text-[var(--text-strong)]">{sheet.assessment_component_name}</h1><p className="mt-2 text-sm text-[var(--text-muted)]">{sheet.assessment_cycle_name} · {sheet.academic_term_name} · {sheet.maximum_marks} marks · {formatBasisPoints(sheet.weight_basis_points)} weight</p><p className="mt-1 text-xs text-[var(--text-subtle)]">Teacher: {sheet.teacher_name} · Roster: {formatDate(sheet.roster_on)}</p></div>
        <Badge tone={statusTone(sheet.status)}>{displayValue(sheet.status)}</Badge>
      </div>
      <div className="grid grid-cols-2 md:grid-cols-5"><Fact label="Learners" value={String(marks.length)} /><Fact label="Scored" value={String(counts.scored)} /><Fact label="Absent" value={String(counts.absent)} /><Fact label="Unmarked" value={String(counts.unmarked)} /><Fact label="Average" value={formatPercentage(sheet.average_percentage_basis_points)} /></div>
    </section>

    {sheet.reopen_reason ? <section className="border border-[var(--tone-warn-bd)] bg-[var(--badge-warning-bg)] p-4 text-sm text-[var(--badge-warning-text)]"><span className="font-semibold">Reopened:</span> {sheet.reopen_reason}</section> : null}
    {dirty ? <section className="border border-[var(--brand-100)] bg-[var(--badge-info-bg)] p-4 text-sm text-[var(--badge-info-text)]">Save the changed marks before submitting this mark sheet.</section> : null}

    <div><h2 className="text-lg font-semibold text-[var(--text-strong)]">Learner marks</h2><p className="mt-1 text-sm text-[var(--text-muted)]">{sheet.status === "draft" ? "Every learner must be scored, absent, or exempt before submission." : sheet.status === "submitted" ? "Submitted marks await publication." : "Published marks are read-only."}</p></div>

    <TableWrap><TableScroll><Table className="min-w-[1020px]"><THead><tr><TH>Learner</TH><TH className="w-44">Status</TH><TH className="w-36">Mark / {sheet.maximum_marks}</TH><TH className="w-28">Percent</TH><TH>Note</TH></tr></THead><TBody>
      {marks.map((mark) => <TR key={mark.id}>
        <TD><p className="font-medium text-[var(--text-strong)]">{mark.learner_name}</p><p className="mt-1 font-tabular text-xs text-[var(--text-muted)]">{mark.learner_number}</p></TD>
        <TD><Select aria-label={`Status for ${mark.learner_name}`} disabled={!editable || saving} onChange={(event) => updateMarkStatus(setMarks, setRawScores, mark.id, event.target.value as GradebookMarkStatus)} value={mark.mark_status}><option value="unmarked">Unmarked</option><option value="scored">Scored</option><option value="absent">Absent</option><option value="exempt">Exempt</option></Select></TD>
        <TD><Input aria-label={`Mark for ${mark.learner_name}`} disabled={!editable || saving || mark.mark_status !== "scored"} inputMode="decimal" max={sheet.maximum_marks} min={0} onChange={(event) => setRawScores((current) => ({ ...current, [mark.id]: normalizeScoreInput(event.target.value) }))} placeholder={mark.mark_status === "scored" ? "0.00" : "—"} step="0.01" type="number" value={rawScores[mark.id] ?? ""} /></TD>
        <TD className="font-tabular text-[var(--text-muted)]">{mark.mark_status === "scored" ? percentageFromRaw(rawScores[mark.id] ?? "", sheet.maximum_marks) : "—"}</TD>
        <TD><Input aria-label={`Note for ${mark.learner_name}`} disabled={!editable || saving || mark.mark_status === "unmarked"} maxLength={1000} onChange={(event) => updateMark(setMarks, mark.id, { note: event.target.value || null })} placeholder={mark.mark_status === "unmarked" ? "Choose a status first" : "Optional"} value={mark.note ?? ""} /></TD>
      </TR>)}
    </TBody></Table></TableScroll></TableWrap>

    {sheet.status === "draft" && counts.unmarked > 0 ? <p className="text-sm text-[var(--text-muted)]">{counts.unmarked} {counts.unmarked === 1 ? "learner is" : "learners are"} still unmarked.</p> : null}

    <MarkSheetActionDrawer action={action === "submit" || action === "publish" || action === "reopen" ? action : null} onClose={() => setAction(null)} onCompleted={(updated) => { applySheet(updated); setAction(null); }} sheet={sheet} />
    <ConfirmDrawer confirmLabel="Delete mark sheet" description={`Delete the draft mark sheet for ${sheet.assessment_component_name}?`} isPending={saving} onClose={() => setAction(null)} onConfirm={() => void (async () => {
      setSaving(true);
      try {
        const response = await gradebookService.deleteMarkSheet(sheet.id, sheet.version);
        if (!response.success) throw new Error(responseMessage(response, "Mark sheet could not be deleted"));
        toast.success("Mark sheet deleted");
        void navigate({ to: "/modules/academics/gradebook" });
      } catch (deleteError) { toast.error(deleteError instanceof Error ? deleteError.message : "Mark sheet could not be deleted"); setSaving(false); }
    })()} open={action === "delete"} title="Delete mark sheet?" />
  </div>;
}

function MarkSheetActionDrawer({ action, onClose, onCompleted, sheet }: { action: "submit" | "publish" | "reopen" | null; onClose: () => void; onCompleted: (sheet: GradebookSheet) => void; sheet: GradebookSheet }) {
  const [reason, setReason] = useState("");
  const [pending, setPending] = useState(false);
  useEffect(() => { if (action) setReason(""); }, [action]);
  if (!action) return null;

  const run = async () => {
    if (action === "reopen" && !reason.trim()) { toast.error("Enter a reason for reopening the mark sheet"); return; }
    setPending(true);
    try {
      const response = action === "submit" ? await gradebookService.submitMarkSheet(sheet.id, sheet.version)
        : action === "publish" ? await gradebookService.publishMarkSheet(sheet.id, sheet.version)
          : await gradebookService.reopenMarkSheet(sheet.id, sheet.version, reason.trim());
      if (!response.success || !response.data) throw new Error(responseMessage(response, "Mark sheet could not be updated"));
      toast.success(action === "submit" ? "Mark sheet submitted" : action === "publish" ? "Marks published" : "Mark sheet reopened");
      onCompleted(response.data);
    } catch (actionError) {
      toast.error(actionError instanceof Error ? actionError.message : "Mark sheet could not be updated");
    } finally {
      setPending(false);
    }
  };

  const title = action === "submit" ? "Submit mark sheet?" : action === "publish" ? "Publish learner marks?" : "Reopen mark sheet?";
  const description = action === "submit" ? "Submitting locks mark capture and sends this sheet for publication." : action === "publish" ? "Publishing makes these results final for downstream reporting. Reopening will require a recorded reason." : "Reopening returns this mark sheet to draft so its marks can be corrected.";
  return <DialogShell onClose={pending ? () => undefined : onClose} open><DialogHeader onClose={pending ? undefined : onClose} title={title} /><DialogBody className="space-y-5"><div className="flex gap-4"><span className="flex size-10 shrink-0 items-center justify-center rounded-[9px] bg-[var(--badge-info-bg)] text-[var(--badge-info-text)]"><CheckCircle2 className="size-5" /></span><p className="text-sm leading-6 text-[var(--text-muted)]">{description}</p></div>{action === "reopen" ? <div><Label htmlFor="gradebook-reopen-reason">Reason</Label><Textarea className="mt-1.5 min-h-28" data-autofocus="true" id="gradebook-reopen-reason" maxLength={1000} onChange={(event) => setReason(event.target.value)} required value={reason} /></div> : null}</DialogBody><DialogFooter><Button disabled={pending} onClick={onClose} type="button" variant="secondary">Cancel</Button><Button disabled={pending || (action === "reopen" && !reason.trim())} onClick={() => void run()} type="button">{pending ? <Loader2 className="size-4 animate-spin" /> : null}{pending ? "Updating…" : action === "submit" ? "Submit mark sheet" : action === "publish" ? "Publish marks" : "Reopen mark sheet"}</Button></DialogFooter></DialogShell>;
}

function updateMark(setMarks: React.Dispatch<React.SetStateAction<GradebookMark[]>>, id: string, patch: Partial<GradebookMark>) { setMarks((current) => current.map((mark) => mark.id === id ? { ...mark, ...patch } : mark)); }
function updateMarkStatus(setMarks: React.Dispatch<React.SetStateAction<GradebookMark[]>>, setRawScores: React.Dispatch<React.SetStateAction<Record<string, string>>>, id: string, status: GradebookMarkStatus) {
  setMarks((current) => current.map((mark) => mark.id === id ? { ...mark, mark_status: status, marks_awarded_hundredths: null, note: status === "unmarked" ? null : mark.note } : mark));
  setRawScores((current) => ({ ...current, [id]: "" }));
}
function markPayload(mark: GradebookMark, rawScore: string): GradebookMarkInput { return { learner_id: mark.learner_id, mark_status: mark.mark_status, marks_awarded_hundredths: mark.mark_status === "scored" ? parseHundredths(rawScore) : null, note: mark.mark_status === "unmarked" ? null : mark.note?.trim() || null }; }
function markFingerprint(marks: GradebookMarkInput[]) { return JSON.stringify([...marks].sort((left, right) => left.learner_id.localeCompare(right.learner_id))); }
function countMarks(marks: GradebookMark[]) { return marks.reduce((counts, mark) => ({ ...counts, [mark.mark_status]: counts[mark.mark_status] + 1 }), { unmarked: 0, scored: 0, absent: 0, exempt: 0 } as Record<GradebookMarkStatus, number>); }
function parseHundredths(value: string): number | null { const match = value.trim().match(/^(\d+)(?:\.(\d{0,2}))?$/); if (!match) return null; return Number(match[1]) * 100 + Number((match[2] ?? "").padEnd(2, "0")); }
function formatHundredths(value: number | null) { if (value === null) return ""; return `${Math.floor(value / 100)}.${String(value % 100).padStart(2, "0")}`; }
function normalizeScoreInput(value: string) { const match = value.match(/^\d*(?:\.\d{0,2})?$/); return match ? value : value.slice(0, -1); }
function percentageFromRaw(value: string, maximum: number) { const parsed = parseHundredths(value); return parsed === null || maximum <= 0 ? "—" : `${((parsed / (maximum * 100)) * 100).toFixed(1)}%`; }
function formatPercentage(value: number | null) { return value === null ? "—" : `${(value / 100).toFixed(1)}%`; }
function formatBasisPoints(value: number) { return `${(value / 100).toFixed(value % 100 === 0 ? 0 : 2)}%`; }
function statusTone(status: GradebookSheet["status"]): "warning" | "info" | "success" { return status === "published" ? "success" : status === "submitted" ? "info" : "warning"; }
function displayValue(value: string) { return value.replace(/_/g, " ").replace(/^./, (letter) => letter.toUpperCase()); }
function formatDate(value: string) { return new Intl.DateTimeFormat(undefined, { day: "numeric", month: "short", year: "numeric", timeZone: "UTC" }).format(new Date(`${value}T00:00:00Z`)); }
function Fact({ label, value }: { label: string; value: string }) { return <div className="border-b border-r border-[var(--border)] p-4 last:border-r-0 md:border-b-0"><p className="text-xs font-medium uppercase tracking-[0.12em] text-[var(--text-subtle)]">{label}</p><p className="mt-2 font-tabular text-lg font-semibold text-[var(--text-strong)]">{value}</p></div>; }
function Unavailable({ description, onRetry, title }: { description: string; onRetry?: () => void; title: string }) { return <div className="border border-[var(--border)] bg-[var(--surface)] p-8 text-center"><h1 className="text-lg font-semibold text-[var(--text-strong)]">{title}</h1><p className="mx-auto mt-2 max-w-lg text-sm text-[var(--text-muted)]">{description}</p>{onRetry ? <Button className="mt-5" onClick={onRetry} variant="secondary">Retry</Button> : <Link className="mt-5 inline-flex text-sm font-semibold text-[var(--brand-strong)] hover:underline" to="/modules/academics/gradebook">Back to gradebook</Link>}</div>; }

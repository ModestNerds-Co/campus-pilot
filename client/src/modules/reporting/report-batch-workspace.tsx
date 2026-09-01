import { useCallback, useEffect, useMemo, useState } from "react";
import { Link, useNavigate } from "@tanstack/react-router";
import { ArrowLeft, CheckCircle2, FileClock, Loader2, MessageSquareText, RotateCcw, Send, Trash2 } from "lucide-react";
import toast from "react-hot-toast";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { ConfirmDrawer } from "@/components/ui/confirm-drawer";
import { Table, TableScroll, TableWrap, TBody, TD, TH, THead, TR } from "@/components/ui/data-table";
import { DialogBody, DialogFooter, DialogHeader, DialogShell } from "@/components/ui/dialog";
import { Label, Select, Textarea } from "@/components/ui/input";
import { usePageChrome } from "@/modules/admin/layouts/page-chrome";
import { useAuthStore } from "@/stores/auth-store";

import { reportingService, responseMessage } from "./service";
import type { GradeLevelReference, ProgressionOutcome, ReportBatch, ReportCard } from "./types";

type BatchAction = "review" | "publish" | "reopen" | "delete" | null;
type CardDrawer = { kind: "teacher" | "review"; card: ReportCard } | null;

export function ReportBatchWorkspace({ reportBatchId }: { reportBatchId: string }) {
  const navigate = useNavigate();
  const permissions = useAuthStore((state) => state.user?.permissions ?? []);
  const canEdit = permissions.includes("*") || permissions.includes("academics:edit");
  const canManage = permissions.includes("*") || permissions.includes("academics:manage");
  const canDelete = permissions.includes("*") || permissions.includes("academics:delete");
  const [report, setReport] = useState<ReportBatch | null>(null);
  const [gradeLevels, setGradeLevels] = useState<GradeLevelReference[]>([]);
  const [selectedCardId, setSelectedCardId] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [notFound, setNotFound] = useState(false);
  const [action, setAction] = useState<BatchAction>(null);
  const [cardDrawer, setCardDrawer] = useState<CardDrawer>(null);
  const [pending, setPending] = useState(false);

  const applyReport = useCallback((next: ReportBatch) => {
    setReport(next);
    setSelectedCardId((current) => next.cards.some((card) => card.id === current) ? current : next.cards[0]?.id ?? null);
  }, []);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    setNotFound(false);
    try {
      const [reportResponse, referenceResponse] = await Promise.all([reportingService.readReportBatch(reportBatchId), reportingService.references()]);
      if (!reportResponse.success || !reportResponse.data) {
        if (reportResponse.issues?.some((issue) => (typeof issue === "string" ? issue : issue.detail)?.toLowerCase().includes("not found"))) setNotFound(true);
        else throw new Error(responseMessage(reportResponse, "Academic report could not be loaded"));
        return;
      }
      applyReport(reportResponse.data);
      if (referenceResponse.success && referenceResponse.data) setGradeLevels(referenceResponse.data.grade_levels);
    } catch (loadError) { setError(loadError instanceof Error ? loadError.message : "Academic report could not be loaded"); } finally { setLoading(false); }
  }, [applyReport, reportBatchId]);

  useEffect(() => { void load(); }, [load]);
  const selectedCard = useMemo(() => report?.cards.find((card) => card.id === selectedCardId) ?? report?.cards[0] ?? null, [report, selectedCardId]);

  usePageChrome("Academic report", report ? <div className="flex flex-wrap items-center gap-2">
    {report.status === "draft" && canManage ? <Button disabled={report.incomplete_subject_count > 0} onClick={() => setAction("review")}><CheckCircle2 className="size-4" />Complete review</Button> : null}
    {report.status === "reviewed" && canManage ? <Button onClick={() => setAction("publish")}><Send className="size-4" />Publish</Button> : null}
    {(report.status === "reviewed" || report.status === "published") && canManage ? <Button onClick={() => setAction("reopen")} variant="secondary"><RotateCcw className="size-4" />Reopen</Button> : null}
    {report.status === "draft" && canDelete ? <Button aria-label="Delete academic report" onClick={() => setAction("delete")} size="icon" variant="ghost"><Trash2 className="size-4" /></Button> : null}
  </div> : null);

  const runBatchAction = async (reason = "") => {
    if (!report || !action || action === "delete" || pending) return;
    setPending(true);
    try {
      const response = action === "review" ? await reportingService.reviewReportBatch(report.id, report.version)
        : action === "publish" ? await reportingService.publishReportBatch(report.id, report.version)
          : await reportingService.reopenReportBatch(report.id, report.version, reason);
      if (!response.success || !response.data) throw new Error(responseMessage(response, "Academic report could not be updated"));
      applyReport(response.data);
      toast.success(action === "review" ? "Report review completed" : action === "publish" ? "Academic reports published" : "Academic report reopened");
      setAction(null);
    } catch (actionError) { toast.error(actionError instanceof Error ? actionError.message : "Academic report could not be updated"); } finally { setPending(false); }
  };

  const deleteReport = async () => {
    if (!report || pending) return;
    setPending(true);
    try {
      const response = await reportingService.deleteReportBatch(report.id, report.version);
      if (!response.success) throw new Error(responseMessage(response, "Academic report could not be deleted"));
      toast.success("Academic report deleted");
      void navigate({ to: "/modules/academics/reporting" });
    } catch (deleteError) { toast.error(deleteError instanceof Error ? deleteError.message : "Academic report could not be deleted"); setPending(false); }
  };

  if (loading) return <Loading />;
  if (notFound) return <Unavailable description="This academic report does not exist or is no longer available." title="Academic report not found" />;
  if (error || !report) return <Unavailable description={error || "Academic report could not be loaded."} onRetry={() => void load()} title="Academic report unavailable" />;

  return <div className="space-y-6">
    <Link className="inline-flex items-center gap-2 text-sm font-medium text-[var(--text-muted)] hover:text-[var(--text-strong)]" to="/modules/academics/reporting"><ArrowLeft className="size-4" />Progress & reporting</Link>

    <section className="border border-[var(--border)] bg-[var(--surface)]">
      <div className="flex flex-col gap-4 border-b border-[var(--border)] p-5 sm:flex-row sm:items-start sm:justify-between sm:p-6"><div><p className="text-xs font-semibold uppercase tracking-[0.14em] text-[var(--brand-strong)]">{report.class_group_name} · {report.academic_year_name}</p><h1 className="mt-2 text-2xl font-semibold tracking-[-0.03em] text-[var(--text-strong)]">{report.assessment_cycle_name}</h1><p className="mt-2 text-sm text-[var(--text-muted)]">{report.academic_term_name} · {report.grading_scheme_name} version {report.grading_scheme_version}</p></div><Badge tone={statusTone(report.status)}>{displayValue(report.status)}</Badge></div>
      <div className="grid grid-cols-2 md:grid-cols-4"><Fact label="Learners" value={String(report.learner_count)} /><Fact label="Graded results" value={String(report.graded_subject_count)} /><Fact label="Incomplete" value={String(report.incomplete_subject_count)} /><Fact label="Published" value={report.published_at ? formatDateTime(report.published_at) : "—"} /></div>
    </section>

    {report.incomplete_subject_count > 0 ? <section className="border border-[var(--tone-danger-bd)] bg-[var(--tone-danger-bg)] p-4 text-sm text-[var(--tone-danger)]">This report cannot move to review while subject results are incomplete. Delete the draft, correct the Gradebook source, then generate it again.</section> : null}
    {report.reopen_reason ? <section className="border border-[var(--tone-warn-bd)] bg-[var(--badge-warning-bg)] p-4 text-sm text-[var(--badge-warning-text)]"><span className="font-semibold">Reopened:</span> {report.reopen_reason}</section> : null}

    <div><h2 className="text-lg font-semibold text-[var(--text-strong)]">Learner reports</h2><p className="mt-1 text-sm text-[var(--text-muted)]">Select a learner to review their result snapshot and remarks.</p></div>
    <TableWrap><TableScroll><Table className="min-w-[850px]"><THead><tr><TH>Learner</TH><TH>Overall</TH><TH>Attendance</TH><TH>Progression</TH><TH>Results</TH><TH className="w-28">Action</TH></tr></THead><TBody>
      {report.cards.map((card) => <TR className={card.id === selectedCard?.id ? "bg-[var(--surface-muted)]" : undefined} key={card.id}><TD><p className="font-medium text-[var(--text-strong)]">{card.learner_name}</p><p className="mt-1 font-tabular text-xs text-[var(--text-muted)]">{card.learner_number}</p></TD><TD><p className="font-tabular font-medium text-[var(--text-strong)]">{formatPercentage(card.overall_percentage_basis_points)}</p><p className="mt-1 text-xs text-[var(--text-muted)]">{card.overall_grade_code ?? "—"}{card.overall_grade_label ? ` · ${card.overall_grade_label}` : ""}</p></TD><TD className="font-tabular text-[var(--text-muted)]">{formatPercentage(card.attendance.attendance_percentage_basis_points)}</TD><TD><Badge tone={progressionTone(card.progression_outcome)}>{displayValue(card.progression_outcome)}</Badge></TD><TD>{card.subjects.some((subject) => subject.result_status === "incomplete") ? <Badge tone="danger">Incomplete</Badge> : <Badge tone="success">Complete</Badge>}</TD><TD><Button onClick={() => setSelectedCardId(card.id)} size="sm" variant={card.id === selectedCard?.id ? "secondary" : "ghost"}>View</Button></TD></TR>)}
    </TBody></Table></TableScroll></TableWrap>

    {selectedCard ? <ReportCardDetail canEdit={canEdit && report.status === "draft"} canManage={canManage && report.status === "draft"} card={selectedCard} onEdit={(kind) => setCardDrawer({ kind, card: selectedCard })} published={report.status === "published"} /> : null}

    <BatchActionDrawer action={action === "review" || action === "publish" || action === "reopen" ? action : null} onClose={() => setAction(null)} onConfirm={(reason) => void runBatchAction(reason)} pending={pending} />
    <ConfirmDrawer confirmLabel="Delete report batch" description={`Delete the draft reports for ${report.assessment_cycle_name} and ${report.class_group_name}?`} isPending={pending} onClose={() => setAction(null)} onConfirm={() => void deleteReport()} open={action === "delete"} title="Delete academic reports?" />
    <ReportCardDrawer drawer={cardDrawer} gradeLevels={gradeLevels} onClose={() => setCardDrawer(null)} onSaved={(next) => { applyReport(next); setCardDrawer(null); }} />
  </div>;
}

function ReportCardDetail({ canEdit, canManage, card, onEdit, published }: { canEdit: boolean; canManage: boolean; card: ReportCard; onEdit: (kind: "teacher" | "review") => void; published: boolean }) {
  return <section className="border border-[var(--border)] bg-[var(--surface)]"><div className="flex flex-col gap-4 border-b border-[var(--border)] p-5 sm:flex-row sm:items-start sm:justify-between sm:p-6"><div><h2 className="text-xl font-semibold text-[var(--text-strong)]">{card.learner_name}</h2><p className="mt-1 font-tabular text-sm text-[var(--text-muted)]">{card.learner_number}</p></div>{published ? <Link className="text-sm font-semibold text-[var(--brand-strong)] hover:underline" params={{ learnerId: card.learner_id }} to="/modules/academics/reporting/transcripts/$learnerId">Open transcript</Link> : null}</div>
    <div className="grid gap-0 border-b border-[var(--border)] sm:grid-cols-2 lg:grid-cols-5"><Fact label="Overall" value={formatPercentage(card.overall_percentage_basis_points)} /><Fact label="Grade" value={card.overall_grade_code ?? "—"} /><Fact label="Attendance" value={formatPercentage(card.attendance.attendance_percentage_basis_points)} /><Fact label="Present / late" value={`${card.attendance.present_count} / ${card.attendance.late_count}`} /><Fact label="Absent / excused" value={`${card.attendance.absent_count} / ${card.attendance.excused_count}`} /></div>
    <div className="p-5 sm:p-6"><h3 className="font-semibold text-[var(--text-strong)]">Subject results</h3><TableWrap className="mt-4 shadow-none"><TableScroll><Table className="min-w-[720px]"><THead><tr><TH>Subject</TH><TH>Result</TH><TH>Grade</TH><TH>Components</TH><TH>Pass</TH></tr></THead><TBody>{card.subjects.map((subject) => <TR key={subject.id}><TD className="font-medium text-[var(--text-strong)]">{subject.subject_name}</TD><TD className="font-tabular text-[var(--text-muted)]">{subject.result_status === "graded" ? formatPercentage(subject.percentage_basis_points) : displayValue(subject.result_status)}</TD><TD>{subject.grade_code ?? "—"}{subject.grade_label ? ` · ${subject.grade_label}` : ""}</TD><TD className="font-tabular text-[var(--text-muted)]">{subject.scored_component_count} scored · {subject.absent_component_count} absent · {subject.exempt_component_count} exempt</TD><TD>{subject.is_pass === null ? "—" : <Badge tone={subject.is_pass ? "success" : "danger"}>{subject.is_pass ? "Pass" : "Not passed"}</Badge>}</TD></TR>)}</TBody></Table></TableScroll></TableWrap></div>
    <div className="grid border-t border-[var(--border)] lg:grid-cols-2"><Remark label="Teacher comment" onEdit={canEdit ? () => onEdit("teacher") : undefined} value={card.teacher_comment} /><Remark label="Review and progression" onEdit={canManage ? () => onEdit("review") : undefined} value={[card.reviewer_comment, displayValue(card.progression_outcome), card.target_grade_level_name].filter(Boolean).join(" · ") || null} /></div>
  </section>;
}

function ReportCardDrawer({ drawer, gradeLevels, onClose, onSaved }: { drawer: CardDrawer; gradeLevels: GradeLevelReference[]; onClose: () => void; onSaved: (report: ReportBatch) => void }) {
  const [comment, setComment] = useState("");
  const [outcome, setOutcome] = useState<ProgressionOutcome>("not_applicable");
  const [targetId, setTargetId] = useState("");
  const [saving, setSaving] = useState(false);
  useEffect(() => { if (drawer) { setComment(drawer.kind === "teacher" ? drawer.card.teacher_comment ?? "" : drawer.card.reviewer_comment ?? ""); setOutcome(drawer.card.progression_outcome); setTargetId(drawer.card.target_grade_level_id ?? ""); } }, [drawer]);
  if (!drawer) return null;
  const submit = async (event: React.FormEvent) => {
    event.preventDefault();
    if (outcome === "promoted" && !targetId) { toast.error("Choose the target grade level"); return; }
    setSaving(true);
    try {
      const response = drawer.kind === "teacher" ? await reportingService.updateTeacherComment(drawer.card.id, drawer.card.version, comment.trim() || null) : await reportingService.updateReportReview(drawer.card.id, { expected_version: drawer.card.version, reviewer_comment: comment.trim() || null, progression_outcome: outcome, target_grade_level_id: outcome === "promoted" ? targetId : null });
      if (!response.success || !response.data) throw new Error(responseMessage(response, "Report card could not be updated"));
      toast.success(drawer.kind === "teacher" ? "Teacher comment saved" : "Review saved");
      onSaved(response.data);
    } catch (saveError) { toast.error(saveError instanceof Error ? saveError.message : "Report card could not be updated"); } finally { setSaving(false); }
  };
  return <DialogShell onClose={saving ? () => undefined : onClose} open><DialogHeader onClose={saving ? undefined : onClose} title={drawer.kind === "teacher" ? "Teacher comment" : "Review and progression"} /><form onSubmit={submit}><DialogBody className="space-y-5"><div><p className="font-medium text-[var(--text-strong)]">{drawer.card.learner_name}</p><p className="mt-1 font-tabular text-xs text-[var(--text-muted)]">{drawer.card.learner_number}</p></div><div><Label htmlFor="report-card-comment">{drawer.kind === "teacher" ? "Comment" : "Reviewer comment"}</Label><Textarea className="mt-1.5 min-h-36" data-autofocus="true" id="report-card-comment" maxLength={2000} onChange={(event) => setComment(event.target.value)} value={comment} /></div>{drawer.kind === "review" ? <><div><Label htmlFor="report-card-outcome">Progression outcome</Label><Select className="mt-1.5" id="report-card-outcome" onChange={(event) => { const next = event.target.value as ProgressionOutcome; setOutcome(next); if (next !== "promoted") setTargetId(""); }} value={outcome}><option value="not_applicable">Not applicable</option><option value="pending">Pending</option><option value="promoted">Promoted</option><option value="retained">Retained</option><option value="completed">Completed</option></Select></div>{outcome === "promoted" ? <div><Label htmlFor="report-card-target">Target grade level</Label><Select className="mt-1.5" id="report-card-target" onChange={(event) => setTargetId(event.target.value)} required value={targetId}><option value="">Choose grade level</option>{gradeLevels.map((grade) => <option key={grade.id} value={grade.id}>{grade.name} · {grade.code}</option>)}</Select></div> : null}</> : null}</DialogBody><DialogFooter><Button disabled={saving} onClick={onClose} type="button" variant="secondary">Cancel</Button><Button disabled={saving || (drawer.kind === "review" && outcome === "promoted" && !targetId)} type="submit">{saving ? <Loader2 className="size-4 animate-spin" /> : null}{saving ? "Saving…" : "Save"}</Button></DialogFooter></form></DialogShell>;
}

function BatchActionDrawer({ action, onClose, onConfirm, pending }: { action: "review" | "publish" | "reopen" | null; onClose: () => void; onConfirm: (reason: string) => void; pending: boolean }) {
  const [reason, setReason] = useState("");
  useEffect(() => { if (action) setReason(""); }, [action]);
  if (!action) return null;
  const content = action === "review" ? { title: "Complete report review?", description: "This locks report-card remarks and progression decisions until the batch is reopened.", label: "Complete review" } : action === "publish" ? { title: "Publish academic reports?", description: "Published reports become available in learner transcripts. Reopening requires a recorded reason.", label: "Publish reports" } : { title: "Reopen academic reports?", description: "Reopening returns the batch to draft so remarks and progression decisions can be corrected.", label: "Reopen reports" };
  return <DialogShell onClose={pending ? () => undefined : onClose} open><DialogHeader onClose={pending ? undefined : onClose} title={content.title} /><DialogBody className="space-y-5"><div className="flex gap-4"><span className="flex size-10 shrink-0 items-center justify-center rounded-[9px] bg-[var(--badge-info-bg)] text-[var(--badge-info-text)]"><FileClock className="size-5" /></span><p className="text-sm leading-6 text-[var(--text-muted)]">{content.description}</p></div>{action === "reopen" ? <div><Label htmlFor="report-reopen-reason">Reason</Label><Textarea className="mt-1.5 min-h-28" data-autofocus="true" id="report-reopen-reason" maxLength={1000} onChange={(event) => setReason(event.target.value)} required value={reason} /></div> : null}</DialogBody><DialogFooter><Button disabled={pending} onClick={onClose} variant="secondary">Cancel</Button><Button disabled={pending || (action === "reopen" && !reason.trim())} onClick={() => onConfirm(reason.trim())}>{pending ? <Loader2 className="size-4 animate-spin" /> : null}{pending ? "Updating…" : content.label}</Button></DialogFooter></DialogShell>;
}

function Remark({ label, onEdit, value }: { label: string; onEdit?: () => void; value: string | null }) { return <div className="border-b border-[var(--border)] p-5 last:border-b-0 lg:border-b-0 lg:border-r lg:last:border-r-0 sm:p-6"><div className="flex items-center justify-between gap-4"><h3 className="font-semibold text-[var(--text-strong)]">{label}</h3>{onEdit ? <Button onClick={onEdit} size="sm" variant="ghost"><MessageSquareText className="size-4" />Edit</Button> : null}</div><p className="mt-3 text-sm leading-6 text-[var(--text-muted)]">{value || "No comment recorded."}</p></div>; }
function Fact({ label, value }: { label: string; value: string }) { return <div className="border-b border-r border-[var(--border)] p-4 last:border-r-0 md:border-b-0"><p className="text-xs font-medium uppercase tracking-[0.12em] text-[var(--text-subtle)]">{label}</p><p className="mt-2 font-tabular text-base font-semibold text-[var(--text-strong)]">{value}</p></div>; }
function Loading() { return <div aria-label="Loading academic report" className="flex min-h-64 items-center justify-center border border-[var(--border)] bg-[var(--surface)]" role="status"><Loader2 className="size-6 animate-spin text-[var(--brand-strong)]" /></div>; }
function Unavailable({ description, onRetry, title }: { description: string; onRetry?: () => void; title: string }) { return <div className="border border-[var(--border)] bg-[var(--surface)] p-8 text-center"><h1 className="text-lg font-semibold text-[var(--text-strong)]">{title}</h1><p className="mx-auto mt-2 max-w-lg text-sm text-[var(--text-muted)]">{description}</p>{onRetry ? <Button className="mt-5" onClick={onRetry} variant="secondary">Retry</Button> : <Link className="mt-5 inline-flex text-sm font-semibold text-[var(--brand-strong)] hover:underline" to="/modules/academics/reporting">Back to reporting</Link>}</div>; }
function statusTone(status: ReportBatch["status"]): "warning" | "info" | "success" { return status === "published" ? "success" : status === "reviewed" ? "info" : "warning"; }
function progressionTone(outcome: ProgressionOutcome): "neutral" | "info" | "success" | "warning" { return outcome === "promoted" || outcome === "completed" ? "success" : outcome === "pending" ? "warning" : outcome === "retained" ? "info" : "neutral"; }
function formatPercentage(value: number | null) { return value === null ? "—" : `${(value / 100).toFixed(value % 100 === 0 ? 0 : 1)}%`; }
function formatDateTime(value: string) { return new Intl.DateTimeFormat(undefined, { day: "numeric", month: "short", year: "numeric" }).format(new Date(value)); }
function displayValue(value: string) { return value.replace(/_/g, " ").replace(/^./, (letter) => letter.toUpperCase()); }

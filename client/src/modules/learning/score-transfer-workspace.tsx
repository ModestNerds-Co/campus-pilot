/** Full-page review of one immutable Learning-to-Gradebook score proposal. */

import { useCallback, useEffect, useState } from "react";
import { Link } from "@tanstack/react-router";
import { ArrowLeft, ArrowRightLeft, CheckCircle2, Loader2, XCircle } from "lucide-react";
import toast from "react-hot-toast";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Table, TableScroll, TableWrap, TBody, TD, TH, THead, TR } from "@/components/ui/data-table";
import { DialogBody, DialogFooter, DialogHeader, DialogShell } from "@/components/ui/dialog";
import { Label, Textarea } from "@/components/ui/input";
import { usePageChrome } from "@/modules/admin/layouts/page-chrome";
import { useAuthStore } from "@/stores/auth-store";

import { learningService, responseMessage } from "./service";
import type { LearningScoreTransfer } from "./types";
import { formatHundredths, formatLearningDateTime, LearningState, LearningStatusBadge } from "./ui";

export function LearningScoreTransferWorkspace({ proposalId }: { proposalId: string }) {
  const user = useAuthStore((state) => state.user);
  const permissions = user?.permissions ?? [];
  const canManage = permissions.includes("*") || permissions.includes("learning:manage");
  const [record, setRecord] = useState<LearningScoreTransfer | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [decision, setDecision] = useState<"apply" | "reject" | null>(null);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const response = await learningService.scoreTransfer(proposalId);
      if (!response.success || !response.data) {
        throw new Error(responseMessage(response, "Score transfer could not be loaded"));
      }
      setRecord(response.data);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : "Score transfer could not be loaded");
    } finally {
      setLoading(false);
    }
  }, [proposalId]);

  useEffect(() => { void load(); }, [load]);
  usePageChrome("Score transfer");

  if (loading) return <LearningState busy title="Loading score transfer…" />;
  if (error) return <LearningState description={error} onRetry={() => void load()} title="Score transfer unavailable" />;
  if (!record) return <LearningState description="This score transfer does not exist or is outside your current access." title="Score transfer not found" />;

  const ownProposal = user?.id === record.proposed_by_id;
  const canReview = canManage && record.status === "pending" && !ownProposal;
  return <div className="space-y-7">
    <Link className="inline-flex items-center gap-2 text-sm font-medium text-[var(--text-muted)] hover:text-[var(--text-strong)]" search={{ page: 1, status: "all" }} to="/modules/learning/score-transfers"><ArrowLeft className="size-4" />Score transfers</Link>
    <header className="flex flex-wrap items-start justify-between gap-5"><div><div className="flex flex-wrap items-center gap-2"><LearningStatusBadge status={record.status} /><Badge>{label(record.source_type)}</Badge></div><h1 className="mt-3 text-2xl font-semibold tracking-[-0.03em] text-[var(--text-strong)]">{record.source_title}</h1><p className="mt-2 text-sm text-[var(--text-muted)]">{record.learning_space_title} · {record.subject_name} · {record.class_group_name}</p></div>{canReview ? <div className="flex flex-wrap gap-2"><Button onClick={() => setDecision("reject")} variant="secondary"><XCircle className="size-4" />Reject</Button><Button onClick={() => setDecision("apply")}><CheckCircle2 className="size-4" />Apply to Gradebook</Button></div> : null}</header>

    {record.status === "pending" && ownProposal ? <section className="border border-[var(--border)] bg-[var(--surface-muted)] p-4 text-sm text-[var(--text-muted)]">Prepared by you. A different Academic Manager must review it.</section> : null}
    <section className="grid gap-px border border-[var(--border)] bg-[var(--border)] sm:grid-cols-2 lg:grid-cols-4">{[
      ["Gradebook target", record.target_assessment_name],
      ["Ready", record.ready_count.toString()],
      ["Missing source", record.missing_source_count.toString()],
      ["Already marked", record.target_already_marked_count.toString()],
    ].map(([name, value]) => <div className="bg-[var(--surface)] p-4" key={name}><p className="text-xs font-semibold uppercase tracking-[0.12em] text-[var(--text-muted)]">{name}</p><p className="mt-2 font-tabular text-lg font-semibold text-[var(--text-strong)]">{value}</p></div>)}</section>

    <section className="grid gap-4 border border-[var(--border)] bg-[var(--surface)] p-5 text-sm sm:grid-cols-2 lg:grid-cols-4"><Fact label="Prepared by" value={record.proposed_by_name} /><Fact label="Prepared" value={formatLearningDateTime(record.proposed_at)} /><Fact label="Target maximum" value={`${record.target_maximum_marks} marks`} /><Fact label="Reviewed by" value={record.reviewed_by_name ?? "Not reviewed"} />{record.review_reason ? <div className="sm:col-span-2 lg:col-span-4"><Fact label="Review reason" value={record.review_reason} /></div> : null}</section>

    <section><div className="mb-4"><h2 className="text-lg font-semibold text-[var(--text-strong)]">Learner rows</h2><p className="mt-1 text-sm text-[var(--text-muted)]">Ready rows will update unmarked Gradebook entries only.</p></div><TableWrap><TableScroll><Table className="min-w-[760px]"><THead><tr><TH>Learner</TH><TH>Learning score</TH><TH>Proposed mark</TH><TH>Outcome</TH></tr></THead><TBody>{record.rows.map((row) => <TR key={row.id}><TD><p className="font-medium text-[var(--text-strong)]">{row.learner_name}</p><p className="mt-1 font-tabular text-xs text-[var(--text-muted)]">{row.learner_number}</p></TD><TD className="font-tabular text-[var(--text-muted)]">{row.source_score_basis_points === null ? "—" : `${formatBasisPoints(row.source_score_basis_points)}`}</TD><TD className="font-tabular">{row.proposed_marks_hundredths === null ? "—" : `${formatHundredths(row.proposed_marks_hundredths)} / ${record.target_maximum_marks}`}</TD><TD><LearningStatusBadge status={row.outcome} /></TD></TR>)}</TBody></Table></TableScroll></TableWrap></section>

    <p className="flex items-start gap-2 text-xs leading-5 text-[var(--text-muted)]"><ArrowRightLeft className="mt-0.5 size-4 shrink-0" />Applying records both the Learning review and the Gradebook mark-sheet event in one transaction.</p>
    <ReviewDecisionDrawer decision={decision} onClose={() => setDecision(null)} onSaved={(next) => { setRecord(next); setDecision(null); }} record={record} />
  </div>;
}

function ReviewDecisionDrawer({ decision, onClose, onSaved, record }: { decision: "apply" | "reject" | null; onClose: () => void; onSaved: (next: LearningScoreTransfer) => void; record: LearningScoreTransfer }) {
  const [reason, setReason] = useState("");
  const [pending, setPending] = useState(false);
  useEffect(() => { if (decision) setReason(""); }, [decision]);
  if (!decision) return null;
  const run = async () => {
    if (pending || (decision === "reject" && !reason.trim())) return;
    setPending(true);
    try {
      const response = decision === "apply" ? await learningService.applyScoreTransfer(record) : await learningService.rejectScoreTransfer(record, reason.trim());
      if (!response.success || !response.data) throw new Error(responseMessage(response, `Score transfer could not be ${decision === "apply" ? "applied" : "rejected"}`));
      toast.success(decision === "apply" ? "Scores applied to Gradebook" : "Score transfer rejected");
      onSaved(response.data);
    } catch (cause) {
      toast.error(cause instanceof Error ? cause.message : "Score-transfer review failed");
    } finally {
      setPending(false);
    }
  };
  return <DialogShell onClose={pending ? () => undefined : onClose} open><DialogHeader onClose={pending ? undefined : onClose} title={decision === "apply" ? "Apply scores to Gradebook?" : "Reject score transfer?"} /><DialogBody className="space-y-5"><p className="text-sm leading-6 text-[var(--text-muted)]">{decision === "apply" ? `${record.ready_count} ready learner mark${record.ready_count === 1 ? "" : "s"} will be written to the unchanged draft mark sheet. Missing and already-marked rows remain untouched.` : "The proposal and its learner-row evidence will remain in the review history."}</p>{decision === "reject" ? <div><Label htmlFor="score-transfer-rejection">Reason</Label><Textarea className="mt-1.5 min-h-32" data-autofocus="true" id="score-transfer-rejection" maxLength={2000} onChange={(event) => setReason(event.target.value)} required value={reason} /></div> : null}</DialogBody><DialogFooter><Button disabled={pending} onClick={onClose} type="button" variant="secondary">Cancel</Button><Button disabled={pending || (decision === "reject" && !reason.trim())} onClick={() => void run()} type="button" variant={decision === "reject" ? "destructive" : "default"}>{pending ? <Loader2 className="size-4 animate-spin" /> : null}{pending ? "Recording…" : decision === "apply" ? "Apply scores" : "Reject proposal"}</Button></DialogFooter></DialogShell>;
}

function Fact({ label: name, value }: { label: string; value: string }) { return <div><p className="text-xs font-semibold uppercase tracking-[0.12em] text-[var(--text-muted)]">{name}</p><p className="mt-1 text-[var(--text-strong)]">{value}</p></div>; }
function label(value: string) { return value.replace(/_/g, " ").replace(/^./, (letter) => letter.toUpperCase()); }
function formatBasisPoints(value: number) { return `${(value / 100).toFixed(value % 100 === 0 ? 0 : 2)}%`; }

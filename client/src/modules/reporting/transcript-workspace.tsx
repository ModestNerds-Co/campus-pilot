import { useCallback, useEffect, useState } from "react";
import { Link } from "@tanstack/react-router";
import { ArrowLeft, FileText, Loader2 } from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Table, TableEmpty, TableScroll, TableWrap, TBody, TD, TH, THead, TR } from "@/components/ui/data-table";
import { usePageChrome } from "@/modules/admin/layouts/page-chrome";

import { reportingService, responseMessage } from "./service";
import type { LearnerTranscript, ProgressionOutcome } from "./types";

export function TranscriptWorkspace({ learnerId }: { learnerId: string }) {
  const [transcript, setTranscript] = useState<LearnerTranscript | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [notFound, setNotFound] = useState(false);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    setNotFound(false);
    try {
      const response = await reportingService.learnerTranscript(learnerId);
      if (!response.success || !response.data) {
        if (response.issues?.some((issue) => (typeof issue === "string" ? issue : issue.detail)?.toLowerCase().includes("not found"))) setNotFound(true);
        else throw new Error(responseMessage(response, "Learner transcript could not be loaded"));
        return;
      }
      setTranscript(response.data);
    } catch (loadError) { setError(loadError instanceof Error ? loadError.message : "Learner transcript could not be loaded"); } finally { setLoading(false); }
  }, [learnerId]);

  useEffect(() => { void load(); }, [load]);
  usePageChrome("Learner transcript");

  if (loading) return <div aria-label="Loading learner transcript" className="flex min-h-64 items-center justify-center border border-[var(--border)] bg-[var(--surface)]" role="status"><Loader2 className="size-6 animate-spin text-[var(--brand-strong)]" /></div>;
  if (notFound) return <Unavailable description="This learner does not exist or their transcript is unavailable to this account." title="Transcript not found" />;
  if (error || !transcript) return <Unavailable description={error || "Learner transcript could not be loaded."} onRetry={() => void load()} title="Transcript unavailable" />;

  return <div className="space-y-6">
    <Link className="inline-flex items-center gap-2 text-sm font-medium text-[var(--text-muted)] hover:text-[var(--text-strong)]" to="/modules/academics/reporting"><ArrowLeft className="size-4" />Progress & reporting</Link>
    <section className="border border-[var(--border)] bg-[var(--surface)] p-5 sm:p-6"><p className="text-xs font-semibold uppercase tracking-[0.14em] text-[var(--brand-strong)]">Published academic record</p><h1 className="mt-2 text-2xl font-semibold tracking-[-0.03em] text-[var(--text-strong)]">{transcript.learner_name}</h1><p className="mt-2 font-tabular text-sm text-[var(--text-muted)]">{transcript.learner_number}</p></section>
    {transcript.entries.length === 0 ? <TableWrap><TableEmpty description="Published report cards will appear here." icon={<FileText />} title="No published results yet" /></TableWrap> : transcript.entries.map((entry) => <section className="border border-[var(--border)] bg-[var(--surface)]" key={entry.report_batch_id}><div className="flex flex-col gap-4 border-b border-[var(--border)] p-5 sm:flex-row sm:items-start sm:justify-between sm:p-6"><div><p className="text-xs font-semibold uppercase tracking-[0.14em] text-[var(--brand-strong)]">{entry.academic_year_name} · {entry.class_group_name}</p><h2 className="mt-2 text-xl font-semibold text-[var(--text-strong)]">{entry.assessment_cycle_name}</h2><p className="mt-1 text-sm text-[var(--text-muted)]">{entry.academic_term_name} · Published {formatDate(entry.published_at)}</p></div><div className="flex flex-wrap items-center gap-2"><Badge tone={progressionTone(entry.progression_outcome)}>{displayValue(entry.progression_outcome)}</Badge><Badge tone="brand">{formatPercentage(entry.overall_percentage_basis_points)}{entry.overall_grade_code ? ` · ${entry.overall_grade_code}` : ""}</Badge></div></div><TableScroll><Table className="min-w-[680px]"><THead><tr><TH>Subject</TH><TH>Result</TH><TH>Grade</TH><TH>Pass</TH></tr></THead><TBody>{entry.subjects.map((subject) => <TR key={subject.id}><TD className="font-medium text-[var(--text-strong)]">{subject.subject_name}</TD><TD className="font-tabular text-[var(--text-muted)]">{subject.result_status === "graded" ? formatPercentage(subject.percentage_basis_points) : displayValue(subject.result_status)}</TD><TD>{subject.grade_code ?? "—"}{subject.grade_label ? ` · ${subject.grade_label}` : ""}</TD><TD>{subject.is_pass === null ? "—" : <Badge tone={subject.is_pass ? "success" : "danger"}>{subject.is_pass ? "Pass" : "Not passed"}</Badge>}</TD></TR>)}</TBody></Table></TableScroll></section>)}
  </div>;
}

function Unavailable({ description, onRetry, title }: { description: string; onRetry?: () => void; title: string }) { return <div className="border border-[var(--border)] bg-[var(--surface)] p-8 text-center"><h1 className="text-lg font-semibold text-[var(--text-strong)]">{title}</h1><p className="mx-auto mt-2 max-w-lg text-sm text-[var(--text-muted)]">{description}</p>{onRetry ? <Button className="mt-5" onClick={onRetry} variant="secondary">Retry</Button> : <Link className="mt-5 inline-flex text-sm font-semibold text-[var(--brand-strong)] hover:underline" to="/modules/academics/reporting">Back to reporting</Link>}</div>; }
function progressionTone(outcome: ProgressionOutcome): "neutral" | "info" | "success" | "warning" { return outcome === "promoted" || outcome === "completed" ? "success" : outcome === "pending" ? "warning" : outcome === "retained" ? "info" : "neutral"; }
function formatPercentage(value: number | null) { return value === null ? "—" : `${(value / 100).toFixed(value % 100 === 0 ? 0 : 1)}%`; }
function formatDate(value: string) { return new Intl.DateTimeFormat(undefined, { day: "numeric", month: "short", year: "numeric" }).format(new Date(value)); }
function displayValue(value: string) { return value.replace(/_/g, " ").replace(/^./, (letter) => letter.toUpperCase()); }

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Link } from "@tanstack/react-router";
import { CheckCircle2, Loader2, Save, Send } from "lucide-react";
import toast from "react-hot-toast";

import { Button } from "@/components/ui/button";
import { DialogBody, DialogFooter, DialogHeader, DialogShell } from "@/components/ui/dialog";
import { Input, Label, Textarea } from "@/components/ui/input";
import { usePageChrome } from "@/modules/admin/layouts/page-chrome";
import { useAuthStore } from "@/stores/auth-store";

import { clearLearningRecovery, purgeLearningRecoveryForOtherUsers, readLearningRecovery, writeLearningRecovery } from "./draft-recovery";
import { learningService, responseMessage } from "./service";
import type { LearningAssignment, LearningFeedback, LearningReviewOutcome, LearningSubmission } from "./types";
import { formatHundredths, formatLearningDateTime, LearningState, LearningStatusBadge, parseHundredths } from "./ui";

export function LearningSubmissionWorkspace({ onVersionChange, submissionId, versionId }: {
  onVersionChange: (versionId: string) => void;
  submissionId: string;
  versionId: string;
}) {
  const user = useAuthStore((state) => state.user);
  const permissions = useAuthStore((state) => state.user?.permissions ?? []);
  const canTeach = permissions.includes("*") || permissions.includes("learning:teach");
  const [submission, setSubmission] = useState<LearningSubmission | null>(null);
  const [assignment, setAssignment] = useState<LearningAssignment | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [overall, setOverall] = useState("");
  const [scores, setScores] = useState<Record<string, string>>({});
  const [savedFingerprint, setSavedFingerprint] = useState("");
  const [saveState, setSaveState] = useState<"idle" | "saving" | "saved" | "error">("idle");
  const [saveError, setSaveError] = useState<string | null>(null);
  const [releaseOutcome, setReleaseOutcome] = useState<LearningReviewOutcome | null>(null);
  const [releasing, setReleasing] = useState(false);
  const requestRef = useRef(0);
  const saveRequestRef = useRef(0);
  const releaseKeyRef = useRef<string | null>(null);

  const load = useCallback(async () => {
    const requestId = ++requestRef.current;
    setLoading(true);
    setError(null);
    try {
      const submissionResponse = await learningService.submission(submissionId);
      if (!submissionResponse.success || !submissionResponse.data) throw new Error(responseMessage(submissionResponse, "Submission could not be loaded"));
      const assignmentResponse = await learningService.assignment(submissionResponse.data.learning_assignment_id);
      if (!assignmentResponse.success || !assignmentResponse.data) throw new Error(responseMessage(assignmentResponse, "Assignment could not be loaded"));
      if (requestId !== requestRef.current) return;
      setSubmission(submissionResponse.data);
      setAssignment(assignmentResponse.data);
    } catch (loadError) {
      if (requestId !== requestRef.current) return;
      setError(loadError instanceof Error ? loadError.message : "Submission could not be loaded");
    } finally {
      if (requestId === requestRef.current) setLoading(false);
    }
  }, [submissionId]);

  useEffect(() => {
    if (user) purgeLearningRecoveryForOtherUsers(user.id);
    void load();
    return () => { requestRef.current += 1; saveRequestRef.current += 1; };
  }, [load, user]);

  const latestVersion = submission?.versions[submission.versions.length - 1] ?? null;
  const selectedVersion = submission?.versions.find((version) => version.id === versionId) ?? latestVersion;
  useEffect(() => {
    if (submission && selectedVersion && versionId !== selectedVersion.id) onVersionChange(selectedVersion.id);
  }, [onVersionChange, selectedVersion, submission, versionId]);

  useEffect(() => {
    if (!submission || !assignment || !selectedVersion) return;
    const feedback = submission.feedback?.submission_version_id === selectedVersion.id ? submission.feedback : null;
    const serverOverall = feedback?.overall_feedback ?? "";
    const recovery = user && selectedVersion.id === submission.current_submission_version_id
      ? readLearningRecovery(user.id, "review", submission.id)
      : null;
    const useRecovery = recovery && recovery.savedAt > new Date(submission.updated_at).getTime();
    const initialOverall = useRecovery ? recovery.body : serverOverall;
    const initialScores = Object.fromEntries((feedback?.scores ?? []).map((score) => [score.rubric_criterion_id, formatHundredths(score.earned_score_hundredths)]));
    setOverall(initialOverall);
    setScores(initialScores);
    setSavedFingerprint(reviewFingerprint(serverOverall, initialScores));
    setSaveState(initialOverall === serverOverall ? "saved" : "idle");
  }, [assignment, selectedVersion, submission, user]);

  useEffect(() => {
    if (!user) return;
    const currentUserId = user.id;
    return () => {
      if (useAuthStore.getState().user?.id !== currentUserId) clearLearningRecovery(currentUserId, "review", submissionId);
    };
  }, [submissionId, user]);

  const currentFeedback = submission?.feedback ?? null;
  const selectedFeedback = currentFeedback?.submission_version_id === selectedVersion?.id ? currentFeedback : null;
  const editable = Boolean(canTeach && submission?.status === "submitted" && selectedVersion?.id === submission.current_submission_version_id && selectedFeedback?.status !== "released");
  const currentFingerprint = useMemo(() => reviewFingerprint(overall, scores), [overall, scores]);
  const dirty = editable && currentFingerprint !== savedFingerprint;

  useEffect(() => {
    if (!dirty || !user || !submission) return;
    writeLearningRecovery(user.id, "review", submission.id, overall);
    const warn = (event: BeforeUnloadEvent) => { event.preventDefault(); event.returnValue = ""; };
    window.addEventListener("beforeunload", warn);
    return () => window.removeEventListener("beforeunload", warn);
  }, [dirty, overall, submission, user]);

  const save = useCallback(async () => {
    if (!submission || !assignment || !selectedVersion || !editable || !dirty) return submission?.feedback ?? null;
    const requestId = ++saveRequestRef.current;
    setSaveState("saving");
    setSaveError(null);
    try {
      const scorePayload = assignment.rubric.flatMap((criterion) => {
        const parsed = parseHundredths(scores[criterion.id] ?? "");
        return parsed === null ? [] : [{ rubric_criterion_id: criterion.id, earned_score_hundredths: parsed, feedback: null }];
      });
      const response = await learningService.updateFeedback(submission.id, {
        submission_version_id: selectedVersion.id,
        overall_feedback: overall.trim() || null,
        scores: scorePayload,
        expected_review_version: selectedFeedback?.version ?? null,
      });
      if (!response.success || !response.data) throw new Error(responseMessage(response, "Feedback draft could not be saved"));
      if (requestId !== saveRequestRef.current) return response.data;
      setSubmission((current) => current ? { ...current, feedback: response.data } : current);
      const normalizedScores = Object.fromEntries(response.data.scores.map((score) => [score.rubric_criterion_id, formatHundredths(score.earned_score_hundredths)]));
      setScores(normalizedScores);
      setSavedFingerprint(reviewFingerprint(response.data.overall_feedback ?? "", normalizedScores));
      setSaveState("saved");
      if (user) clearLearningRecovery(user.id, "review", submission.id);
      return response.data;
    } catch (saveFailure) {
      if (requestId !== saveRequestRef.current) return null;
      const message = saveFailure instanceof Error ? saveFailure.message : "Feedback draft could not be saved";
      setSaveError(message); setSaveState("error"); return null;
    }
  }, [assignment, dirty, editable, overall, scores, selectedFeedback?.version, selectedVersion, submission, user]);

  useEffect(() => {
    if (!dirty || saveState === "saving") return;
    const timer = window.setTimeout(() => { void save(); }, 900);
    return () => window.clearTimeout(timer);
  }, [dirty, save, saveState]);

  const release = async () => {
    if (!releaseOutcome || !submission || releasing) return;
    setReleasing(true);
    try {
      const feedback = dirty ? await save() : submission.feedback;
      if (!feedback || feedback.status !== "draft") throw new Error(saveError || "Save feedback before releasing it");
      releaseKeyRef.current ??= crypto.randomUUID();
      const response = await learningService.releaseFeedback(submission.id, releaseOutcome, feedback.version, releaseKeyRef.current);
      if (!response.success || !response.data) throw new Error(responseMessage(response, "Feedback could not be released"));
      applyReleased(response.data, releaseOutcome);
    } catch (releaseError) {
      const reconciled = await reconcileRelease(submission.id, releaseOutcome);
      if (reconciled) applyReleased(reconciled, releaseOutcome);
      else toast.error(releaseError instanceof Error ? releaseError.message : "Feedback could not be released");
    } finally { setReleasing(false); }
  };

  const applyReleased = (feedback: LearningFeedback, outcome: LearningReviewOutcome) => {
    setSubmission((current) => current ? { ...current, feedback, status: outcome } : current);
    setReleaseOutcome(null); releaseKeyRef.current = null;
    if (user) clearLearningRecovery(user.id, "review", submissionId);
    toast.success(outcome === "graded" ? "Feedback released" : "Revision requested");
  };

  const reconcileRelease = async (id: string, outcome: LearningReviewOutcome) => {
    const response = await learningService.submission(id);
    if (!response.success || !response.data?.feedback || response.data.feedback.status !== "released" || response.data.feedback.outcome !== outcome) return null;
    setSubmission(response.data);
    return response.data.feedback;
  };

  usePageChrome(submission ? `${submission.learner_name} submission` : "Submission");
  if (loading) return <LearningState busy title="Loading submission…" />;
  if (error) return <LearningState description={error} onRetry={() => void load()} title="Submission unavailable" />;
  if (!submission || !assignment || !selectedVersion) return <LearningState description="This submission does not exist or has no submitted version." title="Submission not found" />;
  const completeScores = assignment.rubric.every((criterion) => {
    const value = parseHundredths(scores[criterion.id] ?? "");
    return value !== null && value >= 0 && value <= criterion.max_score_hundredths;
  });

  return <div className="space-y-6">
    <Link className="text-sm font-medium text-[var(--text-muted)] hover:text-[var(--text-strong)]" params={{ assignmentId: assignment.id, spaceId: assignment.learning_space_id }} search={{ tab: "submissions", submission_page: 1, submission_status: "all" }} to="/modules/learning/spaces/$spaceId/assignments/$assignmentId">← {assignment.title}</Link>
    <section className="border border-[var(--border)] bg-[var(--surface)] p-5 sm:p-6"><div className="flex flex-wrap items-start justify-between gap-4"><div><h1 className="text-xl font-semibold text-[var(--text-strong)]">{submission.learner_name}</h1><p className="mt-1 font-tabular text-sm text-[var(--text-muted)]">{submission.learner_number}</p></div><LearningStatusBadge status={submission.status} /></div><div className="mt-5 flex flex-wrap gap-2" aria-label="Submission versions">{submission.versions.map((version) => <Button key={version.id} onClick={() => onVersionChange(version.id)} size="sm" variant={version.id === selectedVersion.id ? "primary" : "secondary"}>Version {version.revision_number}</Button>)}</div></section>
    <section className="border border-[var(--border)] bg-[var(--surface)] p-5 sm:p-6"><div className="flex flex-wrap items-center justify-between gap-3"><h2 className="text-lg font-semibold text-[var(--text-strong)]">Submitted work</h2><p className="text-xs text-[var(--text-muted)]">{formatLearningDateTime(selectedVersion.submitted_at)}{selectedVersion.late ? " · Late" : ""}</p></div><p className="mt-5 whitespace-pre-wrap text-sm leading-7 text-[var(--text-strong)]">{selectedVersion.body}</p></section>
    {editable ? <section className="border border-[var(--border)] bg-[var(--surface)]"><header className="border-b border-[var(--border)] p-5 sm:p-6"><h2 className="text-lg font-semibold text-[var(--text-strong)]">Feedback draft</h2><p className="mt-1 text-sm text-[var(--text-muted)]">Score the current version and save before release.</p></header><div className="space-y-5 p-5 sm:p-6">{assignment.rubric.map((criterion) => <div className="grid gap-3 border-b border-[var(--border)] pb-5 sm:grid-cols-[1fr_160px]" key={criterion.id}><div><p className="font-medium text-[var(--text-strong)]">{criterion.title}</p>{criterion.description ? <p className="mt-1 text-sm text-[var(--text-muted)]">{criterion.description}</p> : null}</div><div><Label htmlFor={`score-${criterion.id}`}>Score / {formatHundredths(criterion.max_score_hundredths)}</Label><Input className="mt-1.5" id={`score-${criterion.id}`} min={0} onChange={(event) => { setScores((current) => ({ ...current, [criterion.id]: event.target.value })); setSaveState("idle"); }} step="0.01" type="number" value={scores[criterion.id] ?? ""} /></div></div>)}<div><Label htmlFor="overall-feedback">Overall feedback</Label><Textarea className="mt-1.5 min-h-40" id="overall-feedback" maxLength={10000} onChange={(event) => { setOverall(event.target.value); setSaveState("idle"); }} value={overall} /></div><div className="flex flex-wrap items-center justify-between gap-3"><p aria-live="polite" className="text-xs text-[var(--text-muted)]">{saveState === "saving" ? "Saving…" : saveState === "error" ? saveError : dirty ? "Unsaved changes" : selectedFeedback ? "Saved" : "Add feedback to begin"}</p><div className="flex flex-wrap gap-2"><Button disabled={!dirty || saveState === "saving"} onClick={() => void save()} variant="secondary"><Save className="size-4" />Save feedback</Button><Button disabled={dirty || !selectedFeedback || saveState === "saving"} onClick={() => setReleaseOutcome("revision_requested")} variant="secondary">Request revision</Button><Button disabled={dirty || !selectedFeedback || !completeScores || saveState === "saving"} onClick={() => setReleaseOutcome("graded")}><Send className="size-4" />Release grade</Button></div></div></div></section> : selectedFeedback?.status === "released" ? <ReleasedFeedback assignment={assignment} feedback={selectedFeedback} /> : <LearningState description={selectedVersion.id === submission.current_submission_version_id ? "Feedback has not been released." : "Only the current submitted version can be reviewed."} title="No feedback available" />}
    <DialogShell onClose={releasing ? () => undefined : () => setReleaseOutcome(null)} open={Boolean(releaseOutcome)}><DialogHeader onClose={releasing ? undefined : () => setReleaseOutcome(null)} title={releaseOutcome === "graded" ? "Release graded feedback?" : "Request a revision?"} /><DialogBody><div className="flex gap-4"><span className="flex size-10 shrink-0 items-center justify-center rounded-[9px] bg-[var(--badge-info-bg)] text-[var(--badge-info-text)]"><CheckCircle2 className="size-5" /></span><p className="text-sm leading-6 text-[var(--text-muted)]">{releaseOutcome === "graded" ? "The rubric score and feedback become visible to the learner and this attempt is marked graded." : "The feedback becomes visible and the learner can prepare a new immutable version. Overall feedback is required."}</p></div></DialogBody><DialogFooter><Button disabled={releasing} onClick={() => setReleaseOutcome(null)} variant="secondary">Cancel</Button><Button disabled={releasing || (releaseOutcome === "revision_requested" && !overall.trim())} onClick={() => void release()}>{releasing ? <><Loader2 className="size-4 animate-spin" />Releasing…</> : releaseOutcome === "graded" ? "Release feedback" : "Request revision"}</Button></DialogFooter></DialogShell>
  </div>;
}

function ReleasedFeedback({ assignment, feedback }: { assignment: LearningAssignment; feedback: LearningFeedback }) {
  return <section className="border border-[var(--border)] bg-[var(--surface)] p-5 sm:p-6"><div className="flex flex-wrap items-center justify-between gap-3"><h2 className="text-lg font-semibold text-[var(--text-strong)]">Released feedback</h2><LearningStatusBadge status={feedback.outcome ?? "released"} /></div>{feedback.total_score_hundredths !== null ? <p className="mt-4 font-tabular text-2xl font-semibold text-[var(--text-strong)]">{formatHundredths(feedback.total_score_hundredths)} / {formatHundredths(assignment.max_score_hundredths)}</p> : null}{feedback.overall_feedback ? <p className="mt-4 whitespace-pre-wrap text-sm leading-7 text-[var(--text-muted)]">{feedback.overall_feedback}</p> : null}<div className="mt-5 divide-y divide-[var(--border)] border-y border-[var(--border)]">{assignment.rubric.map((criterion) => { const score = feedback.scores.find((item) => item.rubric_criterion_id === criterion.id); return <div className="flex items-center justify-between gap-4 py-3" key={criterion.id}><p className="text-sm font-medium text-[var(--text-strong)]">{criterion.title}</p><p className="font-tabular text-sm text-[var(--text-muted)]">{formatHundredths(score?.earned_score_hundredths)} / {formatHundredths(criterion.max_score_hundredths)}</p></div>; })}</div></section>;
}

function reviewFingerprint(overall: string, scores: Record<string, string>) { return JSON.stringify({ overall, scores: Object.entries(scores).sort(([left], [right]) => left.localeCompare(right)) }); }

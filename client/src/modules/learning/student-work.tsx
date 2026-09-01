import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { CheckCircle2, Loader2, Save, Send } from "lucide-react";
import toast from "react-hot-toast";

import { Button } from "@/components/ui/button";
import { DialogBody, DialogFooter, DialogHeader, DialogShell } from "@/components/ui/dialog";
import { Textarea } from "@/components/ui/input";
import { useAuthStore } from "@/stores/auth-store";

import {
  clearLearningRecovery,
  purgeLearningRecoveryForOtherUsers,
  readLearningRecovery,
  writeLearningRecovery,
} from "./draft-recovery";
import { learningService, responseMessage } from "./service";
import type { LearningAssignment, LearningSubmission } from "./types";
import { formatHundredths, formatLearningDateTime, LearningState, LearningStatusBadge } from "./ui";

export function StudentWork({ assignment }: { assignment: LearningAssignment }) {
  const user = useAuthStore((state) => state.user);
  const [submission, setSubmission] = useState<LearningSubmission | null>(null);
  const [body, setBody] = useState("");
  const [savedBody, setSavedBody] = useState("");
  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [saveState, setSaveState] = useState<"idle" | "saving" | "saved" | "error">("idle");
  const [saveError, setSaveError] = useState<string | null>(null);
  const [submitOpen, setSubmitOpen] = useState(false);
  const [submitting, setSubmitting] = useState(false);
  const loadRequestRef = useRef(0);
  const saveRequestRef = useRef(0);
  const submitKeyRef = useRef<string | null>(null);

  const applySubmission = useCallback((next: LearningSubmission | null) => {
    setSubmission(next);
    const serverBody = next?.draft_body ?? "";
    const recovery = user ? readLearningRecovery(user.id, "submission", assignment.id) : null;
    const useRecovery = recovery && (!next || recovery.savedAt > new Date(next.updated_at).getTime());
    const nextBody = useRecovery ? recovery.body : serverBody;
    setBody(nextBody);
    setSavedBody(serverBody);
    setSaveState(nextBody === serverBody ? "saved" : "idle");
  }, [assignment.id, user]);

  const load = useCallback(async () => {
    const requestId = ++loadRequestRef.current;
    setLoading(true);
    setLoadError(null);
    try {
      const response = await learningService.mySubmission(assignment.id);
      if (!response.success && !responseLooksMissing(response)) {
        throw new Error(responseMessage(response, "Your work could not be loaded"));
      }
      if (requestId !== loadRequestRef.current) return;
      applySubmission(response.data ?? null);
    } catch (error) {
      if (requestId !== loadRequestRef.current) return;
      setLoadError(error instanceof Error ? error.message : "Your work could not be loaded");
    } finally {
      if (requestId === loadRequestRef.current) setLoading(false);
    }
  }, [applySubmission, assignment.id]);

  useEffect(() => {
    if (user) purgeLearningRecoveryForOtherUsers(user.id);
    void load();
    return () => { loadRequestRef.current += 1; saveRequestRef.current += 1; };
  }, [load, user]);

  useEffect(() => {
    if (!user) return;
    const currentUserId = user.id;
    return () => {
      if (useAuthStore.getState().user?.id !== currentUserId) {
        clearLearningRecovery(currentUserId, "submission", assignment.id);
      }
    };
  }, [assignment.id, user]);

  const editable = assignment.status === "published" && (!submission || ["draft", "revision_requested"].includes(submission.status));
  const dirty = editable && body !== savedBody;

  useEffect(() => {
    if (!user || !dirty) return;
    writeLearningRecovery(user.id, "submission", assignment.id, body);
    const warn = (event: BeforeUnloadEvent) => { event.preventDefault(); event.returnValue = ""; };
    window.addEventListener("beforeunload", warn);
    return () => window.removeEventListener("beforeunload", warn);
  }, [assignment.id, body, dirty, user]);

  const save = useCallback(async (bodyToSave: string) => {
    if (!editable || bodyToSave === savedBody) return submission;
    const requestId = ++saveRequestRef.current;
    setSaveState("saving");
    setSaveError(null);
    try {
      const response = await learningService.saveSubmission(assignment.id, bodyToSave, submission?.version ?? null);
      if (!response.success || !response.data) throw new Error(responseMessage(response, "Your draft could not be saved"));
      if (requestId !== saveRequestRef.current) return response.data;
      setSubmission(response.data);
      setSavedBody(response.data.draft_body ?? "");
      setSaveState("saved");
      if (user && (response.data.draft_body ?? "") === bodyToSave) clearLearningRecovery(user.id, "submission", assignment.id);
      return response.data;
    } catch (error) {
      if (requestId !== saveRequestRef.current) return null;
      const message = error instanceof Error ? error.message : "Your draft could not be saved";
      setSaveError(message);
      setSaveState("error");
      return null;
    }
  }, [assignment.id, editable, savedBody, submission, user]);

  useEffect(() => {
    if (!dirty || saveState === "saving") return;
    const timer = window.setTimeout(() => { void save(body); }, 900);
    return () => window.clearTimeout(timer);
  }, [body, dirty, save, saveState]);

  const submit = async () => {
    if (submitting || !body.trim()) return;
    setSubmitting(true);
    try {
      const saved = dirty ? await save(body) : submission;
      if (!saved || (saved.draft_body ?? "") !== body) throw new Error(saveError || "Save your response before submitting");
      submitKeyRef.current ??= crypto.randomUUID();
      const response = await learningService.submitSubmission(assignment.id, saved.version, submitKeyRef.current);
      if (!response.success || !response.data) throw new Error(responseMessage(response, "Your work could not be submitted"));
      applySubmitted(response.data);
    } catch (error) {
      const reconciled = await reconcileSubmitted(assignment.id, body);
      if (reconciled) applySubmitted(reconciled);
      else toast.error(error instanceof Error ? error.message : "Your work could not be submitted");
    } finally {
      setSubmitting(false);
    }
  };

  const applySubmitted = (next: LearningSubmission) => {
    setSubmission(next);
    setBody(next.draft_body ?? body);
    setSavedBody(next.draft_body ?? body);
    setSubmitOpen(false);
    submitKeyRef.current = null;
    if (user) clearLearningRecovery(user.id, "submission", assignment.id);
    toast.success(next.versions.length > 1 ? "Revision submitted" : "Work submitted");
  };

  const reconcileSubmitted = async (assignmentId: string, submittedBody: string) => {
    const response = await learningService.mySubmission(assignmentId);
    if (!response.success || !response.data || response.data.status !== "submitted") return null;
    const latest = response.data.versions[response.data.versions.length - 1];
    return latest?.body === submittedBody ? response.data : null;
  };

  const latestVersion = submission?.versions[submission.versions.length - 1];
  const releasedFeedback = submission?.feedback?.status === "released" ? submission.feedback : null;

  if (loading) return <LearningState busy title="Loading your work…" />;
  if (loadError) return <LearningState description={loadError} onRetry={() => void load()} title="Your work is unavailable" />;

  return <div className="space-y-6">
    <section className="border border-[var(--border)] bg-[var(--surface)]">
      <header className="flex flex-wrap items-start justify-between gap-4 border-b border-[var(--border)] p-5 sm:p-6">
        <div><h2 className="text-lg font-semibold text-[var(--text-strong)]">My work</h2><p className="mt-1 text-sm text-[var(--text-muted)]">Text response · due {formatLearningDateTime(assignment.due_at)}</p></div>
        {submission ? <LearningStatusBadge status={submission.status} /> : <LearningStatusBadge status="not_started" />}
      </header>
      {editable ? <div className="p-5 sm:p-6">
        {submission?.status === "revision_requested" && releasedFeedback?.overall_feedback ? <div className="mb-5 border-l-4 border-[var(--brand-strong)] bg-[var(--badge-info-bg)] p-4 text-sm text-[var(--badge-info-text)]"><span className="font-semibold">Revision requested:</span> {releasedFeedback.overall_feedback}</div> : null}
        <Textarea aria-label="Assignment response" className="min-h-[360px] resize-y" maxLength={20000} onChange={(event) => { setBody(event.target.value); setSaveState("idle"); }} placeholder="Write your response" value={body} />
        <div className="mt-4 flex flex-wrap items-center justify-between gap-3">
          <div aria-live="polite" className="text-xs text-[var(--text-muted)]">{saveState === "saving" ? "Saving…" : saveState === "error" ? saveError : dirty ? "Unsaved changes" : submission ? "Saved" : "Start writing to create a draft"}</div>
          <div className="flex flex-wrap gap-2"><Button disabled={!dirty || saveState === "saving"} onClick={() => void save(body)} type="button" variant="secondary"><Save className="size-4" />Save draft</Button><Button disabled={!body.trim() || dirty || saveState === "saving"} onClick={() => setSubmitOpen(true)} type="button"><Send className="size-4" />{submission?.versions.length ? "Submit revision" : "Submit work"}</Button></div>
        </div>
      </div> : latestVersion ? <article className="p-5 sm:p-6"><p className="whitespace-pre-wrap text-sm leading-7 text-[var(--text-strong)]">{latestVersion.body}</p><p className="mt-5 text-xs text-[var(--text-muted)]">Version {latestVersion.revision_number} submitted {formatLearningDateTime(latestVersion.submitted_at)}{latestVersion.late ? " · Late" : ""}</p></article> : <div className="p-5 text-sm text-[var(--text-muted)]">This assignment is not accepting responses.</div>}
    </section>

    {releasedFeedback ? <section className="border border-[var(--border)] bg-[var(--surface)] p-5 sm:p-6"><div className="flex flex-wrap items-center justify-between gap-3"><h2 className="text-lg font-semibold text-[var(--text-strong)]">Feedback</h2><LearningStatusBadge status={releasedFeedback.outcome ?? "released"} /></div>{releasedFeedback.total_score_hundredths !== null ? <p className="mt-4 font-tabular text-2xl font-semibold text-[var(--text-strong)]">{formatHundredths(releasedFeedback.total_score_hundredths)} / {formatHundredths(assignment.max_score_hundredths)}</p> : null}{releasedFeedback.overall_feedback ? <p className="mt-4 whitespace-pre-wrap text-sm leading-7 text-[var(--text-muted)]">{releasedFeedback.overall_feedback}</p> : null}</section> : null}

    {submission?.versions.length ? <section><h2 className="text-lg font-semibold text-[var(--text-strong)]">Submission history</h2><div className="mt-3 divide-y divide-[var(--border)] border border-[var(--border)] bg-[var(--surface)]">{[...submission.versions].reverse().map((version) => <article className="p-4 sm:p-5" key={version.id}><div className="flex flex-wrap items-center justify-between gap-3"><p className="font-medium text-[var(--text-strong)]">Version {version.revision_number}</p><p className="text-xs text-[var(--text-muted)]">{formatLearningDateTime(version.submitted_at)}{version.late ? " · Late" : ""}</p></div><p className="mt-3 line-clamp-3 whitespace-pre-wrap text-sm leading-6 text-[var(--text-muted)]">{version.body}</p></article>)}</div></section> : null}

    <DialogShell onClose={submitting ? () => undefined : () => setSubmitOpen(false)} open={submitOpen}>
      <DialogHeader onClose={submitting ? undefined : () => setSubmitOpen(false)} title={submission?.versions.length ? "Submit revision?" : "Submit work?"} />
      <DialogBody><div className="flex gap-4"><span className="flex size-10 shrink-0 items-center justify-center rounded-[9px] bg-[var(--badge-info-bg)] text-[var(--badge-info-text)]"><CheckCircle2 className="size-5" /></span><div><p className="text-sm leading-6 text-[var(--text-muted)]">This creates immutable version {(submission?.versions.length ?? 0) + 1}. You cannot edit it unless feedback requests another revision.</p><p className="mt-2 text-sm font-medium text-[var(--text-strong)]">Due {formatLearningDateTime(assignment.due_at)}</p></div></div></DialogBody>
      <DialogFooter><Button disabled={submitting} onClick={() => setSubmitOpen(false)} type="button" variant="secondary">Keep editing</Button><Button disabled={submitting} onClick={() => void submit()} type="button">{submitting ? <><Loader2 className="size-4 animate-spin" />Submitting…</> : submission?.versions.length ? "Submit revision" : "Submit work"}</Button></DialogFooter>
    </DialogShell>
  </div>;
}

function responseLooksMissing(response: { issues: Array<string | { detail?: string }> | null; message: string | null }) {
  const text = [response.message, ...(response.issues ?? []).map((issue) => typeof issue === "string" ? issue : issue.detail)].filter(Boolean).join(" ").toLowerCase();
  return text.includes("not found") || text.includes("no submission");
}

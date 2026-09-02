/** Learner draft, governed file, submission, and immutable history workspace. */

import { useCallback, useEffect, useRef, useState } from "react";
import {
  CheckCircle2,
  Download,
  FileText,
  Loader2,
  Save,
  Send,
  Trash2,
  Upload,
} from "lucide-react";
import toast from "react-hot-toast";

import { Button } from "@/components/ui/button";
import { ConfirmDrawer } from "@/components/ui/confirm-drawer";
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
import type {
  LearningAssignment,
  LearningSubmission,
  LearningSubmissionFile,
  LearningSubmissionMethod,
} from "./types";
import { formatHundredths, formatLearningDateTime, LearningState, LearningStatusBadge } from "./ui";

const MAX_SUBMISSION_FILE_BYTES = 15 * 1024 * 1024;
const SUBMISSION_FILE_TYPES = ["application/pdf", "image/jpeg", "image/png"];

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
  const [uploading, setUploading] = useState(false);
  const [fileToRemove, setFileToRemove] = useState<LearningSubmissionFile | null>(null);
  const [removing, setRemoving] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);
  const loadRequestRef = useRef(0);
  const saveRequestRef = useRef(0);
  const submitKeyRef = useRef<string | null>(null);

  const applySubmission = useCallback(
    (next: LearningSubmission | null) => {
      setSubmission(next);
      const serverBody = next?.draft_body ?? "";
      const recovery = user ? readLearningRecovery(user.id, "submission", assignment.id) : null;
      const useRecovery = recovery && (!next || recovery.savedAt > new Date(next.updated_at).getTime());
      const nextBody = useRecovery ? recovery.body : serverBody;
      setBody(nextBody);
      setSavedBody(serverBody);
      setSaveState(nextBody === serverBody ? "saved" : "idle");
    },
    [assignment.id, user],
  );

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
    } catch (cause) {
      if (requestId !== loadRequestRef.current) return;
      setLoadError(cause instanceof Error ? cause.message : "Your work could not be loaded");
    } finally {
      if (requestId === loadRequestRef.current) setLoading(false);
    }
  }, [applySubmission, assignment.id]);

  useEffect(() => {
    if (user) purgeLearningRecoveryForOtherUsers(user.id);
    void load();
    return () => {
      loadRequestRef.current += 1;
      saveRequestRef.current += 1;
    };
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

  const editable =
    assignment.status === "published" &&
    (!submission || ["draft", "revision_requested"].includes(submission.status));
  const acceptsText = assignment.submission_method !== "file";
  const acceptsFiles = assignment.submission_method !== "text";
  const dirty = editable && body !== savedBody;

  useEffect(() => {
    if (!user || !dirty) return;
    writeLearningRecovery(user.id, "submission", assignment.id, body);
    const warn = (event: BeforeUnloadEvent) => {
      event.preventDefault();
      event.returnValue = "";
    };
    window.addEventListener("beforeunload", warn);
    return () => window.removeEventListener("beforeunload", warn);
  }, [assignment.id, body, dirty, user]);

  const save = useCallback(
    async (bodyToSave: string) => {
      if (!editable || bodyToSave === savedBody) return submission;
      const requestId = ++saveRequestRef.current;
      setSaveState("saving");
      setSaveError(null);
      try {
        const response = await learningService.saveSubmission(
          assignment.id,
          bodyToSave,
          submission?.version ?? null,
        );
        if (!response.success || !response.data) {
          throw new Error(responseMessage(response, "Your draft could not be saved"));
        }
        if (requestId !== saveRequestRef.current) return response.data;
        setSubmission(response.data);
        setSavedBody(response.data.draft_body ?? "");
        setSaveState("saved");
        if (user && (response.data.draft_body ?? "") === bodyToSave) {
          clearLearningRecovery(user.id, "submission", assignment.id);
        }
        return response.data;
      } catch (cause) {
        if (requestId !== saveRequestRef.current) return null;
        const message = cause instanceof Error ? cause.message : "Your draft could not be saved";
        setSaveError(message);
        setSaveState("error");
        return null;
      }
    },
    [assignment.id, editable, savedBody, submission, user],
  );

  useEffect(() => {
    if (!dirty || saveState === "saving") return;
    const timer = window.setTimeout(() => {
      void save(body);
    }, 900);
    return () => window.clearTimeout(timer);
  }, [body, dirty, save, saveState]);

  const upload = async (file: File) => {
    if (!editable || uploading) return;
    if (!SUBMISSION_FILE_TYPES.includes(file.type)) {
      toast.error("Choose a PDF, JPEG, or PNG file");
      return;
    }
    if (file.size > MAX_SUBMISSION_FILE_BYTES) {
      toast.error("Submission files must be 15 MB or smaller");
      return;
    }
    if ((submission?.draft_files.length ?? 0) >= 5) {
      toast.error("Remove a file before uploading another");
      return;
    }
    setUploading(true);
    try {
      const response = await learningService.uploadSubmissionFile(
        assignment.id,
        file,
        submission?.version ?? null,
      );
      if (!response.success || !response.data) {
        throw new Error(responseMessage(response, "The file could not be uploaded"));
      }
      setSubmission(response.data);
      setSavedBody(response.data.draft_body ?? "");
      toast.success("File added");
    } catch (cause) {
      toast.error(cause instanceof Error ? cause.message : "The file could not be uploaded");
    } finally {
      setUploading(false);
      if (inputRef.current) inputRef.current.value = "";
    }
  };

  const removeFile = async () => {
    if (!fileToRemove || !submission || fileToRemove.version === null || removing) return;
    setRemoving(true);
    try {
      const response = await learningService.removeSubmissionFile(
        assignment.id,
        fileToRemove.id,
        submission.version,
        fileToRemove.version,
      );
      if (!response.success || !response.data) {
        throw new Error(responseMessage(response, "The file could not be removed"));
      }
      setSubmission(response.data);
      setSavedBody(response.data.draft_body ?? "");
      setFileToRemove(null);
      toast.success("File removed from draft");
    } catch (cause) {
      toast.error(cause instanceof Error ? cause.message : "The file could not be removed");
    } finally {
      setRemoving(false);
    }
  };

  const submit = async () => {
    if (submitting || !submissionReady(assignment.submission_method, body, submission?.draft_files)) {
      return;
    }
    setSubmitting(true);
    const draftFileIds = (submission?.draft_files ?? []).map((file) => file.document_file_id);
    try {
      const saved = dirty ? await save(body) : submission;
      if (!saved || (saved.draft_body ?? "") !== body) {
        throw new Error(saveError || "Save your response before submitting");
      }
      submitKeyRef.current ??= crypto.randomUUID();
      const response = await learningService.submitSubmission(
        assignment.id,
        saved.version,
        submitKeyRef.current,
      );
      if (!response.success || !response.data) {
        throw new Error(responseMessage(response, "Your work could not be submitted"));
      }
      applySubmitted(response.data);
    } catch (cause) {
      const reconciled = await reconcileSubmitted(assignment.id, body, draftFileIds);
      if (reconciled) applySubmitted(reconciled);
      else toast.error(cause instanceof Error ? cause.message : "Your work could not be submitted");
    } finally {
      setSubmitting(false);
    }
  };

  const applySubmitted = (next: LearningSubmission) => {
    const latest = next.versions[next.versions.length - 1];
    const finalBody = next.draft_body ?? latest?.body ?? body;
    setSubmission(next);
    setBody(finalBody);
    setSavedBody(finalBody);
    setSubmitOpen(false);
    submitKeyRef.current = null;
    if (user) clearLearningRecovery(user.id, "submission", assignment.id);
    toast.success(next.versions.length > 1 ? "Revision submitted" : "Work submitted");
  };

  const reconcileSubmitted = async (
    assignmentId: string,
    submittedBody: string,
    documentFileIds: string[],
  ) => {
    const response = await learningService.mySubmission(assignmentId);
    if (!response.success || !response.data || response.data.status !== "submitted") return null;
    const latest = response.data.versions[response.data.versions.length - 1];
    if ((latest?.body ?? "") !== submittedBody) return null;
    const submittedFileIds = (latest?.files ?? [])
      .map((file) => file.document_file_id)
      .sort();
    const expectedFileIds = [...documentFileIds].sort();
    return JSON.stringify(submittedFileIds) === JSON.stringify(expectedFileIds)
      ? response.data
      : null;
  };

  const latestVersion = submission?.versions[submission.versions.length - 1];
  const releasedFeedback = submission?.feedback?.status === "released" ? submission.feedback : null;
  const ready = submissionReady(assignment.submission_method, body, submission?.draft_files);

  if (loading) return <LearningState busy title="Loading your work…" />;
  if (loadError) {
    return (
      <LearningState
        description={loadError}
        onRetry={() => void load()}
        title="Your work is unavailable"
      />
    );
  }

  return (
    <div className="space-y-6">
      <section className="border border-[var(--border)] bg-[var(--surface)]">
        <header className="flex flex-wrap items-start justify-between gap-4 border-b border-[var(--border)] p-5 sm:p-6">
          <div>
            <h2 className="text-lg font-semibold text-[var(--text-strong)]">My work</h2>
            <p className="mt-1 text-sm text-[var(--text-muted)]">
              {submissionMethodLabel(assignment.submission_method)} · due {formatLearningDateTime(assignment.due_at)}
            </p>
          </div>
          {submission ? (
            <LearningStatusBadge status={submission.status} />
          ) : (
            <LearningStatusBadge status="not_started" />
          )}
        </header>
        {editable ? (
          <div className="space-y-6 p-5 sm:p-6">
            {submission?.status === "revision_requested" && releasedFeedback?.overall_feedback ? (
              <div className="border-l-4 border-[var(--brand-strong)] bg-[var(--badge-info-bg)] p-4 text-sm text-[var(--badge-info-text)]">
                <span className="font-semibold">Revision requested:</span>{" "}
                {releasedFeedback.overall_feedback}
              </div>
            ) : null}
            {acceptsText ? (
              <div>
                <Textarea
                  aria-label={assignment.submission_method === "text" ? "Assignment response" : "Assignment notes"}
                  className="min-h-[280px] resize-y"
                  maxLength={20000}
                  onChange={(event) => {
                    setBody(event.target.value);
                    setSaveState("idle");
                  }}
                  placeholder={assignment.submission_method === "text" ? "Write your response" : "Add a response or notes"}
                  value={body}
                />
                <div aria-live="polite" className="mt-2 text-xs text-[var(--text-muted)]">
                  {saveState === "saving"
                    ? "Saving…"
                    : saveState === "error"
                      ? saveError
                      : dirty
                        ? "Unsaved changes"
                        : submission
                          ? "Saved"
                          : "Start writing to create a draft"}
                </div>
              </div>
            ) : null}
            {acceptsFiles ? (
              <div className="space-y-3">
                <div className="flex flex-wrap items-center justify-between gap-3">
                  <div>
                    <h3 className="font-semibold text-[var(--text-strong)]">Files</h3>
                    <p className="mt-1 text-xs text-[var(--text-muted)]">
                      PDF, JPEG, or PNG · 15 MB each · up to 5 files
                    </p>
                  </div>
                  <input
                    accept="application/pdf,image/jpeg,image/png"
                    className="sr-only"
                    onChange={(event) => {
                      const file = event.target.files?.[0];
                      if (file) void upload(file);
                    }}
                    ref={inputRef}
                    type="file"
                  />
                  <Button
                    disabled={uploading || (submission?.draft_files.length ?? 0) >= 5}
                    onClick={() => inputRef.current?.click()}
                    type="button"
                    variant="secondary"
                  >
                    {uploading ? <Loader2 className="size-4 animate-spin" /> : <Upload className="size-4" />}
                    {uploading ? "Uploading…" : "Add file"}
                  </Button>
                </div>
                {submission?.draft_files.length ? (
                  <FileList
                    files={submission.draft_files}
                    onDownload={(file) => void downloadFile(file)}
                    onRemove={setFileToRemove}
                  />
                ) : (
                  <p className="border border-dashed border-[var(--border)] p-4 text-sm text-[var(--text-muted)]">
                    No files added.
                  </p>
                )}
              </div>
            ) : null}
            <div className="flex flex-wrap items-center justify-end gap-2">
              {acceptsText ? (
                <Button
                  disabled={!dirty || saveState === "saving"}
                  onClick={() => void save(body)}
                  type="button"
                  variant="secondary"
                >
                  <Save className="size-4" />
                  Save draft
                </Button>
              ) : null}
              <Button
                disabled={!ready || dirty || saveState === "saving" || uploading}
                onClick={() => setSubmitOpen(true)}
                type="button"
              >
                <Send className="size-4" />
                {submission?.versions.length ? "Submit revision" : "Submit work"}
              </Button>
            </div>
          </div>
        ) : latestVersion ? (
          <article className="space-y-4 p-5 sm:p-6">
            {latestVersion.body ? (
              <p className="whitespace-pre-wrap text-sm leading-7 text-[var(--text-strong)]">
                {latestVersion.body}
              </p>
            ) : null}
            {latestVersion.files.length ? (
              <FileList files={latestVersion.files} onDownload={(file) => void downloadFile(file)} />
            ) : null}
            <p className="text-xs text-[var(--text-muted)]">
              Version {latestVersion.revision_number} submitted{" "}
              {formatLearningDateTime(latestVersion.submitted_at)}
              {latestVersion.late ? " · Late" : ""}
            </p>
          </article>
        ) : (
          <div className="p-5 text-sm text-[var(--text-muted)]">
            This assignment is not accepting responses.
          </div>
        )}
      </section>

      {releasedFeedback ? (
        <section className="border border-[var(--border)] bg-[var(--surface)] p-5 sm:p-6">
          <div className="flex flex-wrap items-center justify-between gap-3">
            <h2 className="text-lg font-semibold text-[var(--text-strong)]">Feedback</h2>
            <LearningStatusBadge status={releasedFeedback.outcome ?? "released"} />
          </div>
          {releasedFeedback.total_score_hundredths !== null ? (
            <p className="mt-4 font-tabular text-2xl font-semibold text-[var(--text-strong)]">
              {formatHundredths(releasedFeedback.total_score_hundredths)} /{" "}
              {formatHundredths(assignment.max_score_hundredths)}
            </p>
          ) : null}
          {releasedFeedback.overall_feedback ? (
            <p className="mt-4 whitespace-pre-wrap text-sm leading-7 text-[var(--text-muted)]">
              {releasedFeedback.overall_feedback}
            </p>
          ) : null}
        </section>
      ) : null}

      {submission?.versions.length ? (
        <section>
          <h2 className="text-lg font-semibold text-[var(--text-strong)]">Submission history</h2>
          <div className="mt-3 divide-y divide-[var(--border)] border border-[var(--border)] bg-[var(--surface)]">
            {[...submission.versions].reverse().map((version) => (
              <article className="space-y-3 p-4 sm:p-5" key={version.id}>
                <div className="flex flex-wrap items-center justify-between gap-3">
                  <p className="font-medium text-[var(--text-strong)]">
                    Version {version.revision_number}
                  </p>
                  <p className="text-xs text-[var(--text-muted)]">
                    {formatLearningDateTime(version.submitted_at)}
                    {version.late ? " · Late" : ""}
                  </p>
                </div>
                {version.body ? (
                  <p className="line-clamp-3 whitespace-pre-wrap text-sm leading-6 text-[var(--text-muted)]">
                    {version.body}
                  </p>
                ) : null}
                {version.files.length ? (
                  <FileList files={version.files} onDownload={(file) => void downloadFile(file)} />
                ) : null}
              </article>
            ))}
          </div>
        </section>
      ) : null}

      <DialogShell onClose={submitting ? () => undefined : () => setSubmitOpen(false)} open={submitOpen}>
        <DialogHeader
          onClose={submitting ? undefined : () => setSubmitOpen(false)}
          title={submission?.versions.length ? "Submit revision?" : "Submit work?"}
        />
        <DialogBody>
          <div className="flex gap-4">
            <span className="flex size-10 shrink-0 items-center justify-center rounded-[9px] bg-[var(--badge-info-bg)] text-[var(--badge-info-text)]">
              <CheckCircle2 className="size-5" />
            </span>
            <div>
              <p className="text-sm leading-6 text-[var(--text-muted)]">
                This creates immutable version {(submission?.versions.length ?? 0) + 1}. You
                cannot edit it unless feedback requests another revision.
              </p>
              <p className="mt-2 text-sm font-medium text-[var(--text-strong)]">
                Due {formatLearningDateTime(assignment.due_at)}
              </p>
            </div>
          </div>
        </DialogBody>
        <DialogFooter>
          <Button disabled={submitting} onClick={() => setSubmitOpen(false)} type="button" variant="secondary">
            Keep editing
          </Button>
          <Button disabled={submitting} onClick={() => void submit()} type="button">
            {submitting ? <Loader2 className="size-4 animate-spin" /> : null}
            {submitting
              ? "Submitting…"
              : submission?.versions.length
                ? "Submit revision"
                : "Submit work"}
          </Button>
        </DialogFooter>
      </DialogShell>

      <ConfirmDrawer
        cancelLabel="Keep file"
        confirmLabel="Remove file"
        description={
          fileToRemove
            ? `${fileToRemove.original_file_name} will be removed from this draft. Previously submitted versions are unchanged.`
            : "The file will be removed from this draft."
        }
        isPending={removing}
        onClose={() => setFileToRemove(null)}
        onConfirm={() => void removeFile()}
        open={Boolean(fileToRemove)}
        pendingLabel="Removing…"
        title="Remove draft file?"
      />
    </div>
  );
}

function FileList({
  files,
  onDownload,
  onRemove,
}: {
  files: LearningSubmissionFile[];
  onDownload: (file: LearningSubmissionFile) => void;
  onRemove?: (file: LearningSubmissionFile) => void;
}) {
  return (
    <div className="divide-y divide-[var(--border)] border border-[var(--border)]">
      {files.map((file) => (
        <div className="flex items-center gap-3 p-3" key={file.id}>
          <FileText className="size-5 shrink-0 text-[var(--brand-strong)]" />
          <div className="min-w-0 flex-1">
            <p className="truncate text-sm font-medium text-[var(--text-strong)]">
              {file.original_file_name}
            </p>
            <p className="mt-0.5 text-xs text-[var(--text-muted)]">
              {formatFileSize(file.byte_size)} · {file.document_reference}
            </p>
          </div>
          <Button
            aria-label={`Download ${file.original_file_name}`}
            onClick={() => onDownload(file)}
            size="icon-sm"
            type="button"
            variant="ghost"
          >
            <Download className="size-4" />
          </Button>
          {onRemove ? (
            <Button
              aria-label={`Remove ${file.original_file_name}`}
              onClick={() => onRemove(file)}
              size="icon-sm"
              type="button"
              variant="ghost"
            >
              <Trash2 className="size-4" />
            </Button>
          ) : null}
        </div>
      ))}
    </div>
  );
}

async function downloadFile(file: LearningSubmissionFile) {
  try {
    const response = await learningService.downloadSubmissionFile(file.id);
    if (!response.success || !response.data) {
      throw new Error(responseMessage(response, "The file could not be downloaded"));
    }
    window.open(response.data.url, "_blank", "noopener,noreferrer");
  } catch (cause) {
    toast.error(cause instanceof Error ? cause.message : "The file could not be downloaded");
  }
}

function submissionReady(
  method: LearningSubmissionMethod,
  body: string,
  files: LearningSubmissionFile[] | undefined,
) {
  const hasText = Boolean(body.trim());
  const hasFile = Boolean(files?.length);
  if (method === "text") return hasText;
  if (method === "file") return hasFile;
  return hasText || hasFile;
}

function submissionMethodLabel(method: LearningSubmissionMethod) {
  if (method === "text") return "Text response";
  if (method === "file") return "File response";
  return "Text or file response";
}

function formatFileSize(bytes: number) {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${Math.round(bytes / 1024)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

function responseLooksMissing(response: {
  issues: Array<string | { detail?: string }> | null;
  message: string | null;
}) {
  const text = [
    response.message,
    ...(response.issues ?? []).map((issue) => (typeof issue === "string" ? issue : issue.detail)),
  ]
    .filter(Boolean)
    .join(" ")
    .toLowerCase();
  return text.includes("not found") || text.includes("no submission");
}

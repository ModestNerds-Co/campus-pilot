import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Link } from "@tanstack/react-router";
import { CheckCircle2, Edit3, Loader2, Plus, Send, Trash2 } from "lucide-react";
import toast from "react-hot-toast";

import { Button } from "@/components/ui/button";
import { ConfirmDrawer } from "@/components/ui/confirm-drawer";
import { DialogBody, DialogFooter, DialogHeader, DialogShell } from "@/components/ui/dialog";
import { Input, Label, Textarea } from "@/components/ui/input";
import { usePageChrome } from "@/modules/admin/layouts/page-chrome";
import { useAuthStore } from "@/stores/auth-store";

import { AssignmentSubmissions, type SubmissionListSearchState } from "./submissions-list";
import { GuardedDrawer } from "./guarded-drawer";
import { learningService, responseMessage } from "./service";
import { StudentWork } from "./student-work";
import type { LearningAssignment, LearningRubricCriterion, LearningSpace } from "./types";
import { formatHundredths, formatLearningDateTime, LearningState, LearningStatusBadge, parseHundredths } from "./ui";

export type LearningAssignmentTab = "brief" | "rubric" | "work" | "submissions";
export interface LearningAssignmentDetailSearchState {
  submission_page: number;
  submission_status: SubmissionListSearchState["status"];
  tab: LearningAssignmentTab;
}

export function LearningAssignmentWorkspace({ assignmentId, onSearchChange, search, spaceId }: {
  assignmentId: string;
  onSearchChange: (next: LearningAssignmentDetailSearchState, options?: { replace?: boolean }) => void;
  search: LearningAssignmentDetailSearchState;
  spaceId: string;
}) {
  const permissions = useAuthStore((state) => state.user?.permissions ?? []);
  const canTeach = permissions.includes("*") || permissions.includes("learning:teach");
  const canParticipate = permissions.includes("*") || permissions.includes("learning:participate");
  const [assignment, setAssignment] = useState<LearningAssignment | null>(null);
  const [space, setSpace] = useState<LearningSpace | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [editOpen, setEditOpen] = useState(false);
  const [criterion, setCriterion] = useState<LearningRubricCriterion | "new" | null>(null);
  const [deleteCriterion, setDeleteCriterion] = useState<LearningRubricCriterion | null>(null);
  const [transition, setTransition] = useState<"publish" | "close" | null>(null);
  const [pending, setPending] = useState(false);
  const requestRef = useRef(0);

  const load = useCallback(async () => {
    const requestId = ++requestRef.current;
    setLoading(true);
    setError(null);
    try {
      const [assignmentResponse, spaceResponse] = await Promise.all([
        learningService.assignment(assignmentId),
        learningService.space(spaceId),
      ]);
      if (!assignmentResponse.success || !assignmentResponse.data) throw new Error(responseMessage(assignmentResponse, "Assignment could not be loaded"));
      if (!spaceResponse.success || !spaceResponse.data) throw new Error(responseMessage(spaceResponse, "Learning space could not be loaded"));
      if (requestId !== requestRef.current) return;
      setAssignment(assignmentResponse.data);
      setSpace(spaceResponse.data);
    } catch (loadError) {
      if (requestId !== requestRef.current) return;
      setError(loadError instanceof Error ? loadError.message : "Assignment could not be loaded");
    } finally {
      if (requestId === requestRef.current) setLoading(false);
    }
  }, [assignmentId, spaceId]);

  useEffect(() => {
    void load();
    return () => { requestRef.current += 1; };
  }, [load]);

  const allowedTabs = useMemo(() => [
    "brief" as const,
    "rubric" as const,
    ...(canParticipate ? ["work" as const] : []),
    ...(canTeach ? ["submissions" as const] : []),
  ], [canParticipate, canTeach]);
  useEffect(() => {
    if (!allowedTabs.includes(search.tab)) onSearchChange({ ...search, tab: allowedTabs[0] }, { replace: true });
  }, [allowedTabs, onSearchChange, search]);

  const rubricTotal = assignment?.rubric.reduce((sum, item) => sum + item.max_score_hundredths, 0) ?? 0;
  const publishReady = Boolean(assignment && assignment.rubric.length > 0 && rubricTotal === assignment.max_score_hundredths);
  usePageChrome(assignment?.title ?? "Assignment", assignment && canTeach ? <div className="flex flex-wrap gap-2">{assignment.status === "draft" ? <><Button onClick={() => setEditOpen(true)} variant="secondary"><Edit3 className="size-4" />Edit</Button><Button disabled={!publishReady} onClick={() => setTransition("publish")}><Send className="size-4" />Publish</Button></> : assignment.status === "published" ? <Button onClick={() => setTransition("close")} variant="secondary">Close assignment</Button> : null}</div> : null);

  if (loading) return <LearningState busy title="Loading assignment…" />;
  if (error) return <LearningState description={error} onRetry={() => void load()} title="Assignment unavailable" />;
  if (!assignment || !space) return <LearningState description="This assignment does not exist or is no longer available." title="Assignment not found" />;
  const unit = space.units.find((item) => item.id === assignment.learning_unit_id);

  return <div className="space-y-6">
    <Link className="text-sm font-medium text-[var(--text-muted)] hover:text-[var(--text-strong)]" params={{ spaceId }} search={{ page: 1, status: "all" }} to="/modules/learning/spaces/$spaceId/assignments">← {space.title} assignments</Link>
    <section className="border border-[var(--border)] bg-[var(--surface)] p-5 sm:p-6">
      <div className="flex flex-wrap items-start justify-between gap-4"><div><div className="flex flex-wrap items-center gap-2"><LearningStatusBadge status={assignment.status} /><span className="text-xs font-medium text-[var(--text-muted)]">{unit ? `Unit ${unit.position} · ${unit.title}` : "Unit unavailable"}</span></div><p className="mt-3 text-sm text-[var(--text-muted)]">Due {formatLearningDateTime(assignment.due_at)} · Maximum {formatHundredths(assignment.max_score_hundredths)}</p></div>{canTeach ? <div className="text-right"><p className="font-tabular text-lg font-semibold text-[var(--text-strong)]">{assignment.submission_count} / {assignment.recipient_count || "—"}</p><p className="text-xs text-[var(--text-muted)]">started submissions</p></div> : null}</div>
      {assignment.close_reason ? <p className="mt-4 border-l-2 border-[var(--border-strong)] pl-3 text-sm text-[var(--text-muted)]">Closed: {assignment.close_reason}</p> : null}
    </section>
    <nav aria-label="Assignment sections" className="flex gap-1 overflow-x-auto border-b border-[var(--border)]">
      {allowedTabs.map((tab) => <button aria-current={search.tab === tab ? "page" : undefined} className={`min-h-11 shrink-0 border-b-2 px-4 text-sm font-semibold ${search.tab === tab ? "border-[var(--brand-strong)] text-[var(--brand-strong)]" : "border-transparent text-[var(--text-muted)] hover:text-[var(--text-strong)]"}`} key={tab} onClick={() => onSearchChange({ ...search, tab })} type="button">{tab === "work" ? "My work" : tab === "submissions" ? "Submissions" : tab[0].toUpperCase() + tab.slice(1)}</button>)}
    </nav>

    {search.tab === "brief" ? <section className="border border-[var(--border)] bg-[var(--surface)] p-5 sm:p-6"><h2 className="text-lg font-semibold text-[var(--text-strong)]">Instructions</h2><p className="mt-4 whitespace-pre-wrap text-sm leading-7 text-[var(--text-strong)]">{assignment.instructions}</p></section> : null}
    {search.tab === "rubric" ? <RubricSection assignment={assignment} canEdit={canTeach && assignment.status === "draft"} onDelete={setDeleteCriterion} onEdit={setCriterion} rubricTotal={rubricTotal} /> : null}
    {search.tab === "work" && canParticipate ? <StudentWork assignment={assignment} /> : null}
    {search.tab === "submissions" && canTeach ? <AssignmentSubmissions assignment={assignment} onSearchChange={(next) => onSearchChange({ ...search, submission_page: next.page, submission_status: next.status })} search={{ page: search.submission_page, status: search.submission_status }} /> : null}

    <AssignmentEditorDrawer assignment={assignment} onClose={() => setEditOpen(false)} onSaved={(next) => { setAssignment(next); setEditOpen(false); }} open={editOpen} />
    <CriterionDrawer assignment={assignment} criterion={criterion} onClose={() => setCriterion(null)} onSaved={() => { setCriterion(null); void load(); }} />
    <ConfirmDrawer confirmLabel="Remove criterion" description={`Remove ${deleteCriterion?.title ?? "this rubric criterion"}? The assignment maximum is unchanged.`} isPending={pending} onClose={() => setDeleteCriterion(null)} onConfirm={() => void (async () => { if (!deleteCriterion || pending) return; setPending(true); try { const response = await learningService.deleteRubricCriterion(deleteCriterion); if (!response.success) throw new Error(responseMessage(response, "Rubric criterion could not be removed")); toast.success("Rubric criterion removed"); setDeleteCriterion(null); void load(); } catch (deleteError) { toast.error(deleteError instanceof Error ? deleteError.message : "Rubric criterion could not be removed"); } finally { setPending(false); } })()} open={Boolean(deleteCriterion)} title="Remove rubric criterion?" />
    <AssignmentTransitionDrawer assignment={assignment} action={transition} onClose={() => setTransition(null)} onCompleted={(next) => { setAssignment(next); setTransition(null); }} />
  </div>;
}

function RubricSection({ assignment, canEdit, onDelete, onEdit, rubricTotal }: { assignment: LearningAssignment; canEdit: boolean; onDelete: (criterion: LearningRubricCriterion) => void; onEdit: (criterion: LearningRubricCriterion | "new") => void; rubricTotal: number }) {
  return <section className="space-y-4"><div className="flex flex-wrap items-start justify-between gap-3"><div><h2 className="text-lg font-semibold text-[var(--text-strong)]">Rubric</h2><p className="mt-1 text-sm text-[var(--text-muted)]">{assignment.status === "draft" ? "Criterion scores must total the assignment maximum before publication." : "This rubric was fixed when the assignment was published."}</p></div>{canEdit ? <Button onClick={() => onEdit("new")} size="sm" variant="secondary"><Plus className="size-4" />Add criterion</Button> : null}</div>
    <div className={`border p-4 text-sm ${rubricTotal === assignment.max_score_hundredths ? "border-[var(--border)] bg-[var(--surface)] text-[var(--text-muted)]" : "border-[var(--tone-warn-bd)] bg-[var(--badge-warning-bg)] text-[var(--badge-warning-text)]"}`}>Rubric total: <span className="font-tabular font-semibold">{formatHundredths(rubricTotal)} / {formatHundredths(assignment.max_score_hundredths)}</span></div>
    {assignment.rubric.length === 0 ? <LearningState description={canEdit ? "Add criteria before publishing this assignment." : "No rubric is available."} title="No rubric criteria" /> : <div className="divide-y divide-[var(--border)] border border-[var(--border)] bg-[var(--surface)]">{assignment.rubric.map((item) => <article className="flex flex-wrap items-start justify-between gap-4 p-4 sm:p-5" key={item.id}><div className="min-w-0"><p className="text-xs font-semibold uppercase tracking-[0.12em] text-[var(--brand-strong)]">Criterion {item.position}</p><h3 className="mt-2 font-semibold text-[var(--text-strong)]">{item.title}</h3>{item.description ? <p className="mt-2 whitespace-pre-wrap text-sm leading-6 text-[var(--text-muted)]">{item.description}</p> : null}</div><div className="flex items-center gap-2"><span className="font-tabular text-sm font-semibold text-[var(--text-strong)]">{formatHundredths(item.max_score_hundredths)}</span>{canEdit ? <><Button aria-label={`Edit ${item.title}`} onClick={() => onEdit(item)} size="icon-sm" variant="ghost"><Edit3 className="size-4" /></Button><Button aria-label={`Remove ${item.title}`} onClick={() => onDelete(item)} size="icon-sm" variant="ghost"><Trash2 className="size-4" /></Button></> : null}</div></article>)}</div>}
  </section>;
}

function AssignmentEditorDrawer({ assignment, onClose, onSaved, open }: { assignment: LearningAssignment; onClose: () => void; onSaved: (next: LearningAssignment) => void; open: boolean }) {
  const [position, setPosition] = useState(assignment.position); const [title, setTitle] = useState(assignment.title); const [instructions, setInstructions] = useState(assignment.instructions); const [dueAt, setDueAt] = useState(toDateTimeLocal(assignment.due_at)); const [maximum, setMaximum] = useState(formatHundredths(assignment.max_score_hundredths)); const [saving, setSaving] = useState(false);
  useEffect(() => { if (open) { setPosition(assignment.position); setTitle(assignment.title); setInstructions(assignment.instructions); setDueAt(toDateTimeLocal(assignment.due_at)); setMaximum(formatHundredths(assignment.max_score_hundredths)); } }, [assignment, open]);
  const maximumHundredths = parseHundredths(maximum); const dirty = position !== assignment.position || title !== assignment.title || instructions !== assignment.instructions || dueAt !== toDateTimeLocal(assignment.due_at) || maximumHundredths !== assignment.max_score_hundredths;
  const submit = async (event: React.FormEvent) => { event.preventDefault(); if (maximumHundredths === null || maximumHundredths <= 0 || saving) return; setSaving(true); try { const response = await learningService.updateAssignment(assignment, { position, title: title.trim(), instructions: instructions.trim(), due_at: new Date(dueAt).toISOString(), max_score_hundredths: maximumHundredths }); if (!response.success || !response.data) throw new Error(responseMessage(response, "Assignment could not be updated")); toast.success("Assignment updated"); onSaved(response.data); } catch (saveError) { toast.error(saveError instanceof Error ? saveError.message : "Assignment could not be updated"); } finally { setSaving(false); } };
  return <GuardedDrawer dirty={dirty} discardDescription="The unsaved assignment changes will be lost." onClose={onClose} open={open} pending={saving} panelClassName="sm:max-w-[720px]">{(requestClose) => <><DialogHeader onClose={saving ? undefined : requestClose} title="Edit assignment" /><form onSubmit={submit}><DialogBody className="space-y-5"><div><Label htmlFor="edit-assignment-title">Title</Label><Input className="mt-1.5" data-autofocus="true" id="edit-assignment-title" maxLength={200} onChange={(event) => setTitle(event.target.value)} required value={title} /></div><div><Label htmlFor="edit-assignment-instructions">Instructions</Label><Textarea className="mt-1.5 min-h-48" id="edit-assignment-instructions" maxLength={20000} onChange={(event) => setInstructions(event.target.value)} required value={instructions} /></div><div className="grid gap-5 sm:grid-cols-2"><div><Label htmlFor="edit-assignment-due">Due</Label><Input className="mt-1.5" id="edit-assignment-due" onChange={(event) => setDueAt(event.target.value)} required type="datetime-local" value={dueAt} /></div><div><Label htmlFor="edit-assignment-maximum">Maximum score</Label><Input className="mt-1.5" id="edit-assignment-maximum" min="0.01" onChange={(event) => setMaximum(event.target.value)} required step="0.01" type="number" value={maximum} /></div></div><div><Label htmlFor="edit-assignment-position">Position</Label><Input className="mt-1.5" id="edit-assignment-position" min={1} onChange={(event) => setPosition(Number(event.target.value))} required type="number" value={position} /></div></DialogBody><DialogFooter><Button disabled={saving} onClick={requestClose} type="button" variant="secondary">Cancel</Button><Button disabled={saving || !dirty || !title.trim() || !instructions.trim() || maximumHundredths === null || maximumHundredths <= 0} type="submit">{saving ? <><Loader2 className="size-4 animate-spin" />Saving…</> : "Save changes"}</Button></DialogFooter></form></>}</GuardedDrawer>;
}

function CriterionDrawer({ assignment, criterion, onClose, onSaved }: { assignment: LearningAssignment; criterion: LearningRubricCriterion | "new" | null; onClose: () => void; onSaved: () => void }) {
  const record = criterion && criterion !== "new" ? criterion : null; const [position, setPosition] = useState(1); const [title, setTitle] = useState(""); const [description, setDescription] = useState(""); const [maximum, setMaximum] = useState(""); const [saving, setSaving] = useState(false);
  useEffect(() => { if (criterion) { setPosition(record?.position ?? assignment.rubric.length + 1); setTitle(record?.title ?? ""); setDescription(record?.description ?? ""); setMaximum(record ? formatHundredths(record.max_score_hundredths) : ""); } }, [assignment.rubric.length, criterion, record]);
  const maximumHundredths = parseHundredths(maximum); const dirty = Boolean(criterion && (position !== (record?.position ?? assignment.rubric.length + 1) || title !== (record?.title ?? "") || description !== (record?.description ?? "") || maximumHundredths !== (record?.max_score_hundredths ?? null)));
  const submit = async (event: React.FormEvent) => { event.preventDefault(); if (!criterion || maximumHundredths === null || maximumHundredths <= 0 || saving) return; setSaving(true); try { const payload = { position, title: title.trim(), description: description.trim() || null, max_score_hundredths: maximumHundredths }; const response = record ? await learningService.updateRubricCriterion(record, payload) : await learningService.createRubricCriterion(assignment.id, payload); if (!response.success || !response.data) throw new Error(responseMessage(response, "Rubric criterion could not be saved")); toast.success(record ? "Rubric criterion updated" : "Rubric criterion added"); onSaved(); } catch (saveError) { toast.error(saveError instanceof Error ? saveError.message : "Rubric criterion could not be saved"); } finally { setSaving(false); } };
  return <GuardedDrawer dirty={dirty} discardDescription="The unsaved rubric criterion will be lost." onClose={onClose} open={Boolean(criterion)} pending={saving}>{(requestClose) => <><DialogHeader onClose={saving ? undefined : requestClose} title={record ? "Edit rubric criterion" : "Add rubric criterion"} /><form onSubmit={submit}><DialogBody className="space-y-5"><div><Label htmlFor="criterion-title">Title</Label><Input className="mt-1.5" data-autofocus="true" id="criterion-title" maxLength={200} onChange={(event) => setTitle(event.target.value)} required value={title} /></div><div><Label htmlFor="criterion-description">Description</Label><Textarea className="mt-1.5 min-h-32" id="criterion-description" maxLength={4000} onChange={(event) => setDescription(event.target.value)} value={description} /></div><div className="grid gap-5 sm:grid-cols-2"><div><Label htmlFor="criterion-maximum">Maximum score</Label><Input className="mt-1.5" id="criterion-maximum" min="0.01" onChange={(event) => setMaximum(event.target.value)} required step="0.01" type="number" value={maximum} /></div><div><Label htmlFor="criterion-position">Position</Label><Input className="mt-1.5" id="criterion-position" min={1} onChange={(event) => setPosition(Number(event.target.value))} required type="number" value={position} /></div></div></DialogBody><DialogFooter><Button disabled={saving} onClick={requestClose} type="button" variant="secondary">Cancel</Button><Button disabled={saving || !dirty || !title.trim() || maximumHundredths === null || maximumHundredths <= 0} type="submit">{saving ? <><Loader2 className="size-4 animate-spin" />Saving…</> : record ? "Save changes" : "Add criterion"}</Button></DialogFooter></form></>}</GuardedDrawer>;
}

function AssignmentTransitionDrawer({ action, assignment, onClose, onCompleted }: { action: "publish" | "close" | null; assignment: LearningAssignment; onClose: () => void; onCompleted: (next: LearningAssignment) => void }) {
  const [reason, setReason] = useState(""); const [pending, setPending] = useState(false); useEffect(() => { if (action) setReason(""); }, [action]); if (!action) return null;
  const run = async () => { if (pending || (action === "close" && !reason.trim())) return; setPending(true); try { const response = action === "publish" ? await learningService.publishAssignment(assignment) : await learningService.closeAssignment(assignment, reason.trim()); if (!response.success || !response.data) throw new Error(responseMessage(response, `Assignment could not be ${action === "publish" ? "published" : "closed"}`)); toast.success(action === "publish" ? "Assignment published" : "Assignment closed"); onCompleted(response.data); } catch (transitionError) { const current = await learningService.assignment(assignment.id); if (current.success && current.data && current.data.status === (action === "publish" ? "published" : "closed")) onCompleted(current.data); else toast.error(transitionError instanceof Error ? transitionError.message : "Assignment could not be updated"); } finally { setPending(false); } };
  return <DialogShell onClose={pending ? () => undefined : onClose} open><DialogHeader onClose={pending ? undefined : onClose} title={action === "publish" ? "Publish assignment?" : "Close assignment?"} /><DialogBody className="space-y-5"><div className="flex gap-4"><span className="flex size-10 shrink-0 items-center justify-center rounded-[9px] bg-[var(--badge-info-bg)] text-[var(--badge-info-text)]"><CheckCircle2 className="size-5" /></span><p className="text-sm leading-6 text-[var(--text-muted)]">{action === "publish" ? "Publishing fixes the brief, rubric, and eligible learner roster. Learners can then submit work." : "Closing stops further submissions. Existing versions and feedback remain available."}</p></div>{action === "close" ? <div><Label htmlFor="assignment-close-reason">Reason</Label><Textarea className="mt-1.5 min-h-28" data-autofocus="true" id="assignment-close-reason" maxLength={2000} onChange={(event) => setReason(event.target.value)} required value={reason} /></div> : null}</DialogBody><DialogFooter><Button disabled={pending} onClick={onClose} type="button" variant="secondary">Cancel</Button><Button disabled={pending || (action === "close" && !reason.trim())} onClick={() => void run()} type="button">{pending ? <Loader2 className="size-4 animate-spin" /> : null}{pending ? "Updating…" : action === "publish" ? "Publish assignment" : "Close assignment"}</Button></DialogFooter></DialogShell>;
}

function toDateTimeLocal(value: string) { const date = new Date(value); const local = new Date(date.getTime() - date.getTimezoneOffset() * 60_000); return local.toISOString().slice(0, 16); }

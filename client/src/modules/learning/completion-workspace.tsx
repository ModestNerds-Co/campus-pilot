import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Link } from "@tanstack/react-router";
import { CheckCircle2, Edit3, Loader2, Send } from "lucide-react";
import toast from "react-hot-toast";

import { Button } from "@/components/ui/button";
import {
  Table, TableEmpty, TableScroll, TableWrap, TBody, TD, TH, THead, TR,
} from "@/components/ui/data-table";
import { DialogBody, DialogFooter, DialogHeader, DialogShell } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { usePageChrome } from "@/modules/admin/layouts/page-chrome";
import { useAuthStore } from "@/stores/auth-store";

import { GuardedDrawer } from "./guarded-drawer";
import { learningService, responseMessage } from "./service";
import type {
  LearningCompletionPage, LearningCompletionPolicy,
  LearningCompletionRequirementInput, LearningSpace,
} from "./types";
import { LearningState, LearningStatusBadge } from "./ui";

type CompletionSource = { id: string; kind: "assignment" | "quiz"; title: string; status: string };

export function LearningCompletionWorkspace({ spaceId }: { spaceId: string }) {
  const permissions = useAuthStore((state) => state.user?.permissions ?? []);
  const canTeach = permissions.includes("*") || permissions.includes("learning:teach");
  const [space, setSpace] = useState<LearningSpace | null>(null);
  const [policy, setPolicy] = useState<LearningCompletionPolicy | null>(null);
  const [completion, setCompletion] = useState<LearningCompletionPage | null>(null);
  const [sources, setSources] = useState<CompletionSource[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [editorOpen, setEditorOpen] = useState(false);
  const [publishOpen, setPublishOpen] = useState(false);
  const [publishing, setPublishing] = useState(false);
  const requestRef = useRef(0);

  const load = useCallback(async () => {
    const requestId = ++requestRef.current;
    setLoading(true); setError(null);
    try {
      const spaceResponse = await learningService.space(spaceId);
      if (!spaceResponse.success || !spaceResponse.data) throw new Error(responseMessage(spaceResponse, "Learning space could not be loaded"));
      if (canTeach) {
        const [policyResponse, completionResponse, assignmentResponse, quizResponse] = await Promise.all([
          learningService.completionPolicy(spaceId), learningService.completion(spaceId),
          learningService.assignments(spaceId, { per_page: 100 }), learningService.quizzes(spaceId, { per_page: 100 }),
        ]);
        if (!completionResponse.success || !completionResponse.data) throw new Error(responseMessage(completionResponse, "Completion could not be loaded"));
        if (!assignmentResponse.success || !assignmentResponse.data) throw new Error(responseMessage(assignmentResponse, "Assignments could not be loaded"));
        if (!quizResponse.success || !quizResponse.data) throw new Error(responseMessage(quizResponse, "Quizzes could not be loaded"));
        if (requestId !== requestRef.current) return;
        setPolicy(policyResponse.success ? policyResponse.data : null);
        setCompletion(completionResponse.data);
        setSources([
          ...assignmentResponse.data.assignments.filter((item) => item.status !== "draft").map((item) => ({ id: item.id, kind: "assignment" as const, title: item.title, status: item.status })),
          ...quizResponse.data.quizzes.filter((item) => item.status !== "draft").map((item) => ({ id: item.id, kind: "quiz" as const, title: item.title, status: item.status })),
        ]);
      } else {
        const response = await learningService.myCompletion(spaceId);
        if (!response.success || !response.data) throw new Error(responseMessage(response, "Completion could not be loaded"));
        if (requestId !== requestRef.current) return;
        setPolicy(response.data.policy); setCompletion(response.data); setSources([]);
      }
      setSpace(spaceResponse.data);
    } catch (loadError) {
      if (requestId !== requestRef.current) return;
      setError(loadError instanceof Error ? loadError.message : "Completion could not be loaded");
    } finally { if (requestId === requestRef.current) setLoading(false); }
  }, [canTeach, spaceId]);

  useEffect(() => { void load(); return () => { requestRef.current += 1; }; }, [load]);
  usePageChrome("Completion", canTeach ? <div className="flex flex-wrap gap-2"><Button disabled={sources.length === 0} onClick={() => setEditorOpen(true)} variant="secondary"><Edit3 className="size-4" />{policy?.status === "draft" ? "Edit rules" : "Set rules"}</Button>{policy?.status === "draft" ? <Button onClick={() => setPublishOpen(true)}><Send className="size-4" />Publish rules</Button> : null}</div> : null);

  const publish = async () => {
    if (!policy || policy.status !== "draft" || publishing) return;
    setPublishing(true);
    try { const response = await learningService.publishCompletionPolicy(spaceId, policy); if (!response.success || !response.data) throw new Error(responseMessage(response, "Completion rules could not be published")); toast.success("Completion rules published"); setPublishOpen(false); await load(); }
    catch (publishError) { toast.error(publishError instanceof Error ? publishError.message : "Completion rules could not be published"); }
    finally { setPublishing(false); }
  };

  if (loading) return <LearningState busy title="Loading completion…" />;
  if (error) return <LearningState description={error} onRetry={() => void load()} title="Completion unavailable" />;
  if (!space || !completion) return <LearningState description="This Learning space is unavailable." title="Completion unavailable" />;
  const shownPolicy = policy ?? completion.policy;

  return <div className="space-y-6">
    <Link className="text-sm font-medium text-[var(--text-muted)] hover:text-[var(--text-strong)]" params={{ spaceId }} to="/modules/learning/spaces/$spaceId">← {space.title}</Link>
    <section className="border border-[var(--border)] bg-[var(--surface)] p-5 sm:p-6"><div className="flex flex-wrap items-start justify-between gap-4"><div><h2 className="text-lg font-semibold text-[var(--text-strong)]">Completion rules</h2><p className="mt-1 text-sm text-[var(--text-muted)]">{space.subject_name} · {space.class_group_name}</p></div>{shownPolicy ? <LearningStatusBadge status={shownPolicy.status} /> : null}</div>{shownPolicy ? <div className="mt-5 divide-y divide-[var(--border)] border border-[var(--border)]">{shownPolicy.requirements.map((item) => <div className="flex flex-wrap items-center justify-between gap-3 p-3" key={item.id}><div><p className="text-sm font-medium text-[var(--text-strong)]">{item.source_title}</p><p className="text-xs text-[var(--text-muted)]">{item.requirement_type === "quiz" ? "Quiz" : "Assignment"}</p></div><p className="font-tabular text-sm text-[var(--text-muted)]">Minimum {item.minimum_score_basis_points / 100}%</p></div>)}</div> : <p className="mt-5 text-sm text-[var(--text-muted)]">No completion rules have been published.</p>}</section>

    <section><h2 className="text-lg font-semibold text-[var(--text-strong)]">{canTeach ? "Class completion" : "My completion"}</h2><div className="mt-4"><TableWrap>{completion.progress.length === 0 ? <TableEmpty description={shownPolicy ? "Completion will appear when eligible learner activity is available." : "Publish completion rules first."} icon={<CheckCircle2 />} title="No completion results" /> : <TableScroll><Table className="min-w-[680px]"><THead><tr>{canTeach ? <TH>Learner</TH> : null}<TH>Requirements met</TH><TH>Progress</TH><TH>Status</TH></tr></THead><TBody>{completion.progress.map((entry) => <TR key={entry.learner_id}>{canTeach ? <TD><p className="font-medium text-[var(--text-strong)]">{entry.learner_name}</p><p className="text-xs text-[var(--text-muted)]">{entry.learner_number}</p></TD> : null}<TD className="font-tabular text-[var(--text-muted)]">{entry.completed_count} / {entry.required_count}</TD><TD><div className="h-2 w-36 overflow-hidden rounded-full bg-[var(--surface-muted)]"><div className="h-full bg-[var(--brand-strong)]" style={{ width: `${entry.completion_percent}%` }} /></div><p className="mt-1 font-tabular text-xs text-[var(--text-muted)]">{entry.completion_percent}%</p></TD><TD><LearningStatusBadge status={entry.complete ? "complete" : "in_progress"} /></TD></TR>)}</TBody></Table></TableScroll>}</TableWrap></div></section>

    <CompletionRulesDrawer onClose={() => setEditorOpen(false)} onSaved={() => { setEditorOpen(false); void load(); }} open={editorOpen} policy={policy} sources={sources} spaceId={spaceId} />
    <DialogShell onClose={publishing ? () => undefined : () => setPublishOpen(false)} open={publishOpen}><DialogHeader onClose={publishing ? undefined : () => setPublishOpen(false)} title="Publish completion rules?" /><DialogBody><div className="flex gap-4"><span className="flex size-10 shrink-0 items-center justify-center rounded-[9px] bg-[var(--badge-info-bg)] text-[var(--badge-info-text)]"><CheckCircle2 className="size-5" /></span><p className="text-sm leading-6 text-[var(--text-muted)]">Publishing freezes these requirements and the current eligible class roster. Future changes create a new policy version.</p></div></DialogBody><DialogFooter><Button disabled={publishing} onClick={() => setPublishOpen(false)} variant="secondary">Cancel</Button><Button disabled={publishing} onClick={() => void publish()}>{publishing ? <Loader2 className="size-4 animate-spin" /> : null}Publish rules</Button></DialogFooter></DialogShell>
  </div>;
}

function CompletionRulesDrawer({ onClose, onSaved, open, policy, sources, spaceId }: { onClose: () => void; onSaved: () => void; open: boolean; policy: LearningCompletionPolicy | null; sources: CompletionSource[]; spaceId: string }) {
  const initial = useMemo(() => Object.fromEntries((policy?.status === "draft" ? policy.requirements : []).map((item) => [`${item.requirement_type}:${item.source_id}`, item.minimum_score_basis_points / 100])), [policy]);
  const [selected, setSelected] = useState<Record<string, number>>(initial);
  const [saving, setSaving] = useState(false);
  useEffect(() => { if (open) setSelected(initial); }, [initial, open]);
  const dirty = JSON.stringify(selected) !== JSON.stringify(initial);
  const submit = async (event: React.FormEvent) => {
    event.preventDefault(); if (saving || Object.keys(selected).length === 0) return; setSaving(true);
    try {
      const requirements: LearningCompletionRequirementInput[] = sources.filter((source) => `${source.kind}:${source.id}` in selected).map((source) => ({ requirement_type: source.kind, source_id: source.id, minimum_score_basis_points: Math.round(selected[`${source.kind}:${source.id}`] * 100) }));
      const response = await learningService.saveCompletionPolicy(spaceId, policy, requirements);
      if (!response.success || !response.data) throw new Error(responseMessage(response, "Completion rules could not be saved"));
      toast.success("Completion rules saved"); onSaved();
    } catch (saveError) { toast.error(saveError instanceof Error ? saveError.message : "Completion rules could not be saved"); }
    finally { setSaving(false); }
  };
  return <GuardedDrawer dirty={dirty} discardDescription="The unsaved completion rules will be lost." onClose={onClose} open={open} pending={saving} panelClassName="sm:max-w-[720px]">{(requestClose) => <><DialogHeader onClose={saving ? undefined : requestClose} title="Completion rules" /><form onSubmit={submit}><DialogBody className="space-y-3"><p className="mb-5 text-sm leading-6 text-[var(--text-muted)]">Select the published activities learners must complete and set the minimum score for each.</p>{sources.map((source) => { const key = `${source.kind}:${source.id}`; const checked = key in selected; return <div className={`grid gap-3 border p-4 sm:grid-cols-[1fr_140px] ${checked ? "border-[var(--brand-strong)] bg-[var(--brand-subtle)]" : "border-[var(--border)]"}`} key={key}><label className="flex cursor-pointer items-start gap-3"><input checked={checked} className="mt-0.5 size-4 accent-[var(--brand-strong)]" onChange={(event) => setSelected((current) => { const next = { ...current }; if (event.target.checked) next[key] = 0; else delete next[key]; return next; })} type="checkbox" /><span><span className="block text-sm font-medium text-[var(--text-strong)]">{source.title}</span><span className="mt-1 block text-xs text-[var(--text-muted)]">{source.kind === "quiz" ? "Quiz" : "Assignment"} · {source.status}</span></span></label><div><span className="sr-only">Minimum score for {source.title}</span><Input aria-label={`Minimum score for ${source.title}`} disabled={!checked} max={100} min={0} onChange={(event) => setSelected((current) => ({ ...current, [key]: Number(event.target.value) }))} step="0.01" type="number" value={checked ? selected[key] : 0} /></div></div>; })}</DialogBody><DialogFooter><Button disabled={saving} onClick={requestClose} type="button" variant="secondary">Cancel</Button><Button disabled={saving || !dirty || Object.keys(selected).length === 0} type="submit">{saving ? <Loader2 className="size-4 animate-spin" /> : null}Save rules</Button></DialogFooter></form></>}</GuardedDrawer>;
}

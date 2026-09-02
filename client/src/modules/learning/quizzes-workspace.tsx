import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Link, useNavigate } from "@tanstack/react-router";
import { ClipboardCheck, Loader2, Plus } from "lucide-react";
import toast from "react-hot-toast";

import { Button } from "@/components/ui/button";
import {
  Table, TableControlsBar, TableControlsPagination, TableEmpty, TableError,
  TableLoading, TableScroll, TableWrap, TBody, TD, TH, THead, TR,
} from "@/components/ui/data-table";
import { DialogBody, DialogFooter, DialogHeader } from "@/components/ui/dialog";
import { Input, Label, Select, Textarea } from "@/components/ui/input";
import { usePageChrome } from "@/modules/admin/layouts/page-chrome";
import { useAuthStore } from "@/stores/auth-store";

import { GuardedDrawer } from "./guarded-drawer";
import { learningService, responseMessage } from "./service";
import type { LearningQuiz, LearningQuizStatus, LearningSpace } from "./types";
import { formatLearningDateTime, LearningStatusBadge } from "./ui";

export interface LearningQuizzesSearchState {
  page: number;
  status: "all" | LearningQuizStatus;
}

export function LearningQuizzesWorkspace({ onSearchChange, search, spaceId }: {
  onSearchChange: (next: LearningQuizzesSearchState) => void;
  search: LearningQuizzesSearchState;
  spaceId: string;
}) {
  const navigate = useNavigate();
  const permissions = useAuthStore((state) => state.user?.permissions ?? []);
  const canTeach = permissions.includes("*") || permissions.includes("learning:teach");
  const [space, setSpace] = useState<LearningSpace | null>(null);
  const [quizzes, setQuizzes] = useState<LearningQuiz[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [totalPages, setTotalPages] = useState(1);
  const [createOpen, setCreateOpen] = useState(false);
  const requestRef = useRef(0);

  const load = useCallback(async () => {
    const requestId = ++requestRef.current;
    setLoading(true);
    setError(null);
    try {
      const [spaceResponse, quizResponse] = await Promise.all([
        learningService.space(spaceId),
        learningService.quizzes(spaceId, {
          page: search.page,
          per_page: 25,
          status: search.status === "all" ? undefined : search.status,
        }),
      ]);
      if (!spaceResponse.success || !spaceResponse.data) throw new Error(responseMessage(spaceResponse, "Learning space could not be loaded"));
      if (!quizResponse.success || !quizResponse.data) throw new Error(responseMessage(quizResponse, "Quizzes could not be loaded"));
      if (requestId !== requestRef.current) return;
      setSpace(spaceResponse.data);
      setQuizzes(quizResponse.data.quizzes);
      setTotalPages(Math.max(1, quizResponse.pagination?.total_pages ?? 1));
    } catch (loadError) {
      if (requestId !== requestRef.current) return;
      setError(loadError instanceof Error ? loadError.message : "Quizzes could not be loaded");
    } finally {
      if (requestId === requestRef.current) setLoading(false);
    }
  }, [search.page, search.status, spaceId]);

  useEffect(() => {
    void load();
    return () => { requestRef.current += 1; };
  }, [load]);

  const availableUnits = useMemo(() => space?.units.filter((unit) => unit.status !== "withdrawn") ?? [], [space]);
  const canCreate = canTeach && availableUnits.length > 0;
  usePageChrome("Quizzes", canCreate ? <Button onClick={() => setCreateOpen(true)}><Plus className="size-4" />New quiz</Button> : null);

  return <div className="space-y-6">
    <Link className="text-sm font-medium text-[var(--text-muted)] hover:text-[var(--text-strong)]" params={{ spaceId }} to="/modules/learning/spaces/$spaceId">← {space?.title ?? "Learning space"}</Link>
    <div className="flex flex-wrap items-end justify-between gap-4">
      <div><p className="text-sm font-medium text-[var(--text-strong)]">{space ? `${space.subject_name} · ${space.class_group_name}` : "Space quizzes"}</p><p className="mt-1 text-sm text-[var(--text-muted)]">Class quizzes and attempt history.</p></div>
      {canTeach && space && !canCreate ? <p className="text-xs text-[var(--text-muted)]">Add a unit before creating a quiz.</p> : null}
    </div>
    <TableControlsBar>
      <Select aria-label="Quiz status" className="sm:w-48" onChange={(event) => onSearchChange({ page: 1, status: event.target.value as LearningQuizzesSearchState["status"] })} value={search.status}>
        <option value="all">All statuses</option>{canTeach ? <option value="draft">Draft</option> : null}<option value="published">Published</option><option value="closed">Closed</option>
      </Select>
      {!loading && quizzes.length ? <TableControlsPagination onNext={() => onSearchChange({ ...search, page: Math.min(totalPages, search.page + 1) })} onPrevious={() => onSearchChange({ ...search, page: Math.max(1, search.page - 1) })} page={search.page} totalPages={totalPages} /> : null}
    </TableControlsBar>
    <TableWrap>
      {loading ? <TableLoading columns={canTeach ? 6 : 4} label="Loading quizzes…" /> : error ? <TableError description={error} onRetry={() => void load()} /> : quizzes.length === 0 ? <TableEmpty description={search.status !== "all" ? "Change the status filter." : canTeach ? "Create the first quiz in this space." : "Published quizzes will appear here."} icon={<ClipboardCheck />} title={search.status !== "all" ? "No quizzes match" : "No quizzes yet"} /> : <TableScroll><Table className="min-w-[820px]"><THead><tr><TH>Quiz</TH><TH>Window</TH><TH>Pass mark</TH><TH>Status</TH>{canTeach ? <><TH>Attempts</TH><TH>Recipients</TH></> : null}</tr></THead><TBody>
        {quizzes.map((quiz) => <TR key={quiz.id}><TD><Link className="font-semibold text-[var(--text-strong)] hover:text-[var(--brand-strong)] hover:underline" params={{ quizId: quiz.id, spaceId }} to="/modules/learning/spaces/$spaceId/quizzes/$quizId">{quiz.title}</Link><p className="mt-1 text-xs text-[var(--text-muted)]">{quiz.questions.length} question{quiz.questions.length === 1 ? "" : "s"} · {quiz.attempt_limit} attempt{quiz.attempt_limit === 1 ? "" : "s"}</p></TD><TD className="text-[var(--text-muted)]">{quiz.opens_at ? formatLearningDateTime(quiz.opens_at) : "Available when published"}<br />{quiz.closes_at ? `to ${formatLearningDateTime(quiz.closes_at)}` : "No closing time"}</TD><TD className="font-tabular text-[var(--text-muted)]">{quiz.pass_score_basis_points / 100}%</TD><TD><LearningStatusBadge status={quiz.status} /></TD>{canTeach ? <><TD className="font-tabular text-[var(--text-muted)]">{quiz.submitted_attempt_count}</TD><TD className="font-tabular text-[var(--text-muted)]">{quiz.recipient_count || "—"}</TD></> : null}</TR>)}
      </TBody></Table></TableScroll>}
    </TableWrap>
    <CreateQuizDrawer onClose={() => setCreateOpen(false)} onCreated={(quiz) => { setCreateOpen(false); void navigate({ to: "/modules/learning/spaces/$spaceId/quizzes/$quizId", params: { spaceId, quizId: quiz.id } }); }} open={createOpen} quizCount={quizzes.length} units={availableUnits} />
  </div>;
}

function CreateQuizDrawer({ onClose, onCreated, open, quizCount, units }: { onClose: () => void; onCreated: (quiz: LearningQuiz) => void; open: boolean; quizCount: number; units: LearningSpace["units"] }) {
  const [unitId, setUnitId] = useState("");
  const [position, setPosition] = useState(1);
  const [title, setTitle] = useState("");
  const [instructions, setInstructions] = useState("");
  const [opensAt, setOpensAt] = useState("");
  const [closesAt, setClosesAt] = useState("");
  const [attemptLimit, setAttemptLimit] = useState(1);
  const [passMark, setPassMark] = useState(50);
  const [saving, setSaving] = useState(false);
  const initialized = useRef(false);

  useEffect(() => {
    if (!open) { initialized.current = false; return; }
    if (initialized.current) return;
    initialized.current = true;
    setUnitId(units[0]?.id ?? ""); setPosition(quizCount + 1); setTitle(""); setInstructions(""); setOpensAt(""); setClosesAt(""); setAttemptLimit(1); setPassMark(50);
  }, [open, quizCount, units]);

  const dirty = Boolean(title || instructions || opensAt || closesAt || attemptLimit !== 1 || passMark !== 50);
  const submit = async (event: React.FormEvent) => {
    event.preventDefault();
    if (!unitId || !title.trim() || saving) return;
    setSaving(true);
    try {
      const response = await learningService.createQuiz(unitId, {
        position, title: title.trim(), instructions: instructions.trim() || null,
        opens_at: opensAt ? new Date(opensAt).toISOString() : null,
        closes_at: closesAt ? new Date(closesAt).toISOString() : null,
        attempt_limit: attemptLimit, pass_score_basis_points: Math.round(passMark * 100),
      });
      if (!response.success || !response.data) throw new Error(responseMessage(response, "Quiz could not be created"));
      toast.success("Quiz draft created"); onCreated(response.data);
    } catch (saveError) { toast.error(saveError instanceof Error ? saveError.message : "Quiz could not be created"); }
    finally { setSaving(false); }
  };

  return <GuardedDrawer dirty={dirty} discardDescription="The unsaved quiz setup will be lost." onClose={onClose} open={open} pending={saving} panelClassName="sm:max-w-[720px]">{(requestClose) => <><DialogHeader onClose={saving ? undefined : requestClose} title="New quiz" /><form onSubmit={submit}><DialogBody className="space-y-5">
    <div><Label htmlFor="quiz-unit">Unit</Label><Select className="mt-1.5" data-autofocus="true" id="quiz-unit" onChange={(event) => setUnitId(event.target.value)} required value={unitId}>{units.map((unit) => <option key={unit.id} value={unit.id}>{unit.position}. {unit.title}</option>)}</Select></div>
    <div><Label htmlFor="quiz-title">Title</Label><Input className="mt-1.5" id="quiz-title" maxLength={200} onChange={(event) => setTitle(event.target.value)} required value={title} /></div>
    <div><Label htmlFor="quiz-instructions">Instructions</Label><Textarea className="mt-1.5 min-h-32" id="quiz-instructions" maxLength={10000} onChange={(event) => setInstructions(event.target.value)} value={instructions} /></div>
    <div className="grid gap-5 sm:grid-cols-2"><div><Label htmlFor="quiz-opens">Opens</Label><Input className="mt-1.5" id="quiz-opens" onChange={(event) => setOpensAt(event.target.value)} type="datetime-local" value={opensAt} /></div><div><Label htmlFor="quiz-closes">Closes</Label><Input className="mt-1.5" id="quiz-closes" onChange={(event) => setClosesAt(event.target.value)} type="datetime-local" value={closesAt} /></div></div>
    <div className="grid gap-5 sm:grid-cols-3"><div><Label htmlFor="quiz-attempts">Attempt limit</Label><Input className="mt-1.5" id="quiz-attempts" max={10} min={1} onChange={(event) => setAttemptLimit(Number(event.target.value))} required type="number" value={attemptLimit} /></div><div><Label htmlFor="quiz-pass">Pass mark (%)</Label><Input className="mt-1.5" id="quiz-pass" max={100} min={0} onChange={(event) => setPassMark(Number(event.target.value))} required step="0.01" type="number" value={passMark} /></div><div><Label htmlFor="quiz-position">Position</Label><Input className="mt-1.5" id="quiz-position" min={1} onChange={(event) => setPosition(Number(event.target.value))} required type="number" value={position} /></div></div>
  </DialogBody><DialogFooter><Button disabled={saving} onClick={requestClose} type="button" variant="secondary">Cancel</Button><Button disabled={saving || !unitId || !title.trim()} type="submit">{saving ? <><Loader2 className="size-4 animate-spin" />Creating…</> : "Create quiz"}</Button></DialogFooter></form></>}</GuardedDrawer>;
}

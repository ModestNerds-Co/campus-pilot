import { useCallback, useEffect, useRef, useState } from "react";
import { Link } from "@tanstack/react-router";
import { CheckCircle2, ClipboardCheck, Edit3, Loader2, Plus, Save, Send, Trash2 } from "lucide-react";
import toast from "react-hot-toast";

import { Button } from "@/components/ui/button";
import { ConfirmDrawer } from "@/components/ui/confirm-drawer";
import {
  Table, TableEmpty, TableScroll, TableWrap, TBody, TD, TH, THead, TR,
} from "@/components/ui/data-table";
import { DialogBody, DialogFooter, DialogHeader, DialogShell } from "@/components/ui/dialog";
import { Input, Label, Textarea } from "@/components/ui/input";
import { usePageChrome } from "@/modules/admin/layouts/page-chrome";
import { useAuthStore } from "@/stores/auth-store";

import { GuardedDrawer } from "./guarded-drawer";
import { learningService, responseMessage } from "./service";
import type {
  CreateLearningQuizQuestion, LearningQuiz, LearningQuizAttempt,
  LearningQuizQuestion, LearningSpace,
} from "./types";
import { formatLearningDateTime, LearningState, LearningStatusBadge } from "./ui";

export function LearningQuizWorkspace({ quizId, spaceId }: { quizId: string; spaceId: string }) {
  const permissions = useAuthStore((state) => state.user?.permissions ?? []);
  const canTeach = permissions.includes("*") || permissions.includes("learning:teach");
  const canParticipate = permissions.includes("learning:participate") && !canTeach;
  const [quiz, setQuiz] = useState<LearningQuiz | null>(null);
  const [space, setSpace] = useState<LearningSpace | null>(null);
  const [attempts, setAttempts] = useState<LearningQuizAttempt[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [editOpen, setEditOpen] = useState(false);
  const [question, setQuestion] = useState<LearningQuizQuestion | "new" | null>(null);
  const [deleteQuestion, setDeleteQuestion] = useState<LearningQuizQuestion | null>(null);
  const [transition, setTransition] = useState<"publish" | "close" | null>(null);
  const [pendingDelete, setPendingDelete] = useState(false);
  const requestRef = useRef(0);

  const load = useCallback(async () => {
    const requestId = ++requestRef.current;
    setLoading(true); setError(null);
    try {
      const [quizResponse, spaceResponse, attemptResponse] = await Promise.all([
        learningService.quiz(quizId), learningService.space(spaceId), learningService.quizAttempts(quizId, { per_page: 100 }),
      ]);
      if (!quizResponse.success || !quizResponse.data) throw new Error(responseMessage(quizResponse, "Quiz could not be loaded"));
      if (!spaceResponse.success || !spaceResponse.data) throw new Error(responseMessage(spaceResponse, "Learning space could not be loaded"));
      if (!attemptResponse.success || !attemptResponse.data) throw new Error(responseMessage(attemptResponse, "Quiz attempts could not be loaded"));
      if (requestId !== requestRef.current) return;
      setQuiz(quizResponse.data); setSpace(spaceResponse.data); setAttempts(attemptResponse.data.attempts);
    } catch (loadError) {
      if (requestId !== requestRef.current) return;
      setError(loadError instanceof Error ? loadError.message : "Quiz could not be loaded");
    } finally { if (requestId === requestRef.current) setLoading(false); }
  }, [quizId, spaceId]);

  useEffect(() => { void load(); return () => { requestRef.current += 1; }; }, [load]);

  const publishReady = Boolean(quiz?.questions.length && quiz.questions.every((item) => item.choices.length >= 2 && item.choices.filter((choice) => choice.is_correct).length === 1));
  usePageChrome(quiz?.title ?? "Quiz", quiz && canTeach ? <div className="flex flex-wrap gap-2">{quiz.status === "draft" ? <><Button onClick={() => setEditOpen(true)} variant="secondary"><Edit3 className="size-4" />Edit</Button><Button disabled={!publishReady} onClick={() => setTransition("publish")}><Send className="size-4" />Publish</Button></> : quiz.status === "published" ? <Button onClick={() => setTransition("close")} variant="secondary">Close quiz</Button> : null}</div> : null);

  const removeQuestion = async () => {
    if (!deleteQuestion || pendingDelete) return;
    setPendingDelete(true);
    try {
      const response = await learningService.deleteQuizQuestion(deleteQuestion);
      if (!response.success && response.message) throw new Error(responseMessage(response, "Question could not be removed"));
      setDeleteQuestion(null); toast.success("Question removed"); await load();
    } catch (deleteError) { toast.error(deleteError instanceof Error ? deleteError.message : "Question could not be removed"); }
    finally { setPendingDelete(false); }
  };

  if (loading) return <LearningState busy title="Loading quiz…" />;
  if (error) return <LearningState description={error} onRetry={() => void load()} title="Quiz unavailable" />;
  if (!quiz || !space) return <LearningState description="This quiz does not exist or is no longer available." title="Quiz not found" />;
  const unit = space.units.find((item) => item.id === quiz.learning_unit_id);

  return <div className="space-y-6">
    <Link className="text-sm font-medium text-[var(--text-muted)] hover:text-[var(--text-strong)]" params={{ spaceId }} search={{ page: 1, status: "all" }} to="/modules/learning/spaces/$spaceId/quizzes">← {space.title} quizzes</Link>
    <section className="border border-[var(--border)] bg-[var(--surface)] p-5 sm:p-6">
      <div className="flex flex-wrap items-start justify-between gap-4"><div><div className="flex flex-wrap items-center gap-2"><LearningStatusBadge status={quiz.status} /><span className="text-xs font-medium text-[var(--text-muted)]">{unit ? `Unit ${unit.position} · ${unit.title}` : "Unit unavailable"}</span></div><p className="mt-3 text-sm text-[var(--text-muted)]">Pass mark {quiz.pass_score_basis_points / 100}% · {quiz.attempt_limit} attempt{quiz.attempt_limit === 1 ? "" : "s"}{quiz.closes_at ? ` · closes ${formatLearningDateTime(quiz.closes_at)}` : ""}</p></div>{canTeach ? <div className="text-right"><p className="font-tabular text-lg font-semibold text-[var(--text-strong)]">{quiz.submitted_attempt_count}</p><p className="text-xs text-[var(--text-muted)]">submitted attempts</p></div> : null}</div>
      {quiz.instructions ? <p className="mt-5 whitespace-pre-wrap text-sm leading-7 text-[var(--text-strong)]">{quiz.instructions}</p> : null}
      {quiz.close_reason ? <p className="mt-4 border-l-2 border-[var(--border-strong)] pl-3 text-sm text-[var(--text-muted)]">Closed: {quiz.close_reason}</p> : null}
    </section>

    {canTeach ? <TeacherQuiz quiz={quiz} attempts={attempts} onAdd={() => setQuestion("new")} onDelete={setDeleteQuestion} onEdit={setQuestion} /> : null}
    {canParticipate ? <LearnerQuiz quiz={quiz} attempts={attempts} onChanged={() => void load()} /> : null}

    <QuizEditorDrawer onClose={() => setEditOpen(false)} onSaved={() => { setEditOpen(false); void load(); }} open={editOpen} quiz={quiz} />
    <QuestionDrawer onClose={() => setQuestion(null)} onSaved={() => { setQuestion(null); void load(); }} question={question} quiz={quiz} />
    <ConfirmDrawer confirmLabel="Remove question" description="The draft question and its choices will be removed." isPending={pendingDelete} onClose={() => setDeleteQuestion(null)} onConfirm={() => void removeQuestion()} open={Boolean(deleteQuestion)} title="Remove question?" />
    <QuizTransitionDrawer action={transition} onClose={() => setTransition(null)} onCompleted={(next) => { setTransition(null); setQuiz(next); void load(); }} quiz={quiz} />
  </div>;
}

function TeacherQuiz({ attempts, onAdd, onDelete, onEdit, quiz }: { attempts: LearningQuizAttempt[]; onAdd: () => void; onDelete: (question: LearningQuizQuestion) => void; onEdit: (question: LearningQuizQuestion) => void; quiz: LearningQuiz }) {
  return <>
    <section><div className="flex flex-wrap items-center justify-between gap-3"><div><h2 className="text-lg font-semibold text-[var(--text-strong)]">Questions</h2><p className="mt-1 text-sm text-[var(--text-muted)]">Correct answers are visible only to assigned staff.</p></div>{quiz.status === "draft" ? <Button onClick={onAdd} variant="secondary"><Plus className="size-4" />Add question</Button> : null}</div>
      <div className="mt-4 divide-y divide-[var(--border)] border border-[var(--border)] bg-[var(--surface)]">{quiz.questions.length === 0 ? <div className="p-6 text-sm text-[var(--text-muted)]">No questions yet.</div> : quiz.questions.map((item) => <article className="p-5" key={item.id}><div className="flex items-start justify-between gap-4"><div><p className="font-semibold text-[var(--text-strong)]">{item.position}. {item.prompt}</p><p className="mt-1 text-xs text-[var(--text-muted)]">{item.points} point{item.points === 1 ? "" : "s"}</p></div>{quiz.status === "draft" ? <div className="flex gap-1"><Button aria-label="Edit question" onClick={() => onEdit(item)} size="icon" variant="ghost"><Edit3 className="size-4" /></Button><Button aria-label="Remove question" onClick={() => onDelete(item)} size="icon" variant="ghost"><Trash2 className="size-4" /></Button></div> : null}</div><div className="mt-4 grid gap-2 sm:grid-cols-2">{item.choices.map((choice) => <div className={`border px-3 py-2 text-sm ${choice.is_correct ? "border-[var(--tone-success)] bg-[var(--tone-success-bg)] text-[var(--tone-success)]" : "border-[var(--border)] text-[var(--text-muted)]"}`} key={choice.id}>{choice.label}{choice.is_correct ? " · Correct" : ""}</div>)}</div></article>)}</div>
    </section>
    <section><h2 className="text-lg font-semibold text-[var(--text-strong)]">Attempts</h2><div className="mt-4"><TableWrap>{attempts.length === 0 ? <TableEmpty description="Learner attempts will appear after the quiz is published." icon={<ClipboardCheck />} title="No attempts yet" /> : <TableScroll><Table className="min-w-[700px]"><THead><tr><TH>Learner</TH><TH>Attempt</TH><TH>Status</TH><TH>Score</TH><TH>Started</TH></tr></THead><TBody>{attempts.map((attempt) => <TR key={attempt.id}><TD><p className="font-medium text-[var(--text-strong)]">{attempt.learner_name}</p><p className="text-xs text-[var(--text-muted)]">{attempt.learner_number}</p></TD><TD className="font-tabular">{attempt.attempt_number}</TD><TD><LearningStatusBadge status={attempt.status} /></TD><TD className="font-tabular text-[var(--text-muted)]">{attempt.score_basis_points === null ? "—" : `${attempt.score_basis_points / 100}%`}</TD><TD className="text-[var(--text-muted)]">{formatLearningDateTime(attempt.started_at)}</TD></TR>)}</TBody></Table></TableScroll>}</TableWrap></div></section>
  </>;
}

function LearnerQuiz({ attempts, onChanged, quiz }: { attempts: LearningQuizAttempt[]; onChanged: () => void; quiz: LearningQuiz }) {
  const current = attempts.find((attempt) => attempt.status === "in_progress") ?? null;
  const [attempt, setAttempt] = useState<LearningQuizAttempt | null>(current);
  const [answers, setAnswers] = useState<Record<string, string>>(() => Object.fromEntries((current?.answers ?? []).map((answer) => [answer.question_id, answer.selected_choice_id])));
  const [pending, setPending] = useState(false);
  const submitKey = useRef<string | null>(null);
  useEffect(() => { setAttempt(current); setAnswers(Object.fromEntries((current?.answers ?? []).map((answer) => [answer.question_id, answer.selected_choice_id]))); }, [current]);
  const submitted = attempts.filter((item) => item.status === "submitted");
  const answeredCount = Object.keys(answers).length;
  const canStart = quiz.status === "published" && !attempt && attempts.length < quiz.attempt_limit;

  const start = async () => {
    if (pending) return; setPending(true);
    try { const response = await learningService.startQuizAttempt(quiz.id); if (!response.success || !response.data) throw new Error(responseMessage(response, "Quiz attempt could not be started")); setAttempt(response.data); setAnswers({}); toast.success("Quiz attempt started"); }
    catch (startError) { toast.error(startError instanceof Error ? startError.message : "Quiz attempt could not be started"); }
    finally { setPending(false); }
  };

  const save = async () => {
    if (!attempt || pending) return null; setPending(true);
    try { const response = await learningService.saveQuizAttempt(attempt, Object.entries(answers).map(([question_id, selected_choice_id]) => ({ question_id, selected_choice_id }))); if (!response.success || !response.data) throw new Error(responseMessage(response, "Answers could not be saved")); setAttempt(response.data); toast.success("Answers saved"); return response.data; }
    catch (saveError) { toast.error(saveError instanceof Error ? saveError.message : "Answers could not be saved"); return null; }
    finally { setPending(false); }
  };

  const submit = async () => {
    if (!attempt || pending || answeredCount !== quiz.questions.length) return;
    setPending(true);
    try {
      const saved = await learningService.saveQuizAttempt(attempt, Object.entries(answers).map(([question_id, selected_choice_id]) => ({ question_id, selected_choice_id })));
      if (!saved.success || !saved.data) throw new Error(responseMessage(saved, "Answers could not be saved"));
      submitKey.current ??= crypto.randomUUID();
      const response = await learningService.submitQuizAttempt(saved.data, submitKey.current);
      if (!response.success || !response.data) throw new Error(responseMessage(response, "Quiz attempt could not be submitted"));
      setAttempt(null); submitKey.current = null; toast.success(response.data.passed ? "Quiz submitted · passed" : "Quiz submitted"); onChanged();
    } catch (submitError) { toast.error(submitError instanceof Error ? submitError.message : "Quiz attempt could not be submitted"); }
    finally { setPending(false); }
  };

  return <>
    {attempt ? <section className="space-y-5"><div className="flex flex-wrap items-end justify-between gap-3"><div><h2 className="text-lg font-semibold text-[var(--text-strong)]">Attempt {attempt.attempt_number}</h2><p className="mt-1 text-sm text-[var(--text-muted)]">{answeredCount} of {quiz.questions.length} answered</p></div><div className="flex gap-2"><Button disabled={pending} onClick={() => void save()} variant="secondary"><Save className="size-4" />Save</Button><Button disabled={pending || answeredCount !== quiz.questions.length} onClick={() => void submit()}><Send className="size-4" />Submit</Button></div></div>{quiz.questions.map((question) => <fieldset className="border border-[var(--border)] bg-[var(--surface)] p-5" key={question.id}><legend className="px-1 text-sm font-semibold text-[var(--text-strong)]">{question.position}. {question.prompt}</legend><div className="mt-3 space-y-2">{question.choices.map((choice) => <label className={`flex cursor-pointer items-start gap-3 border p-3 text-sm ${answers[question.id] === choice.id ? "border-[var(--brand-strong)] bg-[var(--brand-subtle)] text-[var(--text-strong)]" : "border-[var(--border)] text-[var(--text-muted)]"}`} key={choice.id}><input checked={answers[question.id] === choice.id} className="mt-0.5 size-4 accent-[var(--brand-strong)]" name={`question-${question.id}`} onChange={() => setAnswers((currentAnswers) => ({ ...currentAnswers, [question.id]: choice.id }))} type="radio" />{choice.label}</label>)}</div></fieldset>)}</section> : <section className="border border-[var(--border)] bg-[var(--surface)] p-6 text-center"><h2 className="text-lg font-semibold text-[var(--text-strong)]">{submitted.length ? "Attempt history" : "Ready to begin?"}</h2><p className="mx-auto mt-2 max-w-lg text-sm text-[var(--text-muted)]">Each submission is retained as a separate attempt. Your answers cannot be changed after submission.</p>{canStart ? <Button className="mt-5" disabled={pending} onClick={() => void start()}>{pending ? <Loader2 className="size-4 animate-spin" /> : null}{submitted.length ? "Start another attempt" : "Start quiz"}</Button> : null}</section>}
    {submitted.length ? <section><h2 className="text-lg font-semibold text-[var(--text-strong)]">Submitted attempts</h2><div className="mt-3 divide-y divide-[var(--border)] border border-[var(--border)] bg-[var(--surface)]">{submitted.map((item) => <article className="flex flex-wrap items-center justify-between gap-4 p-4" key={item.id}><div><p className="font-medium text-[var(--text-strong)]">Attempt {item.attempt_number}</p><p className="mt-1 text-xs text-[var(--text-muted)]">{item.submitted_at ? formatLearningDateTime(item.submitted_at) : "Submitted"}</p></div><div className="text-right"><p className="font-tabular text-lg font-semibold text-[var(--text-strong)]">{item.score_basis_points === null ? "—" : `${item.score_basis_points / 100}%`}</p><p className={`text-xs font-semibold ${item.passed ? "text-[var(--tone-success)]" : "text-[var(--text-muted)]"}`}>{item.passed ? "Passed" : "Not passed"}</p></div></article>)}</div></section> : null}
  </>;
}

function QuizEditorDrawer({ onClose, onSaved, open, quiz }: { onClose: () => void; onSaved: () => void; open: boolean; quiz: LearningQuiz }) {
  const [title, setTitle] = useState(quiz.title); const [instructions, setInstructions] = useState(quiz.instructions ?? ""); const [position, setPosition] = useState(quiz.position); const [opensAt, setOpensAt] = useState(toDateTimeLocal(quiz.opens_at)); const [closesAt, setClosesAt] = useState(toDateTimeLocal(quiz.closes_at)); const [attemptLimit, setAttemptLimit] = useState(quiz.attempt_limit); const [passMark, setPassMark] = useState(quiz.pass_score_basis_points / 100); const [saving, setSaving] = useState(false);
  useEffect(() => { if (open) { setTitle(quiz.title); setInstructions(quiz.instructions ?? ""); setPosition(quiz.position); setOpensAt(toDateTimeLocal(quiz.opens_at)); setClosesAt(toDateTimeLocal(quiz.closes_at)); setAttemptLimit(quiz.attempt_limit); setPassMark(quiz.pass_score_basis_points / 100); } }, [open, quiz]);
  const dirty = title !== quiz.title || instructions !== (quiz.instructions ?? "") || position !== quiz.position || opensAt !== toDateTimeLocal(quiz.opens_at) || closesAt !== toDateTimeLocal(quiz.closes_at) || attemptLimit !== quiz.attempt_limit || passMark !== quiz.pass_score_basis_points / 100;
  const submit = async (event: React.FormEvent) => { event.preventDefault(); if (saving || !dirty) return; setSaving(true); try { const response = await learningService.updateQuiz(quiz, { position, title: title.trim(), instructions: instructions.trim() || null, opens_at: opensAt ? new Date(opensAt).toISOString() : null, closes_at: closesAt ? new Date(closesAt).toISOString() : null, attempt_limit: attemptLimit, pass_score_basis_points: Math.round(passMark * 100) }); if (!response.success || !response.data) throw new Error(responseMessage(response, "Quiz could not be updated")); toast.success("Quiz updated"); onSaved(); } catch (saveError) { toast.error(saveError instanceof Error ? saveError.message : "Quiz could not be updated"); } finally { setSaving(false); } };
  return <GuardedDrawer dirty={dirty} discardDescription="The unsaved quiz changes will be lost." onClose={onClose} open={open} pending={saving} panelClassName="sm:max-w-[720px]">{(requestClose) => <><DialogHeader onClose={saving ? undefined : requestClose} title="Edit quiz" /><form onSubmit={submit}><DialogBody className="space-y-5"><div><Label htmlFor="edit-quiz-title">Title</Label><Input className="mt-1.5" data-autofocus="true" id="edit-quiz-title" maxLength={200} onChange={(event) => setTitle(event.target.value)} required value={title} /></div><div><Label htmlFor="edit-quiz-instructions">Instructions</Label><Textarea className="mt-1.5 min-h-32" id="edit-quiz-instructions" onChange={(event) => setInstructions(event.target.value)} value={instructions} /></div><div className="grid gap-5 sm:grid-cols-2"><div><Label htmlFor="edit-quiz-opens">Opens</Label><Input className="mt-1.5" id="edit-quiz-opens" onChange={(event) => setOpensAt(event.target.value)} type="datetime-local" value={opensAt} /></div><div><Label htmlFor="edit-quiz-closes">Closes</Label><Input className="mt-1.5" id="edit-quiz-closes" onChange={(event) => setClosesAt(event.target.value)} type="datetime-local" value={closesAt} /></div></div><div className="grid gap-5 sm:grid-cols-3"><div><Label htmlFor="edit-quiz-attempts">Attempts</Label><Input className="mt-1.5" id="edit-quiz-attempts" max={10} min={1} onChange={(event) => setAttemptLimit(Number(event.target.value))} type="number" value={attemptLimit} /></div><div><Label htmlFor="edit-quiz-pass">Pass mark (%)</Label><Input className="mt-1.5" id="edit-quiz-pass" max={100} min={0} onChange={(event) => setPassMark(Number(event.target.value))} step="0.01" type="number" value={passMark} /></div><div><Label htmlFor="edit-quiz-position">Position</Label><Input className="mt-1.5" id="edit-quiz-position" min={1} onChange={(event) => setPosition(Number(event.target.value))} type="number" value={position} /></div></div></DialogBody><DialogFooter><Button disabled={saving} onClick={requestClose} type="button" variant="secondary">Cancel</Button><Button disabled={saving || !dirty || !title.trim()} type="submit">{saving ? <Loader2 className="size-4 animate-spin" /> : null}Save changes</Button></DialogFooter></form></>}</GuardedDrawer>;
}

function QuestionDrawer({ onClose, onSaved, question, quiz }: { onClose: () => void; onSaved: () => void; question: LearningQuizQuestion | "new" | null; quiz: LearningQuiz }) {
  const record = question && question !== "new" ? question : null; const [position, setPosition] = useState(1); const [prompt, setPrompt] = useState(""); const [points, setPoints] = useState(1); const [choices, setChoices] = useState<Array<{ label: string; is_correct: boolean }>>([{ label: "", is_correct: true }, { label: "", is_correct: false }]); const [saving, setSaving] = useState(false);
  useEffect(() => { if (!question) return; setPosition(record?.position ?? quiz.questions.length + 1); setPrompt(record?.prompt ?? ""); setPoints(record?.points ?? 1); setChoices(record?.choices.map((choice) => ({ label: choice.label, is_correct: Boolean(choice.is_correct) })) ?? [{ label: "", is_correct: true }, { label: "", is_correct: false }]); }, [question, quiz.questions.length, record]);
  const payload: CreateLearningQuizQuestion = { position, prompt: prompt.trim(), points, choices: choices.map((choice) => ({ ...choice, label: choice.label.trim() })) }; const dirty = Boolean(question) && (!record || position !== record.position || prompt !== record.prompt || points !== record.points || JSON.stringify(payload.choices) !== JSON.stringify(record.choices.map((choice) => ({ label: choice.label, is_correct: Boolean(choice.is_correct) }))));
  const submit = async (event: React.FormEvent) => { event.preventDefault(); if (saving || choices.some((choice) => !choice.label.trim())) return; setSaving(true); try { const response = record ? await learningService.updateQuizQuestion(record, payload) : await learningService.createQuizQuestion(quiz.id, payload); if (!response.success || !response.data) throw new Error(responseMessage(response, "Question could not be saved")); toast.success(record ? "Question updated" : "Question added"); onSaved(); } catch (saveError) { toast.error(saveError instanceof Error ? saveError.message : "Question could not be saved"); } finally { setSaving(false); } };
  return <GuardedDrawer dirty={dirty} discardDescription="The unsaved question and choices will be lost." onClose={onClose} open={Boolean(question)} pending={saving} panelClassName="sm:max-w-[720px]">{(requestClose) => <><DialogHeader onClose={saving ? undefined : requestClose} title={record ? "Edit question" : "Add question"} /><form onSubmit={submit}><DialogBody className="space-y-5"><div><Label htmlFor="quiz-question-prompt">Question</Label><Textarea className="mt-1.5 min-h-28" data-autofocus="true" id="quiz-question-prompt" maxLength={4000} onChange={(event) => setPrompt(event.target.value)} required value={prompt} /></div><div className="grid gap-5 sm:grid-cols-2"><div><Label htmlFor="quiz-question-points">Points</Label><Input className="mt-1.5" id="quiz-question-points" max={1000} min={1} onChange={(event) => setPoints(Number(event.target.value))} type="number" value={points} /></div><div><Label htmlFor="quiz-question-position">Position</Label><Input className="mt-1.5" id="quiz-question-position" min={1} onChange={(event) => setPosition(Number(event.target.value))} type="number" value={position} /></div></div><fieldset><legend className="text-sm font-semibold text-[var(--text-strong)]">Choices</legend><div className="mt-3 space-y-3">{choices.map((choice, index) => <div className="flex items-center gap-2" key={index}><input aria-label={`Mark choice ${index + 1} correct`} checked={choice.is_correct} className="size-4 accent-[var(--brand-strong)]" name="correct-choice" onChange={() => setChoices((current) => current.map((item, itemIndex) => ({ ...item, is_correct: itemIndex === index })))} type="radio" /><Input aria-label={`Choice ${index + 1}`} maxLength={1000} onChange={(event) => setChoices((current) => current.map((item, itemIndex) => itemIndex === index ? { ...item, label: event.target.value } : item))} required value={choice.label} />{choices.length > 2 ? <Button aria-label={`Remove choice ${index + 1}`} onClick={() => setChoices((current) => { const next = current.filter((_, itemIndex) => itemIndex !== index); if (!next.some((item) => item.is_correct)) next[0] = { ...next[0], is_correct: true }; return next; })} size="icon" type="button" variant="ghost"><Trash2 className="size-4" /></Button> : null}</div>)}</div>{choices.length < 8 ? <Button className="mt-3" onClick={() => setChoices((current) => [...current, { label: "", is_correct: false }])} type="button" variant="ghost"><Plus className="size-4" />Add choice</Button> : null}</fieldset></DialogBody><DialogFooter><Button disabled={saving} onClick={requestClose} type="button" variant="secondary">Cancel</Button><Button disabled={saving || !dirty || !prompt.trim() || choices.some((choice) => !choice.label.trim())} type="submit">{saving ? <Loader2 className="size-4 animate-spin" /> : null}{record ? "Save changes" : "Add question"}</Button></DialogFooter></form></>}</GuardedDrawer>;
}

function QuizTransitionDrawer({ action, onClose, onCompleted, quiz }: { action: "publish" | "close" | null; onClose: () => void; onCompleted: (quiz: LearningQuiz) => void; quiz: LearningQuiz }) {
  const [reason, setReason] = useState(""); const [pending, setPending] = useState(false); useEffect(() => { if (action) setReason(""); }, [action]); if (!action) return null;
  const run = async () => { if (pending || (action === "close" && !reason.trim())) return; setPending(true); try { const response = action === "publish" ? await learningService.publishQuiz(quiz) : await learningService.closeQuiz(quiz, reason.trim()); if (!response.success || !response.data) throw new Error(responseMessage(response, `Quiz could not be ${action === "publish" ? "published" : "closed"}`)); toast.success(action === "publish" ? "Quiz published" : "Quiz closed"); onCompleted(response.data); } catch (transitionError) { toast.error(transitionError instanceof Error ? transitionError.message : "Quiz could not be updated"); } finally { setPending(false); } };
  return <DialogShell onClose={pending ? () => undefined : onClose} open><DialogHeader onClose={pending ? undefined : onClose} title={action === "publish" ? "Publish quiz?" : "Close quiz?"} /><DialogBody className="space-y-5"><div className="flex gap-4"><span className="flex size-10 shrink-0 items-center justify-center rounded-[9px] bg-[var(--badge-info-bg)] text-[var(--badge-info-text)]"><CheckCircle2 className="size-5" /></span><p className="text-sm leading-6 text-[var(--text-muted)]">{action === "publish" ? "Publishing freezes the questions, answer key, and eligible learner roster. Learners can then start attempts." : "Closing stops new attempts. Submitted attempts remain available."}</p></div>{action === "close" ? <div><Label htmlFor="quiz-close-reason">Reason</Label><Textarea className="mt-1.5 min-h-28" data-autofocus="true" id="quiz-close-reason" maxLength={2000} onChange={(event) => setReason(event.target.value)} required value={reason} /></div> : null}</DialogBody><DialogFooter><Button disabled={pending} onClick={onClose} type="button" variant="secondary">Cancel</Button><Button disabled={pending || (action === "close" && !reason.trim())} onClick={() => void run()} type="button">{pending ? <Loader2 className="size-4 animate-spin" /> : null}{pending ? "Updating…" : action === "publish" ? "Publish quiz" : "Close quiz"}</Button></DialogFooter></DialogShell>;
}

function toDateTimeLocal(value: string | null) { if (!value) return ""; const date = new Date(value); const local = new Date(date.getTime() - date.getTimezoneOffset() * 60_000); return local.toISOString().slice(0, 16); }

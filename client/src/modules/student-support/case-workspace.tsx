/** Full Student Support case record with governed drawer workflows. */

import { useCallback, useEffect, useState } from "react";
import { Link } from "@tanstack/react-router";
import { ArrowLeft, Edit3, MessageSquarePlus, UserPlus, X } from "lucide-react";
import toast from "react-hot-toast";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { ConfirmDrawer } from "@/components/ui/confirm-drawer";
import { DialogBody, DialogFooter, DialogHeader, DialogShell } from "@/components/ui/dialog";
import { Input, Label, Select, Textarea } from "@/components/ui/input";
import { usePageChrome } from "@/modules/admin/layouts/page-chrome";
import { useAuthStore } from "@/stores/auth-store";

import { responseMessage, studentSupportService } from "./service";
import type {
  CaseAction,
  CaseActionKind,
  CaseRecord,
  CaseSeverity,
  CaseStatus,
  CaseTeamMember,
  CaseTeamRole,
  ConcernCategory,
  StudentSupportReferences,
  UpdateCasePayload,
} from "./types";
import { displayValue, formatDate, formatDateTime, localDateTimeValue, severityTone, statusTone } from "./ui";

type Drawer = "edit" | "action" | "team" | "escalate" | "resolve" | "close" | null;

const categoryValues: ConcernCategory[] = ["wellbeing", "behaviour", "conduct", "safeguarding", "family", "learning_support", "other"];
const severityValues: CaseSeverity[] = ["low", "moderate", "high", "critical"];
const actionKinds: CaseActionKind[] = ["note", "contact", "meeting", "referral", "support_plan", "review"];

export function StudentSupportCaseWorkspace({ caseId }: { caseId: string }) {
  const permissions = useAuthStore((state) => state.user?.permissions ?? []);
  const canEdit = allowed(permissions, "student_support:edit");
  const canManage = allowed(permissions, "student_support:manage");
  const [record, setRecord] = useState<CaseRecord | null>(null);
  const [actions, setActions] = useState<CaseAction[]>([]);
  const [references, setReferences] = useState<StudentSupportReferences | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [drawer, setDrawer] = useState<Drawer>(null);
  const [removeMember, setRemoveMember] = useState<CaseTeamMember | null>(null);
  const [removing, setRemoving] = useState(false);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const [caseResponse, actionsResponse] = await Promise.all([
        studentSupportService.case(caseId),
        studentSupportService.actions(caseId),
      ]);
      if (!caseResponse.success || !caseResponse.data) throw new Error(responseMessage(caseResponse, "Case could not be loaded"));
      if (!actionsResponse.success || !actionsResponse.data) throw new Error(responseMessage(actionsResponse, "Case actions could not be loaded"));
      setRecord(caseResponse.data);
      setActions(actionsResponse.data.actions);
    } catch (loadError) {
      setError(loadError instanceof Error ? loadError.message : "Case could not be loaded");
    } finally {
      setLoading(false);
    }
  }, [caseId]);

  useEffect(() => { void load(); }, [load]);
  useEffect(() => {
    if (!canManage || (drawer !== "team" && references)) return;
    void studentSupportService.references().then((response) => {
      if (response.success && response.data) setReferences(response.data);
    });
  }, [canManage, drawer, references]);

  const active = record && !["resolved", "closed"].includes(record.status);
  usePageChrome(record?.reference ?? "Case record", record ? <div className="flex flex-wrap gap-2">
    {canEdit && active ? <Button onClick={() => setDrawer("action")} variant="secondary"><MessageSquarePlus className="size-4" />Add action</Button> : null}
    {canEdit && active ? <Button onClick={() => setDrawer("edit")}><Edit3 className="size-4" />Edit case</Button> : null}
  </div> : null);

  const refreshAfter = (next: CaseRecord) => {
    setRecord(next);
    setDrawer(null);
    void studentSupportService.actions(caseId).then((response) => { if (response.success && response.data) setActions(response.data.actions); });
  };

  const remove = async () => {
    if (!record || !removeMember || removing) return;
    setRemoving(true);
    try {
      const response = await studentSupportService.removeTeamMember(record.id, removeMember.user_id, record.version);
      if (!response.success || !response.data) throw new Error(responseMessage(response, "Team member could not be removed"));
      toast.success("Case-team member removed");
      setRecord(response.data);
      setRemoveMember(null);
    } catch (cause) {
      toast.error(cause instanceof Error ? cause.message : "Team member could not be removed");
      void load();
    } finally {
      setRemoving(false);
    }
  };

  if (loading) return <RecordLoading />;
  if (error || !record) return <RecordUnavailable description={error ?? "The case was not found or is outside your assigned scope."} onRetry={() => void load()} />;

  return <div className="space-y-6">
    <Link className="inline-flex items-center gap-2 text-sm font-medium text-[var(--text-muted)] hover:text-[var(--text-strong)]" to="/modules/student-support/cases"><ArrowLeft className="size-4" />Cases</Link>
    <section className="border border-[var(--border)] bg-[var(--surface)] p-5 sm:p-6">
      <div className="flex flex-col justify-between gap-5 lg:flex-row lg:items-start">
        <div className="min-w-0"><p className="font-tabular text-xs font-semibold uppercase tracking-[0.14em] text-[var(--brand-strong)]">{record.reference}</p><h1 className="mt-2 text-2xl font-semibold tracking-[-0.03em] text-[var(--text-strong)]">{record.title}</h1><p className="mt-3 max-w-3xl whitespace-pre-wrap text-sm leading-6 text-[var(--text-body)]">{record.summary}</p></div>
        <div className="flex shrink-0 flex-wrap gap-2"><Badge tone={severityTone(record.severity)}>{displayValue(record.severity)}</Badge><Badge tone={statusTone(record.status)}>{displayValue(record.status)}</Badge></div>
      </div>
      <div className="mt-6 grid gap-5 border-t border-[var(--border)] pt-5 sm:grid-cols-2 lg:grid-cols-4"><ReadValue label="Learner" value={record.learner_name} detail={record.learner_number} /><ReadValue label="Concern" value={displayValue(record.category)} /><ReadValue label="Occurred on" value={formatDate(record.occurred_on)} /><ReadValue label="Updated" value={formatDateTime(record.updated_at)} /></div>
    </section>

    {record.escalation_reason || record.resolution_summary || record.closure_reason ? <section className="grid gap-4 lg:grid-cols-3">{record.escalation_reason ? <DecisionCard label="Escalated" value={record.escalation_reason} at={record.escalated_at} /> : null}{record.resolution_summary ? <DecisionCard label="Resolved" value={record.resolution_summary} at={record.resolved_at} /> : null}{record.closure_reason ? <DecisionCard label="Closed" value={record.closure_reason} at={record.closed_at} /> : null}</section> : null}

    <div className="grid gap-6 xl:grid-cols-[minmax(0,1.4fr)_minmax(320px,0.8fr)]">
      <section className="border border-[var(--border)] bg-[var(--surface)]">
        <SectionHeader count={actions.length} title="Case actions" />
        {actions.length === 0 ? <EmptySection text="No case actions have been recorded." /> : <div className="divide-y divide-[var(--border-subtle)]">{actions.map((action) => <article className="p-5" key={action.id}><div className="flex flex-wrap items-center justify-between gap-3"><div className="flex items-center gap-2"><Badge tone="neutral">{displayValue(action.action_kind)}</Badge><p className="font-medium text-[var(--text-strong)]">{action.summary}</p></div><time className="text-xs text-[var(--text-muted)]">{formatDateTime(action.occurred_at)}</time></div>{action.details ? <p className="mt-3 whitespace-pre-wrap text-sm leading-6 text-[var(--text-body)]">{action.details}</p> : null}<p className="mt-3 text-xs text-[var(--text-muted)]">Recorded by {action.created_by_name} · {formatDateTime(action.created_at)}</p></article>)}</div>}
      </section>

      <div className="space-y-6">
        <section className="border border-[var(--border)] bg-[var(--surface)]">
          <div className="flex items-center justify-between gap-4 border-b border-[var(--border)] px-5 py-4"><div><h2 className="font-semibold text-[var(--text-strong)]">Case team</h2><p className="mt-1 text-xs text-[var(--text-muted)]">{record.team.length} active</p></div>{canManage && record.status !== "closed" ? <Button onClick={() => setDrawer("team")} size="sm" variant="secondary"><UserPlus className="size-4" />Assign</Button> : null}</div>
          <div className="divide-y divide-[var(--border-subtle)]">{record.team.map((member) => <div className="flex items-start justify-between gap-3 p-5" key={member.user_id}><div className="min-w-0"><div className="flex flex-wrap items-center gap-2"><p className="font-medium text-[var(--text-strong)]">{member.full_name}</p><Badge tone={member.member_role === "lead" ? "brand" : "neutral"}>{displayValue(member.member_role)}</Badge></div><p className="mt-1 truncate text-xs text-[var(--text-muted)]">{member.email}</p></div>{canManage && member.member_role !== "lead" && record.status !== "closed" ? <Button aria-label={`Remove ${member.full_name}`} onClick={() => setRemoveMember(member)} size="icon-sm" variant="ghost"><X className="size-4" /></Button> : null}</div>)}</div>
        </section>

        {canManage && record.status !== "closed" ? <section className="border border-[var(--border)] bg-[var(--surface)] p-5"><h2 className="font-semibold text-[var(--text-strong)]">Case status</h2><div className="mt-4 flex flex-wrap gap-2">{(record.status === "open" || record.status === "active") ? <Button onClick={() => setDrawer("escalate")} size="sm" variant="secondary">Escalate</Button> : null}{["open", "active", "escalated"].includes(record.status) ? <Button onClick={() => setDrawer("resolve")} size="sm">Resolve</Button> : null}{record.status === "resolved" ? <Button onClick={() => setDrawer("close")} size="sm">Close case</Button> : null}</div></section> : null}
      </div>
    </div>

    <section className="border border-[var(--border)] bg-[var(--surface)]"><SectionHeader count={record.history.length} title="Lifecycle history" />{record.history.length === 0 ? <EmptySection text="No lifecycle events are available." /> : <div className="divide-y divide-[var(--border-subtle)]">{record.history.map((event) => <div className="flex flex-col justify-between gap-2 p-4 sm:flex-row sm:items-center" key={event.id}><div><p className="text-sm font-medium text-[var(--text-strong)]">{displayValue(event.event_type.replace("student_support.case.", ""))}</p><p className="mt-1 text-xs text-[var(--text-muted)]">{event.actor_name}</p></div><time className="whitespace-nowrap text-xs text-[var(--text-muted)]">{formatDateTime(event.created_at)}</time></div>)}</div>}</section>

    <EditCaseDrawer onClose={() => setDrawer(null)} onSaved={refreshAfter} open={drawer === "edit"} record={record} />
    <ActionDrawer onClose={() => setDrawer(null)} onSaved={() => { setDrawer(null); void load(); }} open={drawer === "action"} record={record} />
    <TeamDrawer onClose={() => setDrawer(null)} onSaved={refreshAfter} open={drawer === "team"} record={record} references={references} />
    <TransitionDrawer action={drawer === "escalate" || drawer === "resolve" || drawer === "close" ? drawer : null} onClose={() => setDrawer(null)} onSaved={refreshAfter} record={record} />
    <ConfirmDrawer confirmLabel="Remove member" description={`Remove ${removeMember?.full_name ?? "this person"} from ${record.reference}? Their next case read will be denied.`} isPending={removing} onClose={() => setRemoveMember(null)} onConfirm={() => void remove()} open={removeMember !== null} title="Remove case-team member?" />
  </div>;
}

function EditCaseDrawer({ open, record, onClose, onSaved }: { open: boolean; record: CaseRecord; onClose: () => void; onSaved: (record: CaseRecord) => void }) {
  const [form, setForm] = useState<UpdateCasePayload>({ category: record.category, severity: record.severity, title: record.title, summary: record.summary, occurred_on: record.occurred_on, expected_version: record.version });
  const [saving, setSaving] = useState(false);
  useEffect(() => { if (open) setForm({ category: record.category, severity: record.severity, title: record.title, summary: record.summary, occurred_on: record.occurred_on, expected_version: record.version }); }, [open, record]);
  const submit = async (event: React.FormEvent) => { event.preventDefault(); setSaving(true); try { const response = await studentSupportService.updateCase(record.id, { ...form, title: form.title.trim(), summary: form.summary.trim() }); if (!response.success || !response.data) throw new Error(responseMessage(response, "Case could not be updated")); toast.success("Case updated"); onSaved(response.data); } catch (cause) { toast.error(cause instanceof Error ? cause.message : "Case could not be updated"); } finally { setSaving(false); } };
  return <DialogShell onClose={onClose} open={open} panelClassName="max-w-[680px]"><DialogHeader onClose={onClose} title={`Edit ${record.reference}`} /><form className="flex min-h-0 flex-1 flex-col overflow-hidden" onSubmit={(event) => void submit(event)}><DialogBody><div className="space-y-5"><div className="grid gap-4 sm:grid-cols-2"><Field label="Concern category"><Select data-autofocus="true" onChange={(event) => setForm({ ...form, category: event.target.value as ConcernCategory })} value={form.category}>{categoryValues.map((value) => <option key={value} value={value}>{displayValue(value)}</option>)}</Select></Field><Field label="Severity"><Select onChange={(event) => setForm({ ...form, severity: event.target.value as CaseSeverity })} value={form.severity}>{severityValues.map((value) => <option key={value} value={value}>{displayValue(value)}</option>)}</Select></Field></div><Field label="Title"><Input maxLength={200} onChange={(event) => setForm({ ...form, title: event.target.value })} required value={form.title} /></Field><Field label="Summary"><Textarea maxLength={6000} onChange={(event) => setForm({ ...form, summary: event.target.value })} required rows={8} value={form.summary} /></Field><Field label="Occurred on"><Input max={new Date().toISOString().slice(0, 10)} onChange={(event) => setForm({ ...form, occurred_on: event.target.value || null })} type="date" value={form.occurred_on ?? ""} /></Field></div></DialogBody><DialogFooter><Button onClick={onClose} type="button" variant="secondary">Cancel</Button><Button disabled={saving || !form.title.trim() || !form.summary.trim()} type="submit">{saving ? "Saving…" : "Save changes"}</Button></DialogFooter></form></DialogShell>;
}

function ActionDrawer({ open, record, onClose, onSaved }: { open: boolean; record: CaseRecord; onClose: () => void; onSaved: () => void }) {
  const [kind, setKind] = useState<CaseActionKind>("note"); const [summary, setSummary] = useState(""); const [details, setDetails] = useState(""); const [occurredAt, setOccurredAt] = useState(localDateTimeValue()); const [saving, setSaving] = useState(false);
  useEffect(() => { if (open) { setKind("note"); setSummary(""); setDetails(""); setOccurredAt(localDateTimeValue()); } }, [open]);
  const submit = async (event: React.FormEvent) => { event.preventDefault(); setSaving(true); try { const response = await studentSupportService.createAction(record.id, { action_kind: kind, summary: summary.trim(), details: details.trim() || null, occurred_at: new Date(occurredAt).toISOString(), expected_version: record.version }); if (!response.success) throw new Error(responseMessage(response, "Case action could not be recorded")); toast.success("Case action recorded"); onSaved(); } catch (cause) { toast.error(cause instanceof Error ? cause.message : "Case action could not be recorded"); } finally { setSaving(false); } };
  return <DialogShell onClose={onClose} open={open}><DialogHeader onClose={onClose} title="Record case action" /><form className="flex min-h-0 flex-1 flex-col overflow-hidden" onSubmit={(event) => void submit(event)}><DialogBody><div className="space-y-5"><Field label="Action"><Select data-autofocus="true" onChange={(event) => setKind(event.target.value as CaseActionKind)} value={kind}>{actionKinds.map((value) => <option key={value} value={value}>{displayValue(value)}</option>)}</Select></Field><Field label="Summary"><Input maxLength={300} onChange={(event) => setSummary(event.target.value)} required value={summary} /></Field><Field label="Details"><Textarea maxLength={6000} onChange={(event) => setDetails(event.target.value)} rows={7} value={details} /></Field><Field label="Occurred at"><Input max={localDateTimeValue()} onChange={(event) => setOccurredAt(event.target.value)} required type="datetime-local" value={occurredAt} /></Field></div></DialogBody><DialogFooter><Button onClick={onClose} type="button" variant="secondary">Cancel</Button><Button disabled={saving || !summary.trim() || !occurredAt} type="submit">{saving ? "Recording…" : "Record action"}</Button></DialogFooter></form></DialogShell>;
}

function TeamDrawer({ open, record, references, onClose, onSaved }: { open: boolean; record: CaseRecord; references: StudentSupportReferences | null; onClose: () => void; onSaved: (record: CaseRecord) => void }) {
  const [userId, setUserId] = useState(""); const [role, setRole] = useState<CaseTeamRole>("member"); const [saving, setSaving] = useState(false);
  useEffect(() => { if (open) { setUserId(""); setRole("member"); } }, [open]);
  const assigned = new Set(record.team.map((member) => member.user_id)); const candidates = references?.case_workers.filter((worker) => !assigned.has(worker.user_id)) ?? [];
  const submit = async (event: React.FormEvent) => { event.preventDefault(); setSaving(true); try { const response = await studentSupportService.assignTeamMember(record.id, userId, role, record.version); if (!response.success || !response.data) throw new Error(responseMessage(response, "Case-team member could not be assigned")); toast.success("Case-team member assigned"); onSaved(response.data); } catch (cause) { toast.error(cause instanceof Error ? cause.message : "Case-team member could not be assigned"); } finally { setSaving(false); } };
  return <DialogShell onClose={onClose} open={open}><DialogHeader onClose={onClose} title="Assign case-team member" /><form className="flex min-h-0 flex-1 flex-col overflow-hidden" onSubmit={(event) => void submit(event)}><DialogBody><div className="space-y-5"><Field label="Case Worker"><Select data-autofocus="true" disabled={!references} onChange={(event) => setUserId(event.target.value)} required value={userId}><option value="">Select a Case Worker</option>{candidates.map((worker) => <option key={worker.user_id} value={worker.user_id}>{worker.full_name} · {worker.email}</option>)}</Select></Field><Field label="Team role"><Select onChange={(event) => setRole(event.target.value as CaseTeamRole)} value={role}><option value="member">Member</option><option value="reviewer">Reviewer</option></Select></Field>{references && candidates.length === 0 ? <p className="text-sm text-[var(--text-muted)]">No additional Case Workers are available.</p> : null}</div></DialogBody><DialogFooter><Button onClick={onClose} type="button" variant="secondary">Cancel</Button><Button disabled={saving || !userId} type="submit">{saving ? "Assigning…" : "Assign member"}</Button></DialogFooter></form></DialogShell>;
}

function TransitionDrawer({ action, record, onClose, onSaved }: { action: "escalate" | "resolve" | "close" | null; record: CaseRecord; onClose: () => void; onSaved: (record: CaseRecord) => void }) {
  const [reason, setReason] = useState(""); const [saving, setSaving] = useState(false); useEffect(() => { if (action) setReason(""); }, [action]); if (!action) return null;
  const submit = async (event: React.FormEvent) => { event.preventDefault(); setSaving(true); try { const response = await studentSupportService.transition(record.id, action, reason.trim(), record.version); if (!response.success || !response.data) throw new Error(responseMessage(response, `Case could not be ${action}d`)); toast.success(action === "close" ? "Case closed" : action === "resolve" ? "Case resolved" : "Case escalated"); onSaved(response.data); } catch (cause) { toast.error(cause instanceof Error ? cause.message : `Case could not be ${action}d`); } finally { setSaving(false); } };
  const label = action === "close" ? "Closure reason" : action === "resolve" ? "Resolution summary" : "Escalation reason";
  return <DialogShell onClose={onClose} open><DialogHeader onClose={onClose} title={action === "close" ? "Close case" : action === "resolve" ? "Resolve case" : "Escalate case"} /><form className="flex min-h-0 flex-1 flex-col overflow-hidden" onSubmit={(event) => void submit(event)}><DialogBody><Field label={label}><Textarea data-autofocus="true" maxLength={6000} onChange={(event) => setReason(event.target.value)} required rows={8} value={reason} /></Field></DialogBody><DialogFooter><Button onClick={onClose} type="button" variant="secondary">Cancel</Button><Button disabled={saving || !reason.trim()} type="submit">{saving ? "Saving…" : action === "close" ? "Close case" : action === "resolve" ? "Resolve case" : "Escalate case"}</Button></DialogFooter></form></DialogShell>;
}

function ReadValue({ label, value, detail }: { label: string; value: string; detail?: string }) { return <div><p className="text-xs font-semibold uppercase tracking-[0.12em] text-[var(--text-subtle)]">{label}</p><p className="mt-1 text-sm font-medium text-[var(--text-strong)]">{value}</p>{detail ? <p className="mt-1 font-tabular text-xs text-[var(--text-muted)]">{detail}</p> : null}</div>; }
function DecisionCard({ label, value, at }: { label: string; value: string; at: string | null }) { return <article className="border border-[var(--border)] bg-[var(--surface)] p-5"><div className="flex items-center justify-between gap-3"><p className="font-semibold text-[var(--text-strong)]">{label}</p>{at ? <time className="text-xs text-[var(--text-muted)]">{formatDateTime(at)}</time> : null}</div><p className="mt-3 whitespace-pre-wrap text-sm leading-6 text-[var(--text-body)]">{value}</p></article>; }
function SectionHeader({ count, title }: { count: number; title: string }) { return <div className="flex items-center justify-between gap-4 border-b border-[var(--border)] px-5 py-4"><h2 className="font-semibold text-[var(--text-strong)]">{title}</h2><Badge tone="neutral">{count}</Badge></div>; }
function EmptySection({ text }: { text: string }) { return <p className="p-6 text-sm text-[var(--text-muted)]">{text}</p>; }
function Field({ label, children }: { label: string; children: React.ReactNode }) { return <div className="space-y-2"><Label>{label}</Label>{children}</div>; }
function RecordLoading() { return <div className="space-y-5"><div className="h-5 w-24 animate-pulse bg-[var(--surface-sunken)]" /><div className="h-56 animate-pulse bg-[var(--surface-sunken)]" /><div className="grid gap-6 xl:grid-cols-2"><div className="h-72 animate-pulse bg-[var(--surface-sunken)]" /><div className="h-72 animate-pulse bg-[var(--surface-sunken)]" /></div></div>; }
function RecordUnavailable({ description, onRetry }: { description: string; onRetry: () => void }) { return <div className="border border-[var(--border)] bg-[var(--surface)] p-8 text-center"><h1 className="text-lg font-semibold text-[var(--text-strong)]">Case unavailable</h1><p className="mx-auto mt-2 max-w-lg text-sm text-[var(--text-muted)]">{description}</p><Button className="mt-5" onClick={onRetry} variant="secondary">Try again</Button></div>; }
function allowed(permissions: string[], permission: string) { return permissions.includes("*") || permissions.includes(permission); }

export function availableCaseStatusActions(status: CaseStatus) {
  if (status === "resolved") return ["close"] as const;
  if (status === "open" || status === "active") return ["escalate", "resolve"] as const;
  if (status === "escalated") return ["resolve"] as const;
  return [] as const;
}

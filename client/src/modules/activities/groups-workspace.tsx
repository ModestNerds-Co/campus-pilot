/** Activity groups, canonical people assignments, consent, and lifecycle work. */

import { useCallback, useEffect, useState } from "react";
import { CalendarRange, Plus, Search, UserPlus, UsersRound } from "lucide-react";
import toast from "react-hot-toast";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Table, TableControlsBar, TableEmpty, TableError, TableLoading, TableScroll, TableWrap, TBody, TD, TH, THead, TR } from "@/components/ui/data-table";
import { DialogBody, DialogFooter, DialogHeader, DialogShell } from "@/components/ui/dialog";
import { Input, Label, Select, Textarea } from "@/components/ui/input";
import { usePageChrome } from "@/modules/admin/layouts/page-chrome";
import { useAuthStore } from "@/stores/auth-store";

import { activitiesService, responseMessage } from "./service";
import type {
  ActivitiesReferences, ActivityCatalogItem, ActivityConsentStatus, ActivityGroupRecord,
  ActivityGroupStatus, ActivityGroupSummary, ActivityLeader, ActivityLeaderRole,
  ActivityMembership, GroupPayload,
} from "./types";
import { allowed, displayValue, formatDate, formatDateTime, statusTone } from "./ui";

const today = () => new Date().toISOString().slice(0, 10);
const later = () => { const date = new Date(); date.setDate(date.getDate() + 90); return date.toISOString().slice(0, 10); };
const emptyGroup = (): GroupPayload => ({ activity_id: "", code: "", name: "", starts_on: today(), ends_on: later(), capacity: null, consent_required: false, consent_instructions: null });

export function ActivitiesGroupsWorkspace() {
  const permissions = useAuthStore((state) => state.user?.permissions ?? []);
  const canManage = allowed(permissions, "activities:manage");
  const [records, setRecords] = useState<ActivityGroupSummary[]>([]);
  const [search, setSearch] = useState("");
  const [status, setStatus] = useState<ActivityGroupStatus | "all">("active");
  const [selected, setSelected] = useState<ActivityGroupSummary | null>(null);
  const [createOpen, setCreateOpen] = useState(false);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    setLoading(true); setError(null);
    try {
      const response = await activitiesService.groups({ page: 1, per_page: 100, search: search.trim() || undefined, status: status === "all" ? undefined : status });
      if (!response.success || !response.data) throw new Error(responseMessage(response, "Activity groups could not be loaded"));
      setRecords(response.data);
    } catch (loadError) { setError(loadError instanceof Error ? loadError.message : "Activity groups could not be loaded"); }
    finally { setLoading(false); }
  }, [search, status]);

  useEffect(() => { void load(); }, [load]);
  usePageChrome("Groups", canManage ? <Button onClick={() => setCreateOpen(true)}><Plus className="size-4" />Create group</Button> : null);
  const filtered = search || status !== "active";

  return <div className="space-y-6">
    <TableControlsBar><Input aria-label="Search activity groups" className="sm:w-72" leadingIcon={<Search />} onChange={(event) => setSearch(event.target.value)} placeholder="Search group or activity" value={search} /><Select aria-label="Group status" className="sm:w-44" onChange={(event) => setStatus(event.target.value as ActivityGroupStatus | "all")} value={status}><option value="active">Active</option><option value="draft">Draft</option><option value="closed">Closed</option><option value="cancelled">Cancelled</option><option value="all">All statuses</option></Select></TableControlsBar>
    <TableWrap>{loading ? <TableLoading columns={6} label="Loading activity groups…" /> : error ? <TableError description={error} onRetry={() => void load()} /> : records.length === 0 ? <TableEmpty description={filtered ? "Change the current filters." : canManage ? "Create the first activity group." : "No activity groups are assigned to you."} icon={<UsersRound />} title={filtered ? "No groups match" : "No activity groups"} /> : <TableScroll><Table className="min-w-[980px]"><THead><tr><TH>Group</TH><TH>Activity</TH><TH>Dates</TH><TH>People</TH><TH>Sessions</TH><TH>Status</TH></tr></THead><TBody>{records.map((record) => <TR className="cursor-pointer" key={record.id} onClick={() => setSelected(record)}><TD><p className="font-medium text-[var(--text-strong)]">{record.name}</p><p className="mt-1 font-tabular text-xs text-[var(--text-muted)]">{record.code}</p></TD><TD className="text-[var(--text-muted)]">{record.activity_name}</TD><TD className="whitespace-nowrap text-[var(--text-muted)]">{formatDate(record.starts_on)} – {formatDate(record.ends_on)}</TD><TD className="text-[var(--text-muted)]">{record.member_count} learners · {record.leader_count} leaders</TD><TD className="font-tabular text-[var(--text-muted)]">{record.session_count}</TD><TD><Badge tone={statusTone(record.status)}>{displayValue(record.status)}</Badge></TD></TR>)}</TBody></Table></TableScroll>}</TableWrap>
    {canManage ? <GroupCreateDrawer onClose={() => setCreateOpen(false)} onSaved={() => { setCreateOpen(false); void load(); }} open={createOpen} /> : null}
    <GroupRecordDrawer canManage={canManage} onClose={() => setSelected(null)} onSaved={() => { setSelected(null); void load(); }} open={selected !== null} summary={selected} />
  </div>;
}

function GroupCreateDrawer({ onClose, onSaved, open }: { onClose: () => void; onSaved: () => void; open: boolean }) {
  const [catalog, setCatalog] = useState<ActivityCatalogItem[]>([]);
  const [form, setForm] = useState<GroupPayload>(emptyGroup());
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  useEffect(() => { if (!open) return; setForm(emptyGroup()); setLoading(true); void activitiesService.catalog({ status: "active" }).then((response) => { if (response.success && response.data) setCatalog(response.data); else toast.error(responseMessage(response, "Activity catalog could not be loaded")); }).finally(() => setLoading(false)); }, [open]);
  const save = async (event: React.FormEvent) => { event.preventDefault(); setSaving(true); try { const response = await activitiesService.createGroup(normalizeGroup(form)); if (!response.success) throw new Error(responseMessage(response, "Activity group could not be created")); toast.success("Activity group created"); onSaved(); } catch (saveError) { toast.error(saveError instanceof Error ? saveError.message : "Activity group could not be created"); } finally { setSaving(false); } };
  return <DialogShell onClose={onClose} open={open}><DialogHeader onClose={saving ? undefined : onClose} title="Create activity group" /><form className="flex min-h-0 flex-1 flex-col" onSubmit={(event) => void save(event)}><DialogBody><GroupFields catalog={catalog} disabled={loading} form={form} setForm={setForm} /></DialogBody><DialogFooter><Button onClick={onClose} type="button" variant="secondary">Cancel</Button><Button disabled={saving || loading || !form.activity_id || !form.code.trim() || !form.name.trim()} type="submit">{saving ? "Creating…" : "Create group"}</Button></DialogFooter></form></DialogShell>;
}

type GroupAction = "edit" | "activate" | "close" | "cancel" | "add_leader" | "end_leader" | "add_member" | "consent" | "end_member";

function GroupRecordDrawer({ canManage, onClose, onSaved, open, summary }: { canManage: boolean; onClose: () => void; onSaved: () => void; open: boolean; summary: ActivityGroupSummary | null }) {
  const [record, setRecord] = useState<ActivityGroupRecord | null>(null);
  const [catalog, setCatalog] = useState<ActivityCatalogItem[]>([]);
  const [references, setReferences] = useState<ActivitiesReferences | null>(null);
  const [action, setAction] = useState<GroupAction | null>(null);
  const [selectedLeader, setSelectedLeader] = useState<ActivityLeader | null>(null);
  const [selectedMember, setSelectedMember] = useState<ActivityMembership | null>(null);
  const [form, setForm] = useState<GroupPayload>(emptyGroup());
  const [reason, setReason] = useState("");
  const [effectiveDate, setEffectiveDate] = useState(today());
  const [leaderForm, setLeaderForm] = useState({ employee_id: "", role: "leader" as ActivityLeaderRole, starts_on: today(), ends_on: "" });
  const [learnerId, setLearnerId] = useState("");
  const [consentStatus, setConsentStatus] = useState<ActivityConsentStatus>("pending");
  const [consentNotes, setConsentNotes] = useState("");
  const [memberOutcome, setMemberOutcome] = useState<"ended" | "withdrawn">("ended");
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);

  const loadRecord = useCallback(async () => {
    if (!summary) return; setLoading(true);
    try {
      const [groupResponse, catalogResponse, referenceResponse] = await Promise.all([activitiesService.group(summary.id), canManage ? activitiesService.catalog({ status: "active" }) : Promise.resolve(null), canManage ? activitiesService.references() : Promise.resolve(null)]);
      if (!groupResponse.success || !groupResponse.data) throw new Error(responseMessage(groupResponse, "Activity group could not be loaded"));
      setRecord(groupResponse.data); setForm(groupPayload(groupResponse.data));
      if (catalogResponse?.success && catalogResponse.data) setCatalog(catalogResponse.data);
      if (referenceResponse?.success && referenceResponse.data) setReferences(referenceResponse.data);
    } catch (loadError) { toast.error(loadError instanceof Error ? loadError.message : "Activity group could not be loaded"); }
    finally { setLoading(false); }
  }, [canManage, summary]);

  useEffect(() => { if (!open) return; setAction(null); setSelectedLeader(null); setSelectedMember(null); setReason(""); setEffectiveDate(today()); setLeaderForm({ employee_id: "", role: "leader", starts_on: summary?.starts_on ?? today(), ends_on: "" }); setLearnerId(""); setConsentNotes(""); void loadRecord(); }, [loadRecord, open, summary]);

  const run = async () => {
    if (!record || !action) return; setSaving(true);
    try {
      let response;
      if (action === "edit") response = await activitiesService.updateGroup(record, normalizeGroup(form));
      else if (action === "activate") response = await activitiesService.activateGroup(record);
      else if (action === "close") response = await activitiesService.closeGroup(record, reason.trim());
      else if (action === "cancel") response = await activitiesService.cancelGroup(record, reason.trim());
      else if (action === "add_leader") response = await activitiesService.addLeader(record.id, { employee_id: leaderForm.employee_id, role: leaderForm.role, starts_on: leaderForm.starts_on, ends_on: leaderForm.ends_on || null });
      else if (action === "end_leader" && selectedLeader) response = await activitiesService.endLeader(record.id, selectedLeader, effectiveDate, reason.trim());
      else if (action === "add_member") response = await activitiesService.addMember(record.id, learnerId, effectiveDate);
      else if (action === "consent" && selectedMember) response = await activitiesService.updateMember(record.id, selectedMember, consentStatus, consentNotes.trim() || null);
      else if (action === "end_member" && selectedMember) response = await activitiesService.endMember(record.id, selectedMember, effectiveDate, memberOutcome, reason.trim());
      else return;
      if (!response.success) throw new Error(responseMessage(response, "Activity group could not be updated"));
      toast.success(actionMessage(action)); onSaved();
    } catch (actionError) { toast.error(actionError instanceof Error ? actionError.message : "Activity group could not be updated"); }
    finally { setSaving(false); }
  };

  const beginConsent = (member: ActivityMembership) => { setSelectedMember(member); setConsentStatus(member.consent_status); setConsentNotes(member.consent_notes ?? ""); setAction("consent"); };
  const beginEndMember = (member: ActivityMembership) => { setSelectedMember(member); setEffectiveDate(today()); setReason(""); setMemberOutcome("ended"); setAction("end_member"); };
  const beginEndLeader = (leader: ActivityLeader) => { setSelectedLeader(leader); setEffectiveDate(today()); setReason(""); setAction("end_leader"); };

  return <DialogShell onClose={onClose} open={open} panelClassName="max-w-[800px]"><DialogHeader onClose={saving ? undefined : onClose} title={summary?.name ?? "Activity group"} />
    {action && record ? <div className="flex min-h-0 flex-1 flex-col"><DialogBody>{action === "edit" ? <GroupFields catalog={catalog} form={form} setForm={setForm} /> : action === "activate" ? <ActionNotice>Activate this group. It must have at least one active employee leader and learner member.</ActionNotice> : action === "close" || action === "cancel" ? <div className="space-y-5"><ActionNotice>{action === "close" ? "Close this group after all scheduled sessions are completed or cancelled." : "Cancel this group and its scheduled sessions."}</ActionNotice><Field label={action === "close" ? "Closure reason" : "Cancellation reason"}><Textarea data-autofocus="true" maxLength={2000} onChange={(event) => setReason(event.target.value)} rows={6} value={reason} /></Field></div> : action === "add_leader" ? <div className="space-y-5"><Field label="Employee"><Select data-autofocus="true" onChange={(event) => setLeaderForm({ ...leaderForm, employee_id: event.target.value })} value={leaderForm.employee_id}><option value="">Select an active employee</option>{references?.employees.map((employee) => <option key={employee.id} value={employee.id}>{employee.display_name} · {employee.employee_number}</option>)}</Select></Field><Field label="Leader role"><Select onChange={(event) => setLeaderForm({ ...leaderForm, role: event.target.value as ActivityLeaderRole })} value={leaderForm.role}><option value="lead">Lead</option><option value="leader">Leader</option><option value="assistant">Assistant</option></Select></Field><div className="grid gap-4 sm:grid-cols-2"><Field label="Starts on"><Input min={record.starts_on} max={record.ends_on} onChange={(event) => setLeaderForm({ ...leaderForm, starts_on: event.target.value })} type="date" value={leaderForm.starts_on} /></Field><Field label="Ends on"><Input min={leaderForm.starts_on} max={record.ends_on} onChange={(event) => setLeaderForm({ ...leaderForm, ends_on: event.target.value })} type="date" value={leaderForm.ends_on} /></Field></div></div> : action === "end_leader" ? <EndFields date={effectiveDate} dateLabel="Ends on" max={record.ends_on} min={selectedLeader?.starts_on ?? record.starts_on} reason={reason} setDate={setEffectiveDate} setReason={setReason} /> : action === "add_member" ? <div className="space-y-5"><Field label="Learner"><Select data-autofocus="true" onChange={(event) => setLearnerId(event.target.value)} value={learnerId}><option value="">Select an active learner</option>{references?.learners.map((learner) => <option key={learner.id} value={learner.id}>{learner.display_name} · {learner.learner_number}</option>)}</Select></Field><Field label="Joined on"><Input min={record.starts_on} max={record.ends_on} onChange={(event) => setEffectiveDate(event.target.value)} type="date" value={effectiveDate} /></Field></div> : action === "consent" ? <div className="space-y-5"><p className="text-sm font-medium text-[var(--text-strong)]">{selectedMember?.learner_name}</p><Field label="Consent"><Select data-autofocus="true" onChange={(event) => setConsentStatus(event.target.value as ActivityConsentStatus)} value={consentStatus}>{record.consent_required ? <><option value="pending">Pending</option><option value="granted">Granted</option><option value="declined">Declined</option></> : <option value="not_required">Not required</option>}</Select></Field><Field label="Notes"><Textarea maxLength={3000} onChange={(event) => setConsentNotes(event.target.value)} rows={6} value={consentNotes} /></Field></div> : <div className="space-y-5"><Field label="Outcome"><Select onChange={(event) => setMemberOutcome(event.target.value as "ended" | "withdrawn")} value={memberOutcome}><option value="ended">Ended</option><option value="withdrawn">Withdrawn</option></Select></Field><EndFields date={effectiveDate} dateLabel="Ended on" max={record.ends_on} min={selectedMember?.joined_on ?? record.starts_on} reason={reason} setDate={setEffectiveDate} setReason={setReason} /></div>}</DialogBody><DialogFooter><Button onClick={() => setAction(null)} type="button" variant="secondary">Back</Button><Button disabled={saving || !actionReady(action, { form, reason, leaderForm, learnerId })} onClick={() => void run()} type="button" variant={action === "cancel" ? "destructive" : "default"}>{saving ? "Saving…" : actionButton(action)}</Button></DialogFooter></div> : <div className="flex min-h-0 flex-1 flex-col"><DialogBody>{loading || !record ? <div className="flex min-h-48 items-center justify-center text-sm text-[var(--text-muted)]">{loading ? "Loading group…" : "Group unavailable"}</div> : <div className="space-y-7"><div><div className="flex flex-wrap gap-2"><Badge tone={statusTone(record.status)}>{displayValue(record.status)}</Badge><Badge tone="neutral">{record.activity_name}</Badge></div><p className="mt-3 text-sm text-[var(--text-muted)]">{formatDate(record.starts_on)} – {formatDate(record.ends_on)} · {record.capacity ? `Capacity ${record.capacity}` : "No capacity limit"}</p>{record.consent_required ? <p className="mt-2 text-sm text-[var(--text-body)]">Consent required{record.consent_instructions ? ` · ${record.consent_instructions}` : ""}</p> : null}</div><Section label="Leaders" action={canManage && ["draft", "active"].includes(record.status) ? <Button onClick={() => setAction("add_leader")} size="sm" type="button" variant="secondary"><UserPlus className="size-4" />Assign leader</Button> : null}>{record.leaders.length ? <div className="divide-y divide-[var(--border)] border border-[var(--border)]">{record.leaders.map((leader) => <div className="flex items-center justify-between gap-4 p-4" key={leader.id}><div><p className="font-medium text-[var(--text-strong)]">{leader.employee_name}</p><p className="mt-1 text-xs text-[var(--text-muted)]">{displayValue(leader.role)} · {leader.employee_number} · {formatDate(leader.starts_on)}{leader.ends_on ? ` – ${formatDate(leader.ends_on)}` : ""}</p></div>{canManage && !leader.ended_at && ["draft", "active"].includes(record.status) ? <Button onClick={() => beginEndLeader(leader)} size="sm" type="button" variant="ghost">End</Button> : <Badge tone={leader.ended_at ? "neutral" : "success"}>{leader.ended_at ? "Ended" : "Active"}</Badge>}</div>)}</div> : <p className="text-sm text-[var(--text-muted)]">No leaders assigned.</p>}</Section><Section label="Learners" action={canManage && ["draft", "active"].includes(record.status) ? <Button onClick={() => { setEffectiveDate(record.starts_on); setAction("add_member"); }} size="sm" type="button" variant="secondary"><UserPlus className="size-4" />Add learner</Button> : null}>{record.memberships.length ? <div className="divide-y divide-[var(--border)] border border-[var(--border)]">{record.memberships.map((member) => <div className="flex flex-wrap items-center justify-between gap-4 p-4" key={member.id}><div><p className="font-medium text-[var(--text-strong)]">{member.learner_name}</p><p className="mt-1 text-xs text-[var(--text-muted)]">{member.learner_number} · joined {formatDate(member.joined_on)}</p></div><div className="flex items-center gap-2"><Badge tone={statusTone(member.consent_status)}>{displayValue(member.consent_status)}</Badge><Badge tone={statusTone(member.status)}>{displayValue(member.status)}</Badge>{canManage && member.status === "active" && ["draft", "active"].includes(record.status) ? <><Button onClick={() => beginConsent(member)} size="sm" type="button" variant="ghost">Consent</Button><Button onClick={() => beginEndMember(member)} size="sm" type="button" variant="ghost">End</Button></> : null}</div></div>)}</div> : <p className="text-sm text-[var(--text-muted)]">No learners added.</p>}</Section><Section label="History">{record.history.length ? <div className="space-y-3">{record.history.map((event) => <div className="border-l-2 border-[var(--border-strong)] pl-4" key={event.id}><p className="text-sm font-medium text-[var(--text-strong)]">{displayValue(event.event_type.replace("activities.group.", ""))}</p><p className="mt-1 text-xs text-[var(--text-muted)]">{event.actor_name} · {formatDateTime(event.created_at)}</p></div>)}</div> : <p className="text-sm text-[var(--text-muted)]">No lifecycle events recorded.</p>}</Section></div>}</DialogBody><DialogFooter>{canManage && record?.status === "draft" ? <><Button className="mr-auto" onClick={() => setAction("cancel")} type="button" variant="ghost">Cancel group</Button><Button onClick={() => setAction("edit")} type="button" variant="secondary">Edit</Button><Button onClick={() => setAction("activate")} type="button">Activate</Button></> : null}{canManage && record?.status === "active" ? <><Button className="mr-auto" onClick={() => setAction("cancel")} type="button" variant="ghost">Cancel group</Button><Button onClick={() => setAction("close")} type="button">Close group</Button></> : null}<Button onClick={onClose} type="button" variant="secondary">Close</Button></DialogFooter></div>}
  </DialogShell>;
}

function GroupFields({ catalog, disabled, form, setForm }: { catalog: ActivityCatalogItem[]; disabled?: boolean; form: GroupPayload; setForm: (value: GroupPayload) => void }) { return <div className="space-y-5"><Field label="Activity"><Select data-autofocus="true" disabled={disabled} onChange={(event) => setForm({ ...form, activity_id: event.target.value })} required value={form.activity_id}><option value="">Select an activity</option>{catalog.map((item) => <option key={item.id} value={item.id}>{item.name} · {item.code}</option>)}</Select></Field><div className="grid gap-4 sm:grid-cols-2"><Field label="Group code"><Input maxLength={24} onChange={(event) => setForm({ ...form, code: event.target.value })} required value={form.code} /></Field><Field label="Group name"><Input maxLength={160} onChange={(event) => setForm({ ...form, name: event.target.value })} required value={form.name} /></Field></div><div className="grid gap-4 sm:grid-cols-2"><Field label="Starts on"><Input onChange={(event) => setForm({ ...form, starts_on: event.target.value })} required type="date" value={form.starts_on} /></Field><Field label="Ends on"><Input min={form.starts_on} onChange={(event) => setForm({ ...form, ends_on: event.target.value })} required type="date" value={form.ends_on} /></Field></div><Field label="Capacity"><Input min={1} onChange={(event) => setForm({ ...form, capacity: event.target.value ? Number(event.target.value) : null })} placeholder="No limit" type="number" value={form.capacity ?? ""} /></Field><label className="flex items-start gap-3 border border-[var(--border)] p-4"><input checked={form.consent_required} className="mt-1" onChange={(event) => setForm({ ...form, consent_required: event.target.checked, consent_instructions: event.target.checked ? form.consent_instructions : null })} type="checkbox" /><span><span className="block text-sm font-medium text-[var(--text-strong)]">Consent required</span><span className="mt-1 block text-xs text-[var(--text-muted)]">Track consent on each learner membership.</span></span></label>{form.consent_required ? <Field label="Consent instructions"><Textarea maxLength={3000} onChange={(event) => setForm({ ...form, consent_instructions: event.target.value || null })} rows={5} value={form.consent_instructions ?? ""} /></Field> : null}</div>; }
function EndFields({ date, dateLabel, max, min, reason, setDate, setReason }: { date: string; dateLabel: string; max: string; min: string; reason: string; setDate: (value: string) => void; setReason: (value: string) => void }) { return <div className="space-y-5"><Field label={dateLabel}><Input data-autofocus="true" min={min} max={max} onChange={(event) => setDate(event.target.value)} type="date" value={date} /></Field><Field label="Reason"><Textarea maxLength={1000} onChange={(event) => setReason(event.target.value)} rows={6} value={reason} /></Field></div>; }
function Section({ action, children, label }: { action?: React.ReactNode; children: React.ReactNode; label: string }) { return <section className="border-t border-[var(--border)] pt-5"><div className="mb-3 flex items-center justify-between gap-3"><h4 className="text-xs font-semibold uppercase tracking-[0.14em] text-[var(--text-muted)]">{label}</h4>{action}</div>{children}</section>; }
function Field({ children, label }: { children: React.ReactNode; label: string }) { return <div className="space-y-2"><Label>{label}</Label>{children}</div>; }
function ActionNotice({ children }: { children: React.ReactNode }) { return <div className="border border-[var(--border)] bg-[var(--surface-sunken)] p-4 text-sm text-[var(--text-body)]">{children}</div>; }
function normalizeGroup(form: GroupPayload): GroupPayload { return { ...form, code: form.code.trim(), name: form.name.trim(), consent_instructions: form.consent_instructions?.trim() || null }; }
function groupPayload(record: ActivityGroupRecord): GroupPayload { return { activity_id: record.activity_id, code: record.code, name: record.name, starts_on: record.starts_on, ends_on: record.ends_on, capacity: record.capacity, consent_required: record.consent_required, consent_instructions: record.consent_instructions }; }
function actionReady(action: GroupAction, values: { form: GroupPayload; reason: string; leaderForm: { employee_id: string }; learnerId: string }) { if (action === "edit") return Boolean(values.form.activity_id && values.form.code.trim() && values.form.name.trim()); if (["close", "cancel", "end_leader", "end_member"].includes(action)) return Boolean(values.reason.trim()); if (action === "add_leader") return Boolean(values.leaderForm.employee_id); if (action === "add_member") return Boolean(values.learnerId); return true; }
function actionButton(action: GroupAction) { return ({ edit: "Save changes", activate: "Activate group", close: "Close group", cancel: "Cancel group", add_leader: "Assign leader", end_leader: "End assignment", add_member: "Add learner", consent: "Save consent", end_member: "End membership" } as const)[action]; }
function actionMessage(action: GroupAction) { return ({ edit: "Activity group updated", activate: "Activity group activated", close: "Activity group closed", cancel: "Activity group cancelled", add_leader: "Activity leader assigned", end_leader: "Activity leader assignment ended", add_member: "Learner added", consent: "Consent updated", end_member: "Membership ended" } as const)[action]; }

// Scoped Internal Audit engagement fieldwork workspace.

import { useCallback, useEffect, useState } from "react";
import { Link } from "@tanstack/react-router";
import { ArrowLeft, FileCheck2, FileSearch, Pencil, Plus, ShieldAlert, Trash2 } from "lucide-react";
import toast from "react-hot-toast";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { ConfirmDrawer } from "@/components/ui/confirm-drawer";
import { Table, TableEmpty, TableError, TableLoading, TableScroll, TableWrap, TBody, TD, TH, THead, TR } from "@/components/ui/data-table";
import { DialogBody, DialogFooter, DialogHeader, DialogShell } from "@/components/ui/dialog";
import { Input, Label, Select, Textarea } from "@/components/ui/input";
import { usePageChrome } from "@/modules/admin/layouts/page-chrome";
import { documentRegistryService } from "@/modules/document-registry/service";
import type { RegistryFile } from "@/modules/document-registry/types";
import { useAuthStore } from "@/stores/auth-store";

import { internalAuditService, responseMessage } from "./service";
import type { AuditEngagement, AuditEvidence, AuditFinding, AuditorCandidate, EngagementPayload, FindingPayload, FindingRating } from "./types";
import { allowed, dateTime, dateValue, label, tone } from "./ui";

type Transition = "start" | "reporting" | null;
type FindingDrawerState = { kind: "create"; record: null } | { kind: "edit"; record: AuditFinding } | null;

export function InternalAuditEngagementDetailWorkspace({ engagementId }: { engagementId: string }) {
  const permissions = useAuthStore((state) => state.user?.permissions ?? []);
  const canCreate = allowed(permissions, "internal_audit:create");
  const canEdit = allowed(permissions, "internal_audit:edit");
  const canDelete = allowed(permissions, "internal_audit:delete");
  const canIssue = allowed(permissions, "internal_audit:issue");
  const canManage = allowed(permissions, "internal_audit:manage");
  const [record, setRecord] = useState<AuditEngagement | null>(null);
  const [evidence, setEvidence] = useState<AuditEvidence[]>([]);
  const [findings, setFindings] = useState<AuditFinding[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [editOpen, setEditOpen] = useState(false);
  const [evidenceOpen, setEvidenceOpen] = useState(false);
  const [findingDrawer, setFindingDrawer] = useState<FindingDrawerState>(null);
  const [transition, setTransition] = useState<Transition>(null);
  const [closeOpen, setCloseOpen] = useState(false);
  const [deleteEngagementOpen, setDeleteEngagementOpen] = useState(false);
  const [issueFinding, setIssueFinding] = useState<AuditFinding | null>(null);
  const [deleteFinding, setDeleteFinding] = useState<AuditFinding | null>(null);
  const [pending, setPending] = useState(false);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const [engagementResponse, evidenceResponse, findingResponse] = await Promise.all([
        internalAuditService.engagement(engagementId),
        internalAuditService.evidence(engagementId),
        internalAuditService.findings({ engagement_id: engagementId, per_page: 100 }),
      ]);
      if (!engagementResponse.success || !engagementResponse.data) throw new Error(responseMessage(engagementResponse, "Engagement could not be loaded"));
      if (!evidenceResponse.success || !evidenceResponse.data) throw new Error(responseMessage(evidenceResponse, "Audit evidence could not be loaded"));
      if (!findingResponse.success || !findingResponse.data) throw new Error(responseMessage(findingResponse, "Findings could not be loaded"));
      setRecord(engagementResponse.data);
      setEvidence(evidenceResponse.data.evidence);
      setFindings(findingResponse.data.findings);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : "Engagement could not be loaded");
    } finally {
      setLoading(false);
    }
  }, [engagementId]);

  useEffect(() => { void load(); }, [load]);
  usePageChrome(record?.reference ?? "Engagement");
  if (loading) return <TableLoading columns={1} label="Loading audit engagement…" />;
  if (error || !record) return <TableError description={error ?? "Engagement unavailable"} onRetry={() => void load()} />;

  const transitionRecord = async () => {
    if (!transition || pending) return;
    setPending(true);
    const response = transition === "start" ? await internalAuditService.startEngagement(record) : await internalAuditService.beginReporting(record);
    setPending(false);
    if (response.success) { toast.success(transition === "start" ? "Fieldwork started" : "Engagement moved to reporting"); setTransition(null); void load(); }
    else toast.error(responseMessage(response, "Engagement could not be updated"));
  };
  const removeEngagement = async () => {
    if (pending) return;
    setPending(true);
    const response = await internalAuditService.deleteEngagement(record);
    setPending(false);
    if (response.success) window.location.assign("/modules/internal-audit");
    else toast.error(responseMessage(response, "Engagement could not be deleted"));
  };
  const issue = async () => {
    if (!issueFinding || pending) return;
    setPending(true);
    const response = await internalAuditService.issueFinding(issueFinding);
    setPending(false);
    if (response.success) { toast.success("Finding issued"); setIssueFinding(null); void load(); }
    else toast.error(responseMessage(response, "Finding could not be issued"));
  };
  const removeFinding = async () => {
    if (!deleteFinding || pending) return;
    setPending(true);
    const response = await internalAuditService.deleteFinding(deleteFinding);
    setPending(false);
    if (response.success) { toast.success("Draft finding deleted"); setDeleteFinding(null); void load(); }
    else toast.error(responseMessage(response, "Finding could not be deleted"));
  };

  return <div className="space-y-6">
    <div className="flex flex-wrap items-center justify-between gap-3"><Link className="inline-flex items-center gap-2 text-sm font-medium text-[var(--text-muted)] hover:text-[var(--text-strong)]" params={{ moduleKey: "internal-audit" }} to="/modules/$moduleKey"><ArrowLeft className="size-4" />Engagements</Link><div className="flex flex-wrap gap-2">
      {record.status === "planned" && canEdit ? <Button onClick={() => setEditOpen(true)} variant="secondary"><Pencil className="size-4" />Edit</Button> : null}
      {record.status === "planned" && canEdit ? <Button onClick={() => setTransition("start")}>Start fieldwork</Button> : null}
      {record.status === "fieldwork" && canEdit ? <Button onClick={() => setTransition("reporting")}>Begin reporting</Button> : null}
      {record.status === "reporting" && canManage ? <Button onClick={() => setCloseOpen(true)}>Close engagement</Button> : null}
      {record.status === "planned" && canDelete ? <Button aria-label="Delete engagement" onClick={() => setDeleteEngagementOpen(true)} variant="ghost"><Trash2 className="size-4" /></Button> : null}
    </div></div>

    <Card><CardHeader><div className="flex flex-wrap items-center justify-between gap-3"><div><CardTitle>{record.title}</CardTitle><p className="mt-1 text-sm text-[var(--text-muted)]">{record.plan_reference} · {record.plan_title}</p></div><Badge tone={tone(record.status)}>{label(record.status)}</Badge></div></CardHeader><CardContent><dl className="grid gap-5 md:grid-cols-2 xl:grid-cols-4"><Fact label="Lead auditor" value={record.lead_auditor_name} detail={record.lead_auditor_email} /><Fact label="Dates" value={`${dateValue(record.starts_on)} – ${dateValue(record.due_on)}`} /><Fact label="Evidence" value={String(record.evidence_count)} /><Fact label="Findings" value={String(record.finding_count)} /></dl><div className="mt-6 grid gap-5 lg:grid-cols-2"><TextBlock label="Objective" value={record.objective} /><TextBlock label="Scope" value={record.scope_text} /></div>{record.close_summary ? <div className="mt-5"><TextBlock label="Closure summary" value={record.close_summary} /></div> : null}</CardContent></Card>

    <section className="space-y-3"><div className="flex items-center justify-between gap-3"><div><h2 className="text-lg font-semibold text-[var(--text-strong)]">Evidence</h2><p className="text-sm text-[var(--text-muted)]">Governed records linked from Document Registry.</p></div>{record.status !== "closed" && canCreate ? <Button onClick={() => setEvidenceOpen(true)} size="sm" variant="secondary"><Plus className="size-4" />Link evidence</Button> : null}</div><TableWrap>{evidence.length === 0 ? <TableEmpty description="Link a filed document when it supports this engagement." icon={<FileSearch />} title="No evidence linked" /> : <TableScroll><Table className="min-w-[760px]"><THead><tr><TH>Document</TH><TH>Sensitivity</TH><TH>Purpose</TH><TH>Linked</TH></tr></THead><TBody>{evidence.map((item) => <TR key={item.id}><TD><Link className="font-semibold text-[var(--brand-strong)] hover:underline" params={{ documentId: item.document_file_id }} to="/modules/document-registry/documents/$documentId">{item.document_reference}</Link><p className="mt-1 text-sm">{item.document_title}</p></TD><TD><Badge tone="neutral">{label(item.document_sensitivity)}</Badge></TD><TD className="max-w-96">{item.purpose}</TD><TD>{dateTime(item.linked_at)}</TD></TR>)}</TBody></Table></TableScroll>}</TableWrap></section>

    <section className="space-y-3"><div className="flex items-center justify-between gap-3"><div><h2 className="text-lg font-semibold text-[var(--text-strong)]">Findings</h2><p className="text-sm text-[var(--text-muted)]">Observations identified during fieldwork.</p></div>{["fieldwork", "reporting"].includes(record.status) && canCreate ? <Button onClick={() => setFindingDrawer({ kind: "create", record: null })} size="sm"><Plus className="size-4" />New finding</Button> : null}</div><TableWrap>{findings.length === 0 ? <TableEmpty description="No findings have been recorded for this engagement." icon={<ShieldAlert />} title="No findings" /> : <TableScroll><Table className="min-w-[900px]"><THead><tr><TH>Finding</TH><TH>Rating</TH><TH>Status</TH><TH className="text-right">Actions</TH></tr></THead><TBody>{findings.map((item) => <TR key={item.id}><TD><p className="font-semibold">{item.reference}</p><p className="mt-1 max-w-96 text-sm">{item.title}</p></TD><TD><Badge tone={tone(item.rating)}>{label(item.rating)}</Badge></TD><TD><Badge tone={tone(item.status)}>{label(item.status)}</Badge></TD><TD><div className="flex justify-end gap-2">{item.status === "draft" && canEdit ? <Button aria-label={`Edit ${item.reference}`} onClick={() => setFindingDrawer({ kind: "edit", record: item })} size="sm" variant="ghost"><Pencil className="size-4" /></Button> : null}{item.status === "draft" && canIssue ? <Button onClick={() => setIssueFinding(item)} size="sm" variant="secondary"><FileCheck2 className="size-4" />Issue</Button> : null}{item.status === "draft" && canDelete ? <Button aria-label={`Delete ${item.reference}`} onClick={() => setDeleteFinding(item)} size="sm" variant="ghost"><Trash2 className="size-4" /></Button> : null}</div></TD></TR>)}</TBody></Table></TableScroll>}</TableWrap></section>

    <EngagementEditDrawer onClose={() => setEditOpen(false)} onSaved={() => { setEditOpen(false); void load(); }} open={editOpen} record={record} />
    <EvidenceDrawer engagementId={record.id} onClose={() => setEvidenceOpen(false)} onSaved={() => { setEvidenceOpen(false); void load(); }} open={evidenceOpen} />
    <FindingDrawer engagementId={record.id} onClose={() => setFindingDrawer(null)} onSaved={() => { setFindingDrawer(null); void load(); }} state={findingDrawer} />
    <CloseDrawer onClose={() => setCloseOpen(false)} onSaved={() => { setCloseOpen(false); void load(); }} open={closeOpen} record={record} />
    <ConfirmDrawer confirmLabel={transition === "start" ? "Start fieldwork" : "Begin reporting"} description={transition === "start" ? "The engagement will move from planning into active fieldwork." : "Fieldwork will end and the engagement will move into reporting."} isPending={pending} onClose={() => setTransition(null)} onConfirm={() => void transitionRecord()} open={transition !== null} title={transition === "start" ? "Start this engagement?" : "Move to reporting?"} />
    <ConfirmDrawer confirmLabel="Delete engagement" description={`Delete ${record.reference}? Only planned engagements without evidence or findings can be deleted.`} isPending={pending} onClose={() => setDeleteEngagementOpen(false)} onConfirm={() => void removeEngagement()} open={deleteEngagementOpen} title="Delete planned engagement?" />
    <ConfirmDrawer confirmLabel="Issue finding" description={`Issue ${issueFinding?.reference ?? "this finding"}? Issued findings can no longer be edited.`} isPending={pending} onClose={() => setIssueFinding(null)} onConfirm={() => void issue()} open={issueFinding !== null} title="Issue audit finding?" />
    <ConfirmDrawer confirmLabel="Delete finding" description={`Delete ${deleteFinding?.reference ?? "this draft finding"}?`} isPending={pending} onClose={() => setDeleteFinding(null)} onConfirm={() => void removeFinding()} open={deleteFinding !== null} title="Delete draft finding?" />
  </div>;
}

function EngagementEditDrawer({ open, record, onClose, onSaved }: { open: boolean; record: AuditEngagement; onClose: () => void; onSaved: () => void }) {
  const permissions = useAuthStore((state) => state.user?.permissions ?? []);
  const canManage = allowed(permissions, "internal_audit:manage");
  const [auditors, setAuditors] = useState<AuditorCandidate[]>([]);
  const [form, setForm] = useState<EngagementPayload>(() => engagementValues(record));
  const [saving, setSaving] = useState(false);
  useEffect(() => { if (!open) return; setForm(engagementValues(record)); if (canManage) void internalAuditService.auditors().then((response) => { if (response.success && response.data) setAuditors(response.data); }); }, [canManage, open, record]);
  const submit = async (event: React.FormEvent) => { event.preventDefault(); setSaving(true); const response = await internalAuditService.updateEngagement(record, form); setSaving(false); if (response.success) { toast.success("Engagement updated"); onSaved(); } else toast.error(responseMessage(response, "Engagement could not be updated")); };
  return <DialogShell onClose={onClose} open={open}><form onSubmit={(event) => void submit(event)}><DialogHeader onClose={onClose} title={`Edit ${record.reference}`} /><DialogBody><div className="space-y-5"><Field label="Title"><Input data-autofocus="true" maxLength={200} onChange={(event) => setForm({ ...form, title: event.target.value })} required value={form.title} /></Field><Field label="Objective"><Textarea maxLength={4000} onChange={(event) => setForm({ ...form, objective: event.target.value })} required rows={4} value={form.objective} /></Field><Field label="Scope"><Textarea maxLength={6000} onChange={(event) => setForm({ ...form, scope_text: event.target.value })} required rows={5} value={form.scope_text} /></Field><Field label="Lead auditor">{canManage ? <Select onChange={(event) => setForm({ ...form, lead_auditor_user_id: event.target.value })} required value={form.lead_auditor_user_id}><option value={record.lead_auditor_user_id}>{record.lead_auditor_name}</option>{auditors.filter((item) => item.user_id !== record.lead_auditor_user_id).map((item) => <option key={item.user_id} value={item.user_id}>{item.full_name} · {item.email}</option>)}</Select> : <Input disabled value={`${record.lead_auditor_name} · ${record.lead_auditor_email}`} />}</Field><div className="grid gap-5 sm:grid-cols-2"><Field label="Starts"><Input onChange={(event) => setForm({ ...form, starts_on: event.target.value })} required type="date" value={form.starts_on} /></Field><Field label="Due"><Input min={form.starts_on} onChange={(event) => setForm({ ...form, due_on: event.target.value })} required type="date" value={form.due_on} /></Field></div></div></DialogBody><DialogFooter><Button onClick={onClose} type="button" variant="secondary">Cancel</Button><Button disabled={saving} type="submit">{saving ? "Saving…" : "Save changes"}</Button></DialogFooter></form></DialogShell>;
}

function EvidenceDrawer({ engagementId, open, onClose, onSaved }: { engagementId: string; open: boolean; onClose: () => void; onSaved: () => void }) {
  const [documents, setDocuments] = useState<RegistryFile[]>([]);
  const [documentId, setDocumentId] = useState("");
  const [purpose, setPurpose] = useState("");
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  useEffect(() => { if (!open) return; setDocumentId(""); setPurpose(""); setLoading(true); void documentRegistryService.files({ status: "filed", per_page: 100 }).then((response) => { if (response.success && response.data) setDocuments(response.data.files); else toast.error("Filed documents could not be loaded"); }).finally(() => setLoading(false)); }, [open]);
  const submit = async (event: React.FormEvent) => { event.preventDefault(); setSaving(true); const response = await internalAuditService.linkEvidence(engagementId, documentId, purpose.trim()); setSaving(false); if (response.success) { toast.success("Evidence linked"); onSaved(); } else toast.error(responseMessage(response, "Evidence could not be linked")); };
  return <DialogShell onClose={onClose} open={open}><form onSubmit={(event) => void submit(event)}><DialogHeader onClose={onClose} title="Link audit evidence" /><DialogBody><div className="space-y-5"><Field label="Filed document"><Select data-autofocus="true" disabled={loading} onChange={(event) => setDocumentId(event.target.value)} required value={documentId}><option value="">Choose document</option>{documents.map((item) => <option key={item.id} value={item.id}>{item.reference} · {item.title}</option>)}</Select>{!loading && documents.length === 0 ? <p className="text-xs text-[var(--text-muted)]">No filed documents are available in Document Registry.</p> : null}</Field><Field label="Purpose"><Textarea maxLength={2000} onChange={(event) => setPurpose(event.target.value)} placeholder="How this record supports the audit" required rows={6} value={purpose} /></Field></div></DialogBody><DialogFooter><Button onClick={onClose} type="button" variant="secondary">Cancel</Button><Button disabled={saving || !documentId || !purpose.trim()} type="submit">{saving ? "Linking…" : "Link evidence"}</Button></DialogFooter></form></DialogShell>;
}

function FindingDrawer({ engagementId, state, onClose, onSaved }: { engagementId: string; state: FindingDrawerState; onClose: () => void; onSaved: () => void }) {
  const [form, setForm] = useState<FindingPayload>(() => blankFinding());
  const [saving, setSaving] = useState(false);
  useEffect(() => { if (!state) return; setForm(state.record ? findingValues(state.record) : blankFinding()); }, [state]);
  if (!state) return null;
  const submit = async (event: React.FormEvent) => { event.preventDefault(); setSaving(true); const response = state.kind === "edit" ? await internalAuditService.updateFinding(state.record, form) : await internalAuditService.createFinding(engagementId, form); setSaving(false); if (response.success) { toast.success(state.kind === "edit" ? "Finding updated" : `Created ${response.data?.reference ?? "finding"}`); onSaved(); } else toast.error(responseMessage(response, "Finding could not be saved")); };
  return <DialogShell onClose={onClose} open><form onSubmit={(event) => void submit(event)}><DialogHeader onClose={onClose} title={state.kind === "edit" ? `Edit ${state.record.reference}` : "New audit finding"} /><DialogBody><div className="space-y-5"><Field label="Title"><Input data-autofocus="true" maxLength={240} onChange={(event) => setForm({ ...form, title: event.target.value })} required value={form.title} /></Field><Field label="Rating"><Select onChange={(event) => setForm({ ...form, rating: event.target.value as FindingRating })} value={form.rating}><option value="low">Low</option><option value="moderate">Moderate</option><option value="high">High</option><option value="critical">Critical</option></Select></Field><Field label="Criteria"><Textarea maxLength={6000} onChange={(event) => setForm({ ...form, criteria: event.target.value })} required rows={4} value={form.criteria} /></Field><Field label="Condition"><Textarea maxLength={6000} onChange={(event) => setForm({ ...form, condition: event.target.value })} required rows={4} value={form.condition} /></Field><Field label="Risk or effect"><Textarea maxLength={6000} onChange={(event) => setForm({ ...form, risk_effect: event.target.value })} required rows={4} value={form.risk_effect} /></Field><Field label="Recommendation"><Textarea maxLength={6000} onChange={(event) => setForm({ ...form, recommendation: event.target.value })} required rows={4} value={form.recommendation} /></Field></div></DialogBody><DialogFooter><Button onClick={onClose} type="button" variant="secondary">Cancel</Button><Button disabled={saving || !findingReady(form)} type="submit">{saving ? "Saving…" : "Save finding"}</Button></DialogFooter></form></DialogShell>;
}

function CloseDrawer({ open, record, onClose, onSaved }: { open: boolean; record: AuditEngagement; onClose: () => void; onSaved: () => void }) { const [summary, setSummary] = useState(""); const [saving, setSaving] = useState(false); useEffect(() => { if (open) setSummary(""); }, [open]); const submit = async (event: React.FormEvent) => { event.preventDefault(); setSaving(true); const response = await internalAuditService.closeEngagement(record, summary.trim()); setSaving(false); if (response.success) { toast.success("Engagement closed"); onSaved(); } else toast.error(responseMessage(response, "Engagement could not be closed")); }; return <DialogShell onClose={onClose} open={open}><form onSubmit={(event) => void submit(event)}><DialogHeader onClose={onClose} title={`Close ${record.reference}`} /><DialogBody><Field label="Closure summary"><Textarea data-autofocus="true" maxLength={4000} onChange={(event) => setSummary(event.target.value)} required rows={8} value={summary} /></Field></DialogBody><DialogFooter><Button onClick={onClose} type="button" variant="secondary">Cancel</Button><Button disabled={saving || !summary.trim()} type="submit">{saving ? "Closing…" : "Close engagement"}</Button></DialogFooter></form></DialogShell>; }

function engagementValues(record: AuditEngagement): EngagementPayload { return { plan_id: record.plan_id, title: record.title, objective: record.objective, scope_text: record.scope_text, lead_auditor_user_id: record.lead_auditor_user_id, starts_on: record.starts_on, due_on: record.due_on }; }
function blankFinding(): FindingPayload { return { title: "", rating: "moderate", criteria: "", condition: "", risk_effect: "", recommendation: "" }; }
function findingValues(record: AuditFinding): FindingPayload { return { title: record.title, rating: record.rating, criteria: record.criteria, condition: record.condition, risk_effect: record.risk_effect, recommendation: record.recommendation }; }
function findingReady(form: FindingPayload) { return Boolean(form.title.trim() && form.criteria.trim() && form.condition.trim() && form.risk_effect.trim() && form.recommendation.trim()); }
function Fact({ label: fieldLabel, value, detail }: { label: string; value: string; detail?: string }) { return <div><dt className="text-xs font-semibold uppercase tracking-[0.12em] text-[var(--text-muted)]">{fieldLabel}</dt><dd className="mt-1 font-medium text-[var(--text-strong)]">{value}</dd>{detail ? <dd className="mt-1 text-xs text-[var(--text-muted)]">{detail}</dd> : null}</div>; }
function TextBlock({ label: fieldLabel, value }: { label: string; value: string }) { return <div><h3 className="text-xs font-semibold uppercase tracking-[0.12em] text-[var(--text-muted)]">{fieldLabel}</h3><p className="mt-2 whitespace-pre-wrap text-sm leading-6 text-[var(--text-body)]">{value}</p></div>; }
function Field({ label: fieldLabel, children }: { label: string; children: React.ReactNode }) { return <div className="space-y-2"><Label>{fieldLabel}</Label>{children}</div>; }

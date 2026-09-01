// Assigned and campus-wide Internal Audit engagement worklist.

import { useCallback, useEffect, useState } from "react";
import { Link } from "@tanstack/react-router";
import { ClipboardCheck, Plus, Search } from "lucide-react";
import toast from "react-hot-toast";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Table, TableControlsBar, TableControlsPagination, TableControlsSearch, TableEmpty,
  TableError, TableLoading, TableScroll, TableWrap, TBody, TD, TH, THead, TR,
} from "@/components/ui/data-table";
import { DialogBody, DialogFooter, DialogHeader, DialogShell } from "@/components/ui/dialog";
import { Input, Label, Select, Textarea } from "@/components/ui/input";
import { usePageChrome } from "@/modules/admin/layouts/page-chrome";
import { useAuthStore } from "@/stores/auth-store";

import { internalAuditService, responseMessage } from "./service";
import type { AuditEngagement, AuditPlan, AuditorCandidate, EngagementPayload } from "./types";
import { allowed, dateValue, label, tone } from "./ui";

export function InternalAuditEngagementsWorkspace() {
  const permissions = useAuthStore((state) => state.user?.permissions ?? []);
  const canCreate = allowed(permissions, "internal_audit:manage");
  const [records, setRecords] = useState<AuditEngagement[]>([]);
  const [search, setSearch] = useState("");
  const [submittedSearch, setSubmittedSearch] = useState("");
  const [status, setStatus] = useState("all");
  const [page, setPage] = useState(1);
  const [totalPages, setTotalPages] = useState(1);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [drawerOpen, setDrawerOpen] = useState(false);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const response = await internalAuditService.engagements({
        page,
        per_page: 25,
        search: submittedSearch || undefined,
        status: status === "all" ? undefined : status,
      });
      if (!response.success || !response.data) throw new Error(responseMessage(response, "Engagements could not be loaded"));
      setRecords(response.data.engagements);
      setTotalPages(response.pagination?.total_pages ?? 1);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : "Engagements could not be loaded");
    } finally {
      setLoading(false);
    }
  }, [page, status, submittedSearch]);

  useEffect(() => { void load(); }, [load]);
  usePageChrome("Engagements", canCreate ? <Button onClick={() => setDrawerOpen(true)}><Plus className="size-4" />New engagement</Button> : null);

  const filtered = Boolean(submittedSearch || status !== "all");
  return <div className="space-y-6">
    <TableControlsBar>
      <TableControlsSearch onSubmit={(event) => { event.preventDefault(); setPage(1); setSubmittedSearch(search.trim()); }}>
        <Input aria-label="Search audit engagements" leadingIcon={<Search />} onChange={(event) => setSearch(event.target.value)} placeholder="Search reference, plan, title, or auditor" value={search} />
        <Button type="submit" variant="secondary">Search</Button>
      </TableControlsSearch>
      <Select aria-label="Engagement status" className="sm:w-44" onChange={(event) => { setPage(1); setStatus(event.target.value); }} value={status}>
        <option value="all">All statuses</option><option value="planned">Planned</option><option value="fieldwork">Fieldwork</option><option value="reporting">Reporting</option><option value="closed">Closed</option>
      </Select>
      {!loading && records.length > 0 ? <TableControlsPagination onNext={() => setPage((value) => Math.min(totalPages, value + 1))} onPrevious={() => setPage((value) => Math.max(1, value - 1))} page={page} totalPages={totalPages} /> : null}
    </TableControlsBar>
    <TableWrap>{loading ? <TableLoading columns={7} label="Loading audit engagements…" /> : error ? <TableError description={error} onRetry={() => void load()} /> : records.length === 0 ? <TableEmpty description={filtered ? "Change the current filters." : "Create an engagement from an approved audit plan."} icon={<ClipboardCheck />} title={filtered ? "No engagements match" : "No audit engagements yet"} /> : <TableScroll><Table className="min-w-[1040px]"><THead><tr><TH>Engagement</TH><TH>Plan</TH><TH>Lead auditor</TH><TH>Dates</TH><TH>Evidence</TH><TH>Findings</TH><TH>Status</TH></tr></THead><TBody>{records.map((record) => <TR key={record.id}>
      <TD><Link className="font-semibold text-[var(--brand-strong)] hover:underline" params={{ engagementId: record.id }} to="/modules/internal-audit/engagements/$engagementId">{record.reference}</Link><p className="mt-1 max-w-72 truncate text-sm text-[var(--text-strong)]">{record.title}</p></TD>
      <TD><p className="font-medium text-[var(--text-strong)]">{record.plan_reference}</p><p className="mt-1 max-w-56 truncate text-xs text-[var(--text-muted)]">{record.plan_title}</p></TD>
      <TD><p className="text-[var(--text-strong)]">{record.lead_auditor_name}</p><p className="mt-1 text-xs text-[var(--text-muted)]">{record.lead_auditor_email}</p></TD>
      <TD><p>{dateValue(record.starts_on)}</p><p className="mt-1 text-xs text-[var(--text-muted)]">Due {dateValue(record.due_on)}</p></TD>
      <TD className="font-tabular">{record.evidence_count}</TD><TD className="font-tabular">{record.finding_count}</TD><TD><Badge tone={tone(record.status)}>{label(record.status)}</Badge></TD>
    </TR>)}</TBody></Table></TableScroll>}</TableWrap>
    <EngagementDrawer onClose={() => setDrawerOpen(false)} onSaved={() => { setDrawerOpen(false); void load(); }} open={drawerOpen} />
  </div>;
}

function EngagementDrawer({ open, onClose, onSaved }: { open: boolean; onClose: () => void; onSaved: () => void }) {
  const user = useAuthStore((state) => state.user);
  const permissions = user?.permissions ?? [];
  const canManage = allowed(permissions, "internal_audit:manage");
  const [plans, setPlans] = useState<AuditPlan[]>([]);
  const [auditors, setAuditors] = useState<AuditorCandidate[]>([]);
  const [form, setForm] = useState<EngagementPayload>(() => blankEngagement());
  const [loadingOptions, setLoadingOptions] = useState(false);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    if (!open) return;
    setForm(blankEngagement());
    setLoadingOptions(true);
    void Promise.all([
      internalAuditService.plans({ status: "approved", per_page: 100 }),
      internalAuditService.auditors(),
    ]).then(([planResponse, auditorResponse]) => {
      if (planResponse.success && planResponse.data) setPlans(planResponse.data.plans);
      if (auditorResponse.success && auditorResponse.data) {
        const eligible = canManage ? auditorResponse.data : auditorResponse.data.filter((candidate) => candidate.user_id === user?.id);
        setAuditors(eligible);
        if (eligible.length === 1) setForm((current) => ({ ...current, lead_auditor_user_id: eligible[0].user_id }));
      }
    }).finally(() => setLoadingOptions(false));
  }, [canManage, open, user?.id]);

  const submit = async (event: React.FormEvent) => {
    event.preventDefault();
    setSaving(true);
    try {
      const response = await internalAuditService.createEngagement(form);
      if (!response.success) throw new Error(responseMessage(response, "Engagement could not be created"));
      toast.success(`Created ${response.data?.reference ?? "engagement"}`);
      onSaved();
    } catch (cause) {
      toast.error(cause instanceof Error ? cause.message : "Engagement could not be created");
    } finally {
      setSaving(false);
    }
  };

  const ready = form.plan_id && form.title.trim() && form.objective.trim() && form.scope_text.trim() && form.lead_auditor_user_id && form.starts_on && form.due_on;
  return <DialogShell onClose={onClose} open={open}><form onSubmit={(event) => void submit(event)}><DialogHeader onClose={onClose} title="New audit engagement" /><DialogBody><div className="space-y-5">
    <Field label="Approved plan"><Select data-autofocus="true" disabled={loadingOptions} onChange={(event) => setForm({ ...form, plan_id: event.target.value })} required value={form.plan_id}><option value="">Choose plan</option>{plans.map((plan) => <option key={plan.id} value={plan.id}>{plan.reference} · {plan.title}</option>)}</Select>{!loadingOptions && plans.length === 0 ? <p className="text-xs text-[var(--text-muted)]">Approve an audit plan before creating an engagement.</p> : null}</Field>
    <Field label="Title"><Input maxLength={200} onChange={(event) => setForm({ ...form, title: event.target.value })} required value={form.title} /></Field>
    <Field label="Objective"><Textarea maxLength={4000} onChange={(event) => setForm({ ...form, objective: event.target.value })} required rows={4} value={form.objective} /></Field>
    <Field label="Scope"><Textarea maxLength={6000} onChange={(event) => setForm({ ...form, scope_text: event.target.value })} required rows={5} value={form.scope_text} /></Field>
    <Field label="Lead auditor"><Select disabled={loadingOptions} onChange={(event) => setForm({ ...form, lead_auditor_user_id: event.target.value })} required value={form.lead_auditor_user_id}><option value="">Choose auditor</option>{auditors.map((auditor) => <option key={auditor.user_id} value={auditor.user_id}>{auditor.full_name} · {auditor.email}</option>)}</Select></Field>
    <div className="grid gap-5 sm:grid-cols-2"><Field label="Starts"><Input onChange={(event) => setForm({ ...form, starts_on: event.target.value })} required type="date" value={form.starts_on} /></Field><Field label="Due"><Input min={form.starts_on} onChange={(event) => setForm({ ...form, due_on: event.target.value })} required type="date" value={form.due_on} /></Field></div>
  </div></DialogBody><DialogFooter><Button onClick={onClose} type="button" variant="secondary">Cancel</Button><Button disabled={saving || !ready} type="submit">{saving ? "Creating…" : "Create engagement"}</Button></DialogFooter></form></DialogShell>;
}

function blankEngagement(): EngagementPayload {
  const today = new Date().toISOString().slice(0, 10);
  return { plan_id: "", title: "", objective: "", scope_text: "", lead_auditor_user_id: "", starts_on: today, due_on: today };
}

function Field({ label: fieldLabel, children }: { label: string; children: React.ReactNode }) {
  return <div className="space-y-2"><Label>{fieldLabel}</Label>{children}</div>;
}

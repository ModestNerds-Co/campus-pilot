// Internal Audit plan register and approval lifecycle.

import { useCallback, useEffect, useState } from "react";
import { ClipboardList, Edit3, Plus, Search, ShieldCheck, Trash2 } from "lucide-react";
import toast from "react-hot-toast";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { ConfirmDrawer } from "@/components/ui/confirm-drawer";
import {
  Table, TableControlsBar, TableControlsPagination, TableControlsSearch, TableEmpty,
  TableError, TableLoading, TableScroll, TableWrap, TBody, TD, TH, THead, TR,
} from "@/components/ui/data-table";
import { DialogBody, DialogFooter, DialogHeader, DialogShell } from "@/components/ui/dialog";
import { Input, Label, Select, Textarea } from "@/components/ui/input";
import { usePageChrome } from "@/modules/admin/layouts/page-chrome";
import { useAuthStore } from "@/stores/auth-store";

import { internalAuditService, responseMessage } from "./service";
import type { AuditPlan, PlanPayload } from "./types";
import { allowed, dateValue, label, tone } from "./ui";

type PlanDrawerState = { kind: "create" | "edit" | "close"; record: AuditPlan | null } | null;

export function InternalAuditPlansWorkspace() {
  const permissions = useAuthStore((state) => state.user?.permissions ?? []);
  const canCreate = allowed(permissions, "internal_audit:manage");
  const canEdit = allowed(permissions, "internal_audit:manage");
  const canDelete = allowed(permissions, "internal_audit:delete");
  const canManage = allowed(permissions, "internal_audit:manage");
  const [records, setRecords] = useState<AuditPlan[]>([]);
  const [search, setSearch] = useState("");
  const [submittedSearch, setSubmittedSearch] = useState("");
  const [status, setStatus] = useState("all");
  const [page, setPage] = useState(1);
  const [totalPages, setTotalPages] = useState(1);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [drawer, setDrawer] = useState<PlanDrawerState>(null);
  const [approveRecord, setApproveRecord] = useState<AuditPlan | null>(null);
  const [deleteRecord, setDeleteRecord] = useState<AuditPlan | null>(null);
  const [pending, setPending] = useState(false);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const response = await internalAuditService.plans({ page, per_page: 25, search: submittedSearch || undefined, status: status === "all" ? undefined : status });
      if (!response.success || !response.data) throw new Error(responseMessage(response, "Audit plans could not be loaded"));
      setRecords(response.data.plans);
      setTotalPages(response.pagination?.total_pages ?? 1);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : "Audit plans could not be loaded");
    } finally {
      setLoading(false);
    }
  }, [page, status, submittedSearch]);

  useEffect(() => { void load(); }, [load]);
  usePageChrome("Audit plans", canCreate ? <Button onClick={() => setDrawer({ kind: "create", record: null })}><Plus className="size-4" />New plan</Button> : null);

  const approve = async () => {
    if (!approveRecord || pending) return;
    setPending(true);
    const response = await internalAuditService.approvePlan(approveRecord);
    setPending(false);
    if (response.success) { toast.success("Audit plan approved"); setApproveRecord(null); void load(); }
    else toast.error(responseMessage(response, "Audit plan could not be approved"));
  };

  const remove = async () => {
    if (!deleteRecord || pending) return;
    setPending(true);
    const response = await internalAuditService.deletePlan(deleteRecord);
    setPending(false);
    if (response.success) { toast.success("Audit plan deleted"); setDeleteRecord(null); void load(); }
    else toast.error(responseMessage(response, "Audit plan could not be deleted"));
  };

  const filtered = Boolean(submittedSearch || status !== "all");
  return <div className="space-y-6">
    <TableControlsBar><TableControlsSearch onSubmit={(event) => { event.preventDefault(); setPage(1); setSubmittedSearch(search.trim()); }}><Input aria-label="Search audit plans" leadingIcon={<Search />} onChange={(event) => setSearch(event.target.value)} placeholder="Search reference, title, or objective" value={search} /><Button type="submit" variant="secondary">Search</Button></TableControlsSearch><Select aria-label="Plan status" className="sm:w-44" onChange={(event) => { setPage(1); setStatus(event.target.value); }} value={status}><option value="all">All statuses</option><option value="draft">Draft</option><option value="approved">Approved</option><option value="closed">Closed</option></Select>{!loading && records.length > 0 ? <TableControlsPagination onNext={() => setPage((value) => Math.min(totalPages, value + 1))} onPrevious={() => setPage((value) => Math.max(1, value - 1))} page={page} totalPages={totalPages} /> : null}</TableControlsBar>
    <TableWrap>{loading ? <TableLoading columns={6} label="Loading audit plans…" /> : error ? <TableError description={error} onRetry={() => void load()} /> : records.length === 0 ? <TableEmpty description={filtered ? "Change the current filters." : "Create the first audit plan."} icon={<ClipboardList />} title={filtered ? "No plans match" : "No audit plans yet"} /> : <TableScroll><Table className="min-w-[980px]"><THead><tr><TH>Plan</TH><TH>Period</TH><TH>Risk focus</TH><TH>Engagements</TH><TH>Status</TH><TH className="text-right">Actions</TH></tr></THead><TBody>{records.map((record) => <TR key={record.id}><TD><p className="font-semibold text-[var(--text-strong)]">{record.reference}</p><p className="mt-1 max-w-72 truncate text-sm">{record.title}</p><p className="mt-1 max-w-72 truncate text-xs text-[var(--text-muted)]">{record.objective}</p></TD><TD>{dateValue(record.period_start)}<p className="mt-1 text-xs text-[var(--text-muted)]">to {dateValue(record.period_end)}</p></TD><TD className="max-w-64 text-[var(--text-muted)]">{record.risk_summary || "—"}</TD><TD className="font-tabular">{record.engagement_count}</TD><TD><Badge tone={tone(record.status)}>{label(record.status)}</Badge></TD><TD><div className="flex justify-end gap-2">{record.status === "draft" && canEdit ? <Button aria-label={`Edit ${record.reference}`} onClick={() => setDrawer({ kind: "edit", record })} size="sm" variant="ghost"><Edit3 className="size-4" /></Button> : null}{record.status === "draft" && canManage ? <Button onClick={() => setApproveRecord(record)} size="sm" variant="secondary"><ShieldCheck className="size-4" />Approve</Button> : null}{record.status === "approved" && canManage ? <Button onClick={() => setDrawer({ kind: "close", record })} size="sm" variant="secondary">Close</Button> : null}{record.status === "draft" && canDelete ? <Button aria-label={`Delete ${record.reference}`} onClick={() => setDeleteRecord(record)} size="sm" variant="ghost"><Trash2 className="size-4" /></Button> : null}</div></TD></TR>)}</TBody></Table></TableScroll>}</TableWrap>
    <PlanDrawer drawer={drawer} onClose={() => setDrawer(null)} onSaved={() => { setDrawer(null); void load(); }} />
    <ConfirmDrawer confirmLabel="Approve plan" description={`Approve ${approveRecord?.reference ?? "this audit plan"}? Its dates and scope will become the basis for audit engagements.`} isPending={pending} onClose={() => setApproveRecord(null)} onConfirm={() => void approve()} open={approveRecord !== null} title="Approve audit plan?" />
    <ConfirmDrawer confirmLabel="Delete plan" description={`Delete ${deleteRecord?.reference ?? "this draft plan"}?`} isPending={pending} onClose={() => setDeleteRecord(null)} onConfirm={() => void remove()} open={deleteRecord !== null} title="Delete draft audit plan?" />
  </div>;
}

function PlanDrawer({ drawer, onClose, onSaved }: { drawer: PlanDrawerState; onClose: () => void; onSaved: () => void }) {
  const [form, setForm] = useState<PlanPayload>(() => blankPlan());
  const [summary, setSummary] = useState("");
  const [saving, setSaving] = useState(false);
  useEffect(() => {
    if (!drawer) return;
    setSummary("");
    setForm(drawer.record ? { title: drawer.record.title, objective: drawer.record.objective, risk_summary: drawer.record.risk_summary, period_start: drawer.record.period_start, period_end: drawer.record.period_end } : blankPlan());
  }, [drawer]);
  const submit = async (event: React.FormEvent) => {
    event.preventDefault();
    if (!drawer) return;
    setSaving(true);
    try {
      const response = drawer.kind === "close" && drawer.record
        ? await internalAuditService.closePlan(drawer.record, summary.trim())
        : drawer.kind === "edit" && drawer.record
          ? await internalAuditService.updatePlan(drawer.record, form)
          : await internalAuditService.createPlan(form);
      if (!response.success) throw new Error(responseMessage(response, "Audit plan could not be saved"));
      toast.success(drawer.kind === "close" ? "Audit plan closed" : drawer.kind === "edit" ? "Audit plan updated" : `Created ${response.data?.reference ?? "audit plan"}`);
      onSaved();
    } catch (cause) {
      toast.error(cause instanceof Error ? cause.message : "Audit plan could not be saved");
    } finally {
      setSaving(false);
    }
  };
  if (!drawer) return null;
  return <DialogShell onClose={onClose} open><form onSubmit={(event) => void submit(event)}><DialogHeader onClose={onClose} title={drawer.kind === "create" ? "New audit plan" : drawer.kind === "edit" ? `Edit ${drawer.record?.reference}` : `Close ${drawer.record?.reference}`} /><DialogBody><div className="space-y-5">{drawer.kind === "close" ? <><div className="rounded-[var(--radius-lg)] bg-[var(--surface-muted)] p-4"><p className="font-semibold text-[var(--text-strong)]">{drawer.record?.title}</p><p className="mt-1 text-xs text-[var(--text-muted)]">Every engagement must already be closed.</p></div><Field label="Closure summary"><Textarea data-autofocus="true" maxLength={4000} onChange={(event) => setSummary(event.target.value)} required rows={7} value={summary} /></Field></> : <><Field label="Title"><Input data-autofocus="true" maxLength={200} onChange={(event) => setForm({ ...form, title: event.target.value })} required value={form.title} /></Field><Field label="Objective"><Textarea maxLength={4000} onChange={(event) => setForm({ ...form, objective: event.target.value })} required rows={5} value={form.objective} /></Field><Field label="Risk focus"><Textarea maxLength={4000} onChange={(event) => setForm({ ...form, risk_summary: event.target.value || null })} rows={4} value={form.risk_summary ?? ""} /></Field><div className="grid gap-5 sm:grid-cols-2"><Field label="Period starts"><Input onChange={(event) => setForm({ ...form, period_start: event.target.value })} required type="date" value={form.period_start} /></Field><Field label="Period ends"><Input min={form.period_start} onChange={(event) => setForm({ ...form, period_end: event.target.value })} required type="date" value={form.period_end} /></Field></div></>}</div></DialogBody><DialogFooter><Button onClick={onClose} type="button" variant="secondary">Cancel</Button><Button disabled={saving || (drawer.kind === "close" ? !summary.trim() : !form.title.trim() || !form.objective.trim())} type="submit">{saving ? "Saving…" : drawer.kind === "close" ? "Close plan" : "Save plan"}</Button></DialogFooter></form></DialogShell>;
}

function blankPlan(): PlanPayload {
  const year = new Date().getFullYear();
  return { title: "", objective: "", risk_summary: null, period_start: `${year}-01-01`, period_end: `${year}-12-31` };
}

function Field({ label: fieldLabel, children }: { label: string; children: React.ReactNode }) {
  return <div className="space-y-2"><Label>{fieldLabel}</Label>{children}</div>;
}

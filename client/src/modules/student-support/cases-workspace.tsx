/** Restricted Student Support worklist and case-creation drawer. */

import { useCallback, useEffect, useMemo, useState } from "react";
import { useNavigate } from "@tanstack/react-router";
import { HeartHandshake, Plus, Search } from "lucide-react";
import toast from "react-hot-toast";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Table,
  TableControlsBar,
  TableControlsPagination,
  TableEmpty,
  TableError,
  TableLoading,
  TableScroll,
  TableWrap,
  TBody,
  TD,
  TH,
  THead,
  TR,
} from "@/components/ui/data-table";
import { DialogBody, DialogFooter, DialogHeader, DialogShell } from "@/components/ui/dialog";
import { Input, Label, Select, Textarea } from "@/components/ui/input";
import { usePageChrome } from "@/modules/admin/layouts/page-chrome";
import { useAuthStore } from "@/stores/auth-store";

import { responseMessage, studentSupportService } from "./service";
import type {
  CasePayload,
  CaseSeverity,
  CaseStatus,
  CaseSummary,
  ConcernCategory,
  StudentSupportReferences,
} from "./types";
import { displayValue, formatDateTime, severityTone, statusTone } from "./ui";

const categoryValues: ConcernCategory[] = ["wellbeing", "behaviour", "conduct", "safeguarding", "family", "learning_support", "other"];
const severityValues: CaseSeverity[] = ["low", "moderate", "high", "critical"];
const statusValues: CaseStatus[] = ["open", "active", "escalated", "resolved", "closed"];

export function StudentSupportCasesWorkspace() {
  const navigate = useNavigate();
  const permissions = useAuthStore((state) => state.user?.permissions ?? []);
  const canCreate = allowed(permissions, "student_support:create");
  const canManage = allowed(permissions, "student_support:manage");
  const [records, setRecords] = useState<CaseSummary[]>([]);
  const [search, setSearch] = useState("");
  const [status, setStatus] = useState<CaseStatus | "all">("all");
  const [severity, setSeverity] = useState<CaseSeverity | "all">("all");
  const [category, setCategory] = useState<ConcernCategory | "all">("all");
  const [page, setPage] = useState(1);
  const [totalPages, setTotalPages] = useState(1);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [createOpen, setCreateOpen] = useState(false);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const response = await studentSupportService.cases({
        page,
        per_page: 25,
        search: search.trim() || undefined,
        status: status === "all" ? undefined : status,
        severity: severity === "all" ? undefined : severity,
        category: category === "all" ? undefined : category,
      });
      if (!response.success || !response.data) throw new Error(responseMessage(response, "Cases could not be loaded"));
      setRecords(response.data.cases);
      setTotalPages(response.pagination?.total_pages ?? 1);
    } catch (loadError) {
      setError(loadError instanceof Error ? loadError.message : "Cases could not be loaded");
    } finally {
      setLoading(false);
    }
  }, [category, page, search, severity, status]);

  useEffect(() => { void load(); }, [load]);
  usePageChrome("Cases", canCreate ? <Button onClick={() => setCreateOpen(true)}><Plus className="size-4" />Open case</Button> : null);

  const resetPage = () => setPage(1);
  const filtered = search || status !== "all" || severity !== "all" || category !== "all";

  return <div className="space-y-6">
    <TableControlsBar>
      <Input aria-label="Search cases" className="sm:w-72" leadingIcon={<Search />} onChange={(event) => { resetPage(); setSearch(event.target.value); }} placeholder="Search case or learner" value={search} />
      <Select aria-label="Case status" className="sm:w-40" onChange={(event) => { resetPage(); setStatus(event.target.value as CaseStatus | "all"); }} value={status}><option value="all">All statuses</option>{statusValues.map((value) => <option key={value} value={value}>{displayValue(value)}</option>)}</Select>
      <Select aria-label="Case severity" className="sm:w-40" onChange={(event) => { resetPage(); setSeverity(event.target.value as CaseSeverity | "all"); }} value={severity}><option value="all">All severities</option>{severityValues.map((value) => <option key={value} value={value}>{displayValue(value)}</option>)}</Select>
      <Select aria-label="Concern category" className="sm:w-44" onChange={(event) => { resetPage(); setCategory(event.target.value as ConcernCategory | "all"); }} value={category}><option value="all">All categories</option>{categoryValues.map((value) => <option key={value} value={value}>{displayValue(value)}</option>)}</Select>
      {!loading && records.length ? <TableControlsPagination onNext={() => setPage((value) => Math.min(totalPages, value + 1))} onPrevious={() => setPage((value) => Math.max(1, value - 1))} page={page} totalPages={totalPages} /> : null}
    </TableControlsBar>
    <TableWrap>
      {loading ? <TableLoading columns={7} label="Loading Student Support cases…" /> : error ? <TableError description={error} onRetry={() => void load()} /> : records.length === 0 ? <TableEmpty description={filtered ? "Change the current filters." : canCreate ? "Open the first learner-support case." : "No assigned cases are available."} icon={<HeartHandshake />} title={filtered ? "No cases match" : "No cases yet"} /> : <TableScroll><Table className="min-w-[1120px]"><THead><tr><TH>Case</TH><TH>Learner</TH><TH>Concern</TH><TH>Severity</TH><TH>Lead</TH><TH>Status</TH><TH>Updated</TH></tr></THead><TBody>
        {records.map((record) => <TR className="cursor-pointer" key={record.id} onClick={() => void navigate({ to: "/modules/student-support/cases/$caseId", params: { caseId: record.id } })}>
          <TD><p className="font-medium text-[var(--text-strong)]">{record.title}</p><p className="mt-1 font-tabular text-xs text-[var(--text-muted)]">{record.reference} · {record.action_count} actions · {record.team_member_count} people</p></TD>
          <TD><p className="text-[var(--text-strong)]">{record.learner_name}</p><p className="mt-1 font-tabular text-xs text-[var(--text-muted)]">{record.learner_number}</p></TD>
          <TD className="text-[var(--text-muted)]">{displayValue(record.category)}</TD>
          <TD><Badge tone={severityTone(record.severity)}>{displayValue(record.severity)}</Badge></TD>
          <TD className="text-[var(--text-muted)]">{record.lead_case_worker_name}</TD>
          <TD><Badge tone={statusTone(record.status)}>{displayValue(record.status)}</Badge></TD>
          <TD className="whitespace-nowrap text-[var(--text-muted)]">{formatDateTime(record.updated_at)}</TD>
        </TR>)}
      </TBody></Table></TableScroll>}
    </TableWrap>
    <CreateCaseDrawer canManage={canManage} onClose={() => setCreateOpen(false)} onSaved={(record) => { setCreateOpen(false); void navigate({ to: "/modules/student-support/cases/$caseId", params: { caseId: record.id } }); }} open={createOpen} />
  </div>;
}

function CreateCaseDrawer({ canManage, open, onClose, onSaved }: { canManage: boolean; open: boolean; onClose: () => void; onSaved: (record: CaseSummary) => void }) {
  const [references, setReferences] = useState<StudentSupportReferences | null>(null);
  const [referenceError, setReferenceError] = useState<string | null>(null);
  const [learnerSearch, setLearnerSearch] = useState("");
  const [form, setForm] = useState<CasePayload>({ learner_id: "", lead_case_worker_user_id: null, category: "wellbeing", severity: "moderate", title: "", summary: "", occurred_on: null });
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    if (!open) return;
    setForm({ learner_id: "", lead_case_worker_user_id: null, category: "wellbeing", severity: "moderate", title: "", summary: "", occurred_on: null });
    setLearnerSearch("");
    setReferenceError(null);
    void studentSupportService.references().then((response) => {
      if (response.success && response.data) setReferences(response.data);
      else setReferenceError(responseMessage(response, "Learner references could not be loaded"));
    }).catch(() => setReferenceError("Learner references could not be loaded"));
  }, [open]);

  const learners = useMemo(() => {
    const query = learnerSearch.trim().toLowerCase();
    if (!query) return references?.learners ?? [];
    return (references?.learners ?? []).filter((learner) => `${learner.display_name} ${learner.learner_number}`.toLowerCase().includes(query));
  }, [learnerSearch, references]);

  const submit = async (event: React.FormEvent) => {
    event.preventDefault();
    setSaving(true);
    try {
      const response = await studentSupportService.createCase({ ...form, title: form.title.trim(), summary: form.summary.trim() });
      if (!response.success || !response.data) throw new Error(responseMessage(response, "Case could not be opened"));
      toast.success(`Case ${response.data.reference} opened`);
      onSaved(response.data);
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "Case could not be opened");
    } finally {
      setSaving(false);
    }
  };

  return <DialogShell onClose={onClose} open={open} panelClassName="max-w-[680px]"><DialogHeader onClose={onClose} title="Open Student Support case" /><form className="flex min-h-0 flex-1 flex-col overflow-hidden" onSubmit={(event) => void submit(event)}><DialogBody><div className="space-y-5">
    {referenceError ? <div className="border border-[var(--tone-danger-bd)] bg-[var(--tone-danger-bg)] p-4 text-sm text-[var(--tone-danger)]">{referenceError}</div> : null}
    <Field label="Find learner"><Input data-autofocus="true" onChange={(event) => setLearnerSearch(event.target.value)} placeholder="Name or learner number" value={learnerSearch} /></Field>
    <Field label="Learner"><Select disabled={!references} onChange={(event) => setForm({ ...form, learner_id: event.target.value })} required value={form.learner_id}><option value="">Select a learner</option>{learners.map((learner) => <option key={learner.learner_id} value={learner.learner_id}>{learner.display_name} · {learner.learner_number}</option>)}</Select></Field>
    {canManage ? <Field label="Lead Case Worker"><Select onChange={(event) => setForm({ ...form, lead_case_worker_user_id: event.target.value || null })} required value={form.lead_case_worker_user_id ?? ""}><option value="">Select a Case Worker</option>{references?.case_workers.map((worker) => <option key={worker.user_id} value={worker.user_id}>{worker.full_name} · {worker.email}</option>)}</Select></Field> : null}
    <div className="grid gap-4 sm:grid-cols-2"><Field label="Concern category"><Select onChange={(event) => setForm({ ...form, category: event.target.value as ConcernCategory })} value={form.category}>{categoryValues.map((value) => <option key={value} value={value}>{displayValue(value)}</option>)}</Select></Field><Field label="Severity"><Select onChange={(event) => setForm({ ...form, severity: event.target.value as CaseSeverity })} value={form.severity}>{severityValues.map((value) => <option key={value} value={value}>{displayValue(value)}</option>)}</Select></Field></div>
    <Field label="Title"><Input maxLength={200} onChange={(event) => setForm({ ...form, title: event.target.value })} required value={form.title} /></Field>
    <Field label="Summary"><Textarea maxLength={6000} onChange={(event) => setForm({ ...form, summary: event.target.value })} required rows={7} value={form.summary} /></Field>
    <Field label="Occurred on"><Input max={new Date().toISOString().slice(0, 10)} onChange={(event) => setForm({ ...form, occurred_on: event.target.value || null })} type="date" value={form.occurred_on ?? ""} /></Field>
  </div></DialogBody><DialogFooter><Button onClick={onClose} type="button" variant="secondary">Cancel</Button><Button disabled={saving || !form.learner_id || !form.title.trim() || !form.summary.trim() || (canManage && !form.lead_case_worker_user_id)} type="submit">{saving ? "Opening…" : "Open case"}</Button></DialogFooter></form></DialogShell>;
}

function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return <div className="space-y-2"><Label>{label}</Label>{children}</div>;
}

function allowed(permissions: string[], permission: string) {
  return permissions.includes("*") || permissions.includes(permission);
}

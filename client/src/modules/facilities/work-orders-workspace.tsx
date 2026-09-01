/** Facilities assigned-work queue, completion, and inspection drawers. */

import { useCallback, useEffect, useState } from "react";
import { ClipboardCheck, Plus, Search, Wrench } from "lucide-react";
import toast from "react-hot-toast";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Table, TableControlsBar, TableControlsPagination, TableEmpty, TableError, TableLoading, TableScroll, TableWrap, TBody, TD, TH, THead, TR } from "@/components/ui/data-table";
import { DialogBody, DialogFooter, DialogHeader, DialogShell } from "@/components/ui/dialog";
import { Input, Label, Select, Textarea } from "@/components/ui/input";
import { usePageChrome } from "@/modules/admin/layouts/page-chrome";
import { useAuthStore } from "@/stores/auth-store";

import { facilitiesService, responseMessage } from "./service";
import type { FacilityInspectionOutcome, FacilityReferences, FacilityServiceRequestSummary, FacilityWorkOrderRecord, FacilityWorkOrderStatus, FacilityWorkOrderSummary, WorkOrderPayload } from "./types";
import { allowed, displayValue, formatDate, formatDateTime, workOrderTone } from "./ui";

const statuses: FacilityWorkOrderStatus[] = ["assigned", "in_progress", "ready_for_inspection", "completed", "cancelled"];
const emptyPayload: WorkOrderPayload = { service_request_id: "", assigned_employee_id: "", title: "", instructions: null, target_date: null };

export function FacilitiesWorkOrdersWorkspace() {
  const permissions = useAuthStore((state) => state.user?.permissions ?? []);
  const canOperate = allowed(permissions, "facilities:operate") || allowed(permissions, "facilities:manage");
  const canManage = allowed(permissions, "facilities:manage");
  const [records, setRecords] = useState<FacilityWorkOrderSummary[]>([]);
  const [search, setSearch] = useState("");
  const [status, setStatus] = useState<FacilityWorkOrderStatus | "all">("all");
  const [page, setPage] = useState(1);
  const [totalPages, setTotalPages] = useState(1);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [createOpen, setCreateOpen] = useState(false);
  const [selected, setSelected] = useState<FacilityWorkOrderSummary | null>(null);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const response = await facilitiesService.workOrders({ page, per_page: 25, search: search.trim() || undefined, status: status === "all" ? undefined : status });
      if (!response.success || !response.data) throw new Error(responseMessage(response, "Work orders could not be loaded"));
      setRecords(response.data);
      setTotalPages(response.pagination?.total_pages ?? 1);
    } catch (loadError) {
      setError(loadError instanceof Error ? loadError.message : "Work orders could not be loaded");
    } finally {
      setLoading(false);
    }
  }, [page, search, status]);

  useEffect(() => { void load(); }, [load]);
  usePageChrome("Work orders", canManage ? <Button onClick={() => setCreateOpen(true)}><Plus className="size-4" />Create work order</Button> : null);
  const filtered = search || status !== "all";

  return <div className="space-y-6">
    <TableControlsBar><Input aria-label="Search work orders" className="sm:w-72" leadingIcon={<Search />} onChange={(event) => { setPage(1); setSearch(event.target.value); }} placeholder="Search reference or assignee" value={search} /><Select aria-label="Work-order status" className="sm:w-52" onChange={(event) => { setPage(1); setStatus(event.target.value as FacilityWorkOrderStatus | "all"); }} value={status}><option value="all">All statuses</option>{statuses.map((value) => <option key={value} value={value}>{displayValue(value)}</option>)}</Select>{!loading && records.length ? <TableControlsPagination onNext={() => setPage((value) => Math.min(totalPages, value + 1))} onPrevious={() => setPage((value) => Math.max(1, value - 1))} page={page} totalPages={totalPages} /> : null}</TableControlsBar>
    <TableWrap>{loading ? <TableLoading columns={7} label="Loading Facilities work orders…" /> : error ? <TableError description={error} onRetry={() => void load()} /> : records.length === 0 ? <TableEmpty description={filtered ? "Change the current filters." : canManage ? "Create a work order from an open service request." : "No assigned work orders are available."} icon={<Wrench />} title={filtered ? "No work orders match" : "No work orders yet"} /> : <TableScroll><Table className="min-w-[1100px]"><THead><tr><TH>Work order</TH><TH>Request</TH><TH>Location</TH><TH>Assigned to</TH><TH>Target</TH><TH>Status</TH><TH>Updated</TH></tr></THead><TBody>{records.map((record) => <TR className="cursor-pointer" key={record.id} onClick={() => setSelected(record)}><TD><p className="font-medium text-[var(--text-strong)]">{record.title}</p><p className="mt-1 font-tabular text-xs text-[var(--text-muted)]">{record.reference}</p></TD><TD className="font-tabular text-[var(--text-muted)]">{record.service_request_reference}</TD><TD className="text-[var(--text-muted)]">{record.location_name}</TD><TD><p className="text-[var(--text-strong)]">{record.assigned_employee_name}</p><p className="mt-1 font-tabular text-xs text-[var(--text-muted)]">{record.assigned_employee_number}</p></TD><TD className="whitespace-nowrap text-[var(--text-muted)]">{formatDate(record.target_date)}</TD><TD><Badge tone={workOrderTone(record.status)}>{displayValue(record.status)}</Badge></TD><TD className="whitespace-nowrap text-[var(--text-muted)]">{formatDateTime(record.updated_at)}</TD></TR>)}</TBody></Table></TableScroll>}</TableWrap>
    <CreateWorkOrderDrawer onClose={() => setCreateOpen(false)} onSaved={() => { setCreateOpen(false); void load(); }} open={createOpen} />
    <WorkOrderDrawer canManage={canManage} canOperate={canOperate} onClose={() => setSelected(null)} onSaved={() => { setSelected(null); void load(); }} open={selected !== null} summary={selected} />
  </div>;
}

function CreateWorkOrderDrawer({ onClose, onSaved, open }: { onClose: () => void; onSaved: () => void; open: boolean }) {
  const [references, setReferences] = useState<FacilityReferences | null>(null);
  const [requests, setRequests] = useState<FacilityServiceRequestSummary[]>([]);
  const [form, setForm] = useState<WorkOrderPayload>(emptyPayload);
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    if (!open) return;
    setForm(emptyPayload); setLoading(true); setReferences(null); setRequests([]);
    void Promise.all([facilitiesService.references(), facilitiesService.requests({ status: "open", per_page: 100 })]).then(([referenceResponse, requestResponse]) => {
      if (referenceResponse.success && referenceResponse.data) setReferences(referenceResponse.data); else toast.error(responseMessage(referenceResponse, "Facilities references could not be loaded"));
      if (requestResponse.success && requestResponse.data) setRequests(requestResponse.data); else toast.error(responseMessage(requestResponse, "Open requests could not be loaded"));
    }).catch(() => toast.error("Work-order references could not be loaded")).finally(() => setLoading(false));
  }, [open]);

  const submit = async (event: React.FormEvent) => {
    event.preventDefault(); setSaving(true);
    try {
      const response = await facilitiesService.createWorkOrder({ ...form, title: form.title.trim(), instructions: form.instructions?.trim() || null });
      if (!response.success) throw new Error(responseMessage(response, "Work order could not be created"));
      toast.success("Work order created"); onSaved();
    } catch (saveError) { toast.error(saveError instanceof Error ? saveError.message : "Work order could not be created"); }
    finally { setSaving(false); }
  };

  const noEmployees = !loading && references?.employees.length === 0;
  const noRequests = !loading && requests.length === 0;
  return <DialogShell onClose={onClose} open={open}><DialogHeader onClose={saving ? undefined : onClose} title="Create work order" /><form className="flex min-h-0 flex-1 flex-col" onSubmit={(event) => void submit(event)}><DialogBody><div className="space-y-5">{noEmployees ? <Notice>There are no active employees to assign. Add the employee in HR and payroll first.</Notice> : null}{noRequests ? <Notice>There are no open Facilities requests.</Notice> : null}<Field label="Service request"><Select data-autofocus="true" disabled={loading || noRequests} onChange={(event) => setForm({ ...form, service_request_id: event.target.value })} required value={form.service_request_id}><option value="">{loading ? "Loading requests…" : "Select an open request"}</option>{requests.map((record) => <option key={record.id} value={record.id}>{record.reference} · {record.summary}</option>)}</Select></Field><Field label="Assign employee"><Select disabled={loading || noEmployees} onChange={(event) => setForm({ ...form, assigned_employee_id: event.target.value })} required value={form.assigned_employee_id}><option value="">{loading ? "Loading employees…" : "Select an employee"}</option>{references?.employees.map((employee) => <option key={employee.id} value={employee.id}>{employee.display_name} · {employee.employee_number}</option>)}</Select></Field><Field label="Title"><Input maxLength={200} onChange={(event) => setForm({ ...form, title: event.target.value })} required value={form.title} /></Field><Field label="Instructions"><Textarea maxLength={6000} onChange={(event) => setForm({ ...form, instructions: event.target.value || null })} rows={7} value={form.instructions ?? ""} /></Field><Field label="Target date"><Input min={new Date().toISOString().slice(0, 10)} onChange={(event) => setForm({ ...form, target_date: event.target.value || null })} type="date" value={form.target_date ?? ""} /></Field></div></DialogBody><DialogFooter><Button onClick={onClose} type="button" variant="secondary">Cancel</Button><Button disabled={saving || loading || !form.service_request_id || !form.assigned_employee_id || !form.title.trim()} type="submit">{saving ? "Creating…" : "Create work order"}</Button></DialogFooter></form></DialogShell>;
}

type WorkOrderAction = "completion" | "inspection" | "cancel";

function WorkOrderDrawer({ canManage, canOperate, onClose, onSaved, open, summary }: { canManage: boolean; canOperate: boolean; onClose: () => void; onSaved: () => void; open: boolean; summary: FacilityWorkOrderSummary | null }) {
  const [record, setRecord] = useState<FacilityWorkOrderRecord | null>(null);
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [action, setAction] = useState<WorkOrderAction | null>(null);
  const [notes, setNotes] = useState("");
  const [outcome, setOutcome] = useState<FacilityInspectionOutcome>("pass");

  const loadRecord = useCallback(async () => {
    if (!summary) return;
    setLoading(true);
    try { const response = await facilitiesService.workOrder(summary.id); if (response.success && response.data) setRecord(response.data); else toast.error(responseMessage(response, "Work order could not be loaded")); }
    catch { toast.error("Work order could not be loaded"); }
    finally { setLoading(false); }
  }, [summary]);

  useEffect(() => { if (open) { setRecord(null); setAction(null); setNotes(""); setOutcome("pass"); void loadRecord(); } }, [loadRecord, open]);

  const start = async () => {
    if (!summary) return; setSaving(true);
    try { const response = await facilitiesService.startWorkOrder(summary); if (!response.success) throw new Error(responseMessage(response, "Work order could not be started")); toast.success("Work order started"); onSaved(); }
    catch (startError) { toast.error(startError instanceof Error ? startError.message : "Work order could not be started"); }
    finally { setSaving(false); }
  };

  const transition = async () => {
    if (!summary || !action) return; setSaving(true);
    try {
      const response = action === "completion" ? await facilitiesService.submitCompletion(summary, notes.trim()) : action === "inspection" ? await facilitiesService.inspectWorkOrder(summary, outcome, notes.trim()) : await facilitiesService.cancelWorkOrder(summary, notes.trim());
      if (!response.success) throw new Error(responseMessage(response, "Work order could not be updated"));
      toast.success(action === "completion" ? "Completion submitted" : action === "inspection" ? (outcome === "pass" ? "Inspection passed" : "Work returned for correction") : "Work order cancelled");
      onSaved();
    } catch (transitionError) { toast.error(transitionError instanceof Error ? transitionError.message : "Work order could not be updated"); }
    finally { setSaving(false); }
  };

  return <DialogShell onClose={onClose} open={open} panelClassName="max-w-[760px]"><DialogHeader onClose={saving ? undefined : onClose} title={summary?.reference ?? "Work order"} />{action ? <div className="flex min-h-0 flex-1 flex-col"><DialogBody><div className="space-y-5">{action === "inspection" ? <Field label="Outcome"><Select data-autofocus="true" onChange={(event) => setOutcome(event.target.value as FacilityInspectionOutcome)} value={outcome}><option value="pass">Pass</option><option value="fail">Fail and return for correction</option></Select></Field> : null}<Field label={action === "completion" ? "Completion summary" : action === "inspection" ? "Inspection notes" : "Cancellation reason"}><Textarea data-autofocus={action !== "inspection"} maxLength={6000} onChange={(event) => setNotes(event.target.value)} required rows={8} value={notes} /></Field></div></DialogBody><DialogFooter><Button onClick={() => { setAction(null); setNotes(""); }} type="button" variant="secondary">Back</Button><Button disabled={saving || !notes.trim()} onClick={() => void transition()} type="button" variant={action === "cancel" ? "destructive" : "default"}>{saving ? "Saving…" : action === "completion" ? "Submit for inspection" : action === "inspection" ? "Record inspection" : "Cancel work order"}</Button></DialogFooter></div> : <div className="flex min-h-0 flex-1 flex-col"><DialogBody>{loading || !record ? <div className="flex min-h-48 items-center justify-center text-sm text-[var(--text-muted)]">{loading ? "Loading work order…" : "Work order unavailable"}</div> : <div className="space-y-6"><div><Badge tone={workOrderTone(record.work_order.status)}>{displayValue(record.work_order.status)}</Badge><h3 className="mt-3 text-xl font-semibold text-[var(--text-strong)]">{record.work_order.title}</h3><p className="mt-2 text-sm text-[var(--text-muted)]">{record.work_order.location_name} · {record.work_order.assigned_employee_name}</p></div><div className="grid gap-0 border border-[var(--border)] sm:grid-cols-3"><Fact label="Request" value={record.work_order.service_request_reference} /><Fact label="Target date" value={formatDate(record.work_order.target_date)} /><Fact label="Inspections" value={String(record.work_order.inspection_count)} /></div>{record.instructions ? <Section label="Instructions"><p className="whitespace-pre-wrap text-sm text-[var(--text-body)]">{record.instructions}</p></Section> : null}{record.completion_summary ? <Section label="Completion"><p className="whitespace-pre-wrap text-sm text-[var(--text-body)]">{record.completion_summary}</p></Section> : null}<Section label="Inspections">{record.inspections.length ? <div className="space-y-3">{record.inspections.map((inspection) => <div className="border border-[var(--border)] p-4" key={inspection.id}><div className="flex items-center justify-between gap-3"><Badge tone={inspection.outcome === "pass" ? "success" : "danger"}>{displayValue(inspection.outcome)}</Badge><span className="text-xs text-[var(--text-muted)]">{formatDateTime(inspection.created_at)}</span></div><p className="mt-3 whitespace-pre-wrap text-sm text-[var(--text-body)]">{inspection.notes}</p><p className="mt-2 text-xs text-[var(--text-muted)]">{inspection.inspector_name}</p></div>)}</div> : <p className="text-sm text-[var(--text-muted)]">No inspections recorded.</p>}</Section><Section label="History">{record.history.length ? <div className="space-y-3">{record.history.map((event) => <div className="border-l-2 border-[var(--border-strong)] pl-4" key={event.id}><p className="text-sm font-medium text-[var(--text-strong)]">{displayValue(event.event_type.replace("facilities.work_order.", ""))}</p><p className="mt-1 text-xs text-[var(--text-muted)]">{event.actor_name} · {formatDateTime(event.created_at)}</p></div>)}</div> : <p className="text-sm text-[var(--text-muted)]">No lifecycle events recorded.</p>}</Section></div>}</DialogBody><DialogFooter>{canManage && summary && !["completed", "cancelled"].includes(summary.status) ? <Button className="mr-auto" onClick={() => setAction("cancel")} type="button" variant="ghost">Cancel work order</Button> : null}{canOperate && summary?.status === "assigned" ? <Button disabled={saving} onClick={() => void start()} type="button">{saving ? "Starting…" : "Start work"}</Button> : null}{canOperate && summary?.status === "in_progress" ? <Button onClick={() => setAction("completion")} type="button">Submit completion</Button> : null}{canManage && summary?.status === "ready_for_inspection" ? <Button onClick={() => setAction("inspection")} type="button">Inspect work</Button> : null}<Button onClick={onClose} type="button" variant="secondary">Close</Button></DialogFooter></div>}</DialogShell>;
}

function Field({ children, label }: { children: React.ReactNode; label: string }) { return <div className="space-y-2"><Label>{label}</Label>{children}</div>; }
function Notice({ children }: { children: React.ReactNode }) { return <div className="border border-[var(--tone-warning-bd)] bg-[var(--tone-warning-bg)] p-4 text-sm text-[var(--text-body)]">{children}</div>; }
function Section({ children, label }: { children: React.ReactNode; label: string }) { return <section className="border-t border-[var(--border)] pt-5"><h4 className="mb-3 text-xs font-semibold uppercase tracking-[0.14em] text-[var(--text-muted)]">{label}</h4>{children}</section>; }
function Fact({ label, value }: { label: string; value: string }) { return <div className="border-b border-[var(--border)] p-4 last:border-b-0 sm:border-b-0 sm:border-r sm:last:border-r-0"><p className="text-xs uppercase tracking-[0.12em] text-[var(--text-muted)]">{label}</p><p className="mt-2 font-medium text-[var(--text-strong)]">{value}</p></div>; }

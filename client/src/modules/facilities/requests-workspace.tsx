/** Facilities service-request worklist and request drawers. */

import { useCallback, useEffect, useState } from "react";
import { ClipboardList, Plus, Search } from "lucide-react";
import toast from "react-hot-toast";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Table, TableControlsBar, TableControlsPagination, TableEmpty, TableError, TableLoading, TableScroll, TableWrap, TBody, TD, TH, THead, TR } from "@/components/ui/data-table";
import { DialogBody, DialogFooter, DialogHeader, DialogShell } from "@/components/ui/dialog";
import { Input, Label, Select, Textarea } from "@/components/ui/input";
import { usePageChrome } from "@/modules/admin/layouts/page-chrome";
import { useAuthStore } from "@/stores/auth-store";

import { facilitiesService, responseMessage } from "./service";
import type { FacilityLocation, FacilityPriority, FacilityRequestStatus, FacilityServiceRequestRecord, FacilityServiceRequestSummary, ServiceRequestPayload } from "./types";
import { allowed, displayValue, formatDateTime, priorityTone, requestTone } from "./ui";

const statuses: FacilityRequestStatus[] = ["open", "assigned", "resolved", "closed", "cancelled"];
const priorities: FacilityPriority[] = ["low", "normal", "high", "urgent"];
const emptyPayload: ServiceRequestPayload = { location_id: "", priority: "normal", summary: "", description: "" };

export function FacilitiesRequestsWorkspace() {
  const permissions = useAuthStore((state) => state.user?.permissions ?? []);
  const canRequest = allowed(permissions, "facilities:request");
  const canManage = allowed(permissions, "facilities:manage");
  const [records, setRecords] = useState<FacilityServiceRequestSummary[]>([]);
  const [search, setSearch] = useState("");
  const [status, setStatus] = useState<FacilityRequestStatus | "all">("all");
  const [priority, setPriority] = useState<FacilityPriority | "all">("all");
  const [page, setPage] = useState(1);
  const [totalPages, setTotalPages] = useState(1);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [createOpen, setCreateOpen] = useState(false);
  const [selected, setSelected] = useState<FacilityServiceRequestSummary | null>(null);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const response = await facilitiesService.requests({ page, per_page: 25, search: search.trim() || undefined, status: status === "all" ? undefined : status, priority: priority === "all" ? undefined : priority });
      if (!response.success || !response.data) throw new Error(responseMessage(response, "Service requests could not be loaded"));
      setRecords(response.data);
      setTotalPages(response.pagination?.total_pages ?? 1);
    } catch (loadError) {
      setError(loadError instanceof Error ? loadError.message : "Service requests could not be loaded");
    } finally {
      setLoading(false);
    }
  }, [page, priority, search, status]);

  useEffect(() => { void load(); }, [load]);
  usePageChrome("Service requests", canRequest ? <Button onClick={() => setCreateOpen(true)}><Plus className="size-4" />New request</Button> : null);
  const filtered = search || status !== "all" || priority !== "all";

  return <div className="space-y-6">
    <TableControlsBar>
      <Input aria-label="Search service requests" className="sm:w-72" leadingIcon={<Search />} onChange={(event) => { setPage(1); setSearch(event.target.value); }} placeholder="Search reference or summary" value={search} />
      <Select aria-label="Request status" className="sm:w-44" onChange={(event) => { setPage(1); setStatus(event.target.value as FacilityRequestStatus | "all"); }} value={status}><option value="all">All statuses</option>{statuses.map((value) => <option key={value} value={value}>{displayValue(value)}</option>)}</Select>
      <Select aria-label="Request priority" className="sm:w-40" onChange={(event) => { setPage(1); setPriority(event.target.value as FacilityPriority | "all"); }} value={priority}><option value="all">All priorities</option>{priorities.map((value) => <option key={value} value={value}>{displayValue(value)}</option>)}</Select>
      {!loading && records.length ? <TableControlsPagination onNext={() => setPage((value) => Math.min(totalPages, value + 1))} onPrevious={() => setPage((value) => Math.max(1, value - 1))} page={page} totalPages={totalPages} /> : null}
    </TableControlsBar>
    <TableWrap>{loading ? <TableLoading columns={7} label="Loading Facilities requests…" /> : error ? <TableError description={error} onRetry={() => void load()} /> : records.length === 0 ? <TableEmpty description={filtered ? "Change the current filters." : canRequest ? "Submit the first service request." : "No service requests are available."} icon={<ClipboardList />} title={filtered ? "No requests match" : "No service requests yet"} /> : <TableScroll><Table className="min-w-[1080px]"><THead><tr><TH>Request</TH><TH>Location</TH><TH>Reported by</TH><TH>Priority</TH><TH>Work order</TH><TH>Status</TH><TH>Updated</TH></tr></THead><TBody>{records.map((record) => <TR className="cursor-pointer" key={record.id} onClick={() => setSelected(record)}><TD><p className="font-medium text-[var(--text-strong)]">{record.summary}</p><p className="mt-1 font-tabular text-xs text-[var(--text-muted)]">{record.reference}</p></TD><TD className="text-[var(--text-muted)]">{record.location_name}</TD><TD className="text-[var(--text-muted)]">{record.reporter_name}</TD><TD><Badge tone={priorityTone(record.priority)}>{displayValue(record.priority)}</Badge></TD><TD className="font-tabular text-[var(--text-muted)]">{record.work_order_reference ?? "—"}</TD><TD><Badge tone={requestTone(record.status)}>{displayValue(record.status)}</Badge></TD><TD className="whitespace-nowrap text-[var(--text-muted)]">{formatDateTime(record.updated_at)}</TD></TR>)}</TBody></Table></TableScroll>}</TableWrap>
    <CreateRequestDrawer onClose={() => setCreateOpen(false)} onSaved={() => { setCreateOpen(false); void load(); }} open={createOpen} />
    <RequestDrawer canManage={canManage} canRequest={canRequest} onClose={() => setSelected(null)} onSaved={() => { setSelected(null); void load(); }} open={selected !== null} summary={selected} />
  </div>;
}

function CreateRequestDrawer({ onClose, onSaved, open }: { onClose: () => void; onSaved: () => void; open: boolean }) {
  const permissions = useAuthStore((state) => state.user?.permissions ?? []);
  const canManage = allowed(permissions, "facilities:manage");
  const [locations, setLocations] = useState<FacilityLocation[]>([]);
  const [form, setForm] = useState<ServiceRequestPayload>(emptyPayload);
  const [loadingLocations, setLoadingLocations] = useState(false);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    if (!open) return;
    setForm(emptyPayload);
    setLoadingLocations(true);
    void facilitiesService.locations({ status: "active" }).then((response) => { if (response.success && response.data) setLocations(response.data); else toast.error(responseMessage(response, "Locations could not be loaded")); }).catch(() => toast.error("Locations could not be loaded")).finally(() => setLoadingLocations(false));
  }, [open]);

  const submit = async (event: React.FormEvent) => {
    event.preventDefault();
    setSaving(true);
    try {
      const response = await facilitiesService.createRequest({ ...form, summary: form.summary.trim(), description: form.description.trim() });
      if (!response.success) throw new Error(responseMessage(response, "Service request could not be submitted"));
      toast.success("Service request submitted");
      onSaved();
    } catch (saveError) {
      toast.error(saveError instanceof Error ? saveError.message : "Service request could not be submitted");
    } finally {
      setSaving(false);
    }
  };

  return <DialogShell onClose={onClose} open={open}><DialogHeader onClose={saving ? undefined : onClose} title="New service request" /><form className="flex min-h-0 flex-1 flex-col" onSubmit={(event) => void submit(event)}><DialogBody><div className="space-y-5"><Field label="Location"><Select data-autofocus="true" disabled={loadingLocations || locations.length === 0} onChange={(event) => setForm({ ...form, location_id: event.target.value })} required value={form.location_id}><option value="">{loadingLocations ? "Loading locations…" : "Select a location"}</option>{locations.map((location) => <option key={location.id} value={location.id}>{location.name} · {location.code}</option>)}</Select>{!loadingLocations && locations.length === 0 ? <p className="text-xs leading-5 text-[var(--text-muted)]">{canManage ? "Add an active location in Locations before submitting a request." : "Ask a Facilities Manager to add an active location."}</p> : null}</Field><Field label="Priority"><Select onChange={(event) => setForm({ ...form, priority: event.target.value as FacilityPriority })} value={form.priority}>{priorities.map((value) => <option key={value} value={value}>{displayValue(value)}</option>)}</Select></Field><Field label="Summary"><Input maxLength={200} onChange={(event) => setForm({ ...form, summary: event.target.value })} required value={form.summary} /></Field><Field label="Description"><Textarea maxLength={6000} onChange={(event) => setForm({ ...form, description: event.target.value })} required rows={8} value={form.description} /></Field></div></DialogBody><DialogFooter><Button onClick={onClose} type="button" variant="secondary">Cancel</Button><Button disabled={saving || !form.location_id || !form.summary.trim() || !form.description.trim()} type="submit">{saving ? "Submitting…" : "Submit request"}</Button></DialogFooter></form></DialogShell>;
}

function RequestDrawer({ canManage, canRequest, onClose, onSaved, open, summary }: { canManage: boolean; canRequest: boolean; onClose: () => void; onSaved: () => void; open: boolean; summary: FacilityServiceRequestSummary | null }) {
  const [record, setRecord] = useState<FacilityServiceRequestRecord | null>(null);
  const [loading, setLoading] = useState(false);
  const [action, setAction] = useState<"cancel" | "close" | null>(null);
  const [reason, setReason] = useState("");
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    if (!open || !summary) return;
    setLoading(true); setRecord(null); setAction(null); setReason("");
    void facilitiesService.serviceRequest(summary.id).then((response) => { if (response.success && response.data) setRecord(response.data); else toast.error(responseMessage(response, "Service request could not be loaded")); }).catch(() => toast.error("Service request could not be loaded")).finally(() => setLoading(false));
  }, [open, summary]);

  const transition = async () => {
    if (!summary || !action) return;
    setSaving(true);
    try {
      const response = action === "cancel" ? await facilitiesService.cancelRequest(summary, reason.trim()) : await facilitiesService.closeRequest(summary, reason.trim());
      if (!response.success) throw new Error(responseMessage(response, "Service request could not be updated"));
      toast.success(action === "cancel" ? "Service request cancelled" : "Service request closed");
      onSaved();
    } catch (transitionError) {
      toast.error(transitionError instanceof Error ? transitionError.message : "Service request could not be updated");
    } finally {
      setSaving(false);
    }
  };

  return <DialogShell onClose={onClose} open={open} panelClassName="max-w-[720px]"><DialogHeader onClose={saving ? undefined : onClose} title={summary?.reference ?? "Service request"} />{action ? <div className="flex min-h-0 flex-1 flex-col"><DialogBody><div className="space-y-5"><p className="text-sm text-[var(--text-muted)]">{action === "cancel" ? "Cancel this service request." : "Close this resolved service request."}</p><Field label="Reason"><Textarea data-autofocus="true" maxLength={3000} onChange={(event) => setReason(event.target.value)} required rows={6} value={reason} /></Field></div></DialogBody><DialogFooter><Button onClick={() => { setAction(null); setReason(""); }} type="button" variant="secondary">Back</Button><Button disabled={saving || !reason.trim()} onClick={() => void transition()} type="button" variant={action === "cancel" ? "destructive" : "default"}>{saving ? "Saving…" : action === "cancel" ? "Cancel request" : "Close request"}</Button></DialogFooter></div> : <div className="flex min-h-0 flex-1 flex-col"><DialogBody>{loading || !record ? <div className="flex min-h-48 items-center justify-center text-sm text-[var(--text-muted)]">{loading ? "Loading request…" : "Request unavailable"}</div> : <div className="space-y-6"><div><div className="flex flex-wrap gap-2"><Badge tone={requestTone(record.request.status)}>{displayValue(record.request.status)}</Badge><Badge tone={priorityTone(record.request.priority)}>{displayValue(record.request.priority)}</Badge></div><h3 className="mt-3 text-xl font-semibold text-[var(--text-strong)]">{record.request.summary}</h3><p className="mt-2 text-sm text-[var(--text-muted)]">{record.request.location_name} · reported by {record.request.reporter_name}</p></div><Section label="Description"><p className="whitespace-pre-wrap text-sm text-[var(--text-body)]">{record.description}</p></Section>{record.resolution_summary ? <Section label="Resolution"><p className="whitespace-pre-wrap text-sm text-[var(--text-body)]">{record.resolution_summary}</p></Section> : null}{record.cancellation_reason ? <Section label="Cancellation"><p className="whitespace-pre-wrap text-sm text-[var(--text-body)]">{record.cancellation_reason}</p></Section> : null}<Section label="History">{record.history.length ? <div className="space-y-3">{record.history.map((event) => <div className="border-l-2 border-[var(--border-strong)] pl-4" key={event.id}><p className="text-sm font-medium text-[var(--text-strong)]">{displayValue(event.event_type.replace("facilities.request.", ""))}</p><p className="mt-1 text-xs text-[var(--text-muted)]">{event.actor_name} · {formatDateTime(event.created_at)}</p></div>)}</div> : <p className="text-sm text-[var(--text-muted)]">No lifecycle events recorded.</p>}</Section></div>}</DialogBody><DialogFooter>{canRequest && summary?.status === "open" ? <Button className="mr-auto" onClick={() => setAction("cancel")} type="button" variant="ghost">Cancel request</Button> : null}{canManage && summary?.status === "resolved" ? <Button onClick={() => setAction("close")} type="button">Close request</Button> : null}<Button onClick={onClose} type="button" variant="secondary">Close</Button></DialogFooter></div>}</DialogShell>;
}

function Field({ children, label }: { children: React.ReactNode; label: string }) { return <div className="space-y-2"><Label>{label}</Label>{children}</div>; }
function Section({ children, label }: { children: React.ReactNode; label: string }) { return <section className="border-t border-[var(--border)] pt-5"><h4 className="mb-3 text-xs font-semibold uppercase tracking-[0.14em] text-[var(--text-muted)]">{label}</h4>{children}</section>; }

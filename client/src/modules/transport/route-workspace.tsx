import { useCallback, useEffect, useState } from "react";
import { Link } from "@tanstack/react-router";
import { ArrowLeft, MapPin, Pencil, Plus, Trash2 } from "lucide-react";
import toast from "react-hot-toast";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Table, TableEmpty, TableError, TableLoading, TableScroll, TableWrap, TBody, TD, TH, THead, TR } from "@/components/ui/data-table";
import { DialogBody, DialogFooter, DialogHeader, DialogShell } from "@/components/ui/dialog";
import { Input, Label, Select, Textarea } from "@/components/ui/input";
import { usePageChrome } from "@/modules/admin/layouts/page-chrome";
import { useAuthStore } from "@/stores/auth-store";
import { responseMessage, transportService } from "./service";
import type { RouteDirection, RoutePayload, RouteRecord, RouteStatus, RouteStop, StopPayload } from "./types";
import { allowed, displayValue, statusTone } from "./ui";

export function TransportRouteWorkspace({ routeId }: { routeId: string }) {
  const permissions = useAuthStore((state) => state.user?.permissions ?? []); const canConfigure = allowed(permissions, "transport:configure");
  const [record, setRecord] = useState<RouteRecord | null>(null); const [loading, setLoading] = useState(true); const [error, setError] = useState<string | null>(null);
  const [editOpen, setEditOpen] = useState(false); const [stop, setStop] = useState<RouteStop | "new" | null>(null);
  const load = useCallback(async () => { setLoading(true); setError(null); try { const response = await transportService.route(routeId); if (!response.success || !response.data) throw new Error(responseMessage(response, "Route could not be loaded")); setRecord(response.data); } catch (loadError) { setError(loadError instanceof Error ? loadError.message : "Route could not be loaded"); } finally { setLoading(false); } }, [routeId]);
  useEffect(() => { void load(); }, [load]);
  usePageChrome(record?.code ?? "Route", canConfigure && record ? <div className="flex gap-2"><Button onClick={() => setEditOpen(true)} variant="secondary"><Pencil className="size-4" />Edit</Button><Button onClick={() => setStop("new")}><Plus className="size-4" />Add stop</Button></div> : null);
  if (loading) return <TableLoading columns={4} label="Loading route…" />;
  if (error || !record) return <TableError description={error ?? "Route not found"} onRetry={() => void load()} />;
  return <div className="space-y-6"><Link className="inline-flex items-center gap-2 text-sm font-medium text-[var(--text-muted)] hover:text-[var(--text-strong)]" to="/modules/transport/routes"><ArrowLeft className="size-4" />Routes</Link><section className="grid gap-4 border border-[var(--border)] bg-[var(--surface)] p-5 sm:grid-cols-4"><div className="sm:col-span-2"><p className="text-xs uppercase tracking-[0.14em] text-[var(--text-muted)]">Route</p><h2 className="mt-2 text-xl font-semibold text-[var(--text-strong)]">{record.name}</h2><p className="mt-1 font-tabular text-sm text-[var(--text-muted)]">{record.code}</p></div><Metric label="Direction" value={displayValue(record.direction)} /><div><p className="text-xs uppercase tracking-[0.14em] text-[var(--text-muted)]">Status</p><Badge className="mt-2" tone={statusTone(record.status)}>{displayValue(record.status)}</Badge></div></section><TableWrap><TableScroll><Table className="min-w-[760px]"><THead><tr><TH>Order</TH><TH>Stop</TH><TH>Planned time</TH><TH>Coordinates</TH></tr></THead><TBody>{record.stops.map((item) => <TR className={canConfigure ? "cursor-pointer" : undefined} key={item.id} onClick={() => canConfigure && setStop(item)}><TD>{item.stop_order}</TD><TD><p className="font-medium text-[var(--text-strong)]">{item.name}</p><p className="mt-1 font-tabular text-xs text-[var(--text-muted)]">{item.code}</p></TD><TD className="font-tabular">{item.planned_time.slice(0, 5)}</TD><TD className="text-[var(--text-muted)]">{item.latitude == null ? "—" : `${item.latitude}, ${item.longitude}`}</TD></TR>)}</TBody></Table></TableScroll>{record.stops.length === 0 ? <TableEmpty description={canConfigure ? "Add the first ordered stop." : "No stops are configured."} icon={<MapPin />} title="No stops yet" /> : null}</TableWrap><RouteEditDrawer onClose={() => setEditOpen(false)} onSaved={(value) => { setRecord(value); setEditOpen(false); toast.success("Route updated"); }} open={editOpen} record={record} /><StopDrawer onClose={() => setStop(null)} onSaved={(value) => { setRecord(value); setStop(null); }} open={stop !== null} record={typeof stop === "object" ? stop : null} routeId={record.id} /></div>;
}

function Metric({ label, value }: { label: string; value: string }) { return <div><p className="text-xs uppercase tracking-[0.14em] text-[var(--text-muted)]">{label}</p><p className="mt-2 font-medium text-[var(--text-strong)]">{value}</p></div>; }
function Field({ label, children }: { label: string; children: React.ReactNode }) { return <div className="space-y-2"><Label>{label}</Label>{children}</div>; }

function RouteEditDrawer({ record, open, onClose, onSaved }: { record: RouteRecord; open: boolean; onClose: () => void; onSaved: (record: RouteRecord) => void }) {
  const [form, setForm] = useState<RoutePayload & { status: RouteStatus }>({ code: record.code, name: record.name, direction: record.direction, notes: record.notes, status: record.status }); const [saving, setSaving] = useState(false);
  useEffect(() => { if (open) setForm({ code: record.code, name: record.name, direction: record.direction, notes: record.notes, status: record.status }); }, [open, record]);
  const submit = async (event: React.FormEvent) => { event.preventDefault(); setSaving(true); try { const response = await transportService.updateRoute(record.id, { ...form, code: form.code.trim(), name: form.name.trim(), notes: form.notes?.trim() || null, expected_version: record.version }); if (!response.success || !response.data) throw new Error(responseMessage(response, "Route could not be updated")); onSaved(response.data); } catch (saveError) { toast.error(saveError instanceof Error ? saveError.message : "Route could not be updated"); } finally { setSaving(false); } };
  return <DialogShell onClose={onClose} open={open} panelClassName="max-w-[600px]"><DialogHeader onClose={onClose} title="Edit route" /><form className="flex min-h-0 flex-1 flex-col overflow-hidden" onSubmit={(event) => void submit(event)}><DialogBody><div className="space-y-5"><Field label="Route code"><Input onChange={(event) => setForm({ ...form, code: event.target.value })} required value={form.code} /></Field><Field label="Route name"><Input onChange={(event) => setForm({ ...form, name: event.target.value })} required value={form.name} /></Field><div className="grid gap-4 sm:grid-cols-2"><Field label="Direction"><Select onChange={(event) => setForm({ ...form, direction: event.target.value as RouteDirection })} value={form.direction}><option value="inbound">Inbound</option><option value="outbound">Outbound</option></Select></Field><Field label="Status"><Select onChange={(event) => setForm({ ...form, status: event.target.value as RouteStatus })} value={form.status}><option value="active">Active</option><option value="inactive">Inactive</option></Select></Field></div><Field label="Notes"><Textarea onChange={(event) => setForm({ ...form, notes: event.target.value || null })} rows={5} value={form.notes ?? ""} /></Field></div></DialogBody><DialogFooter><Button onClick={onClose} type="button" variant="secondary">Cancel</Button><Button disabled={saving} type="submit">{saving ? "Saving…" : "Save changes"}</Button></DialogFooter></form></DialogShell>;
}

function StopDrawer({ routeId, record, open, onClose, onSaved }: { routeId: string; record: RouteStop | null; open: boolean; onClose: () => void; onSaved: (record: RouteRecord) => void }) {
  const empty: StopPayload = { code: "", name: "", stop_order: 1, planned_time: "07:00", latitude: null, longitude: null };
  const [form, setForm] = useState<StopPayload>(empty);
  const [saving, setSaving] = useState(false);
  useEffect(() => {
    if (open) setForm(record ? { code: record.code, name: record.name, stop_order: record.stop_order, planned_time: record.planned_time.slice(0, 5), latitude: record.latitude, longitude: record.longitude } : empty);
  }, [open, record]);
  const submit = async (event: React.FormEvent) => {
    event.preventDefault(); setSaving(true);
    try {
      const response = record ? await transportService.updateStop(routeId, record.id, { ...form, expected_version: record.version }) : await transportService.createStop(routeId, form);
      if (!response.success || !response.data) throw new Error(responseMessage(response, "Stop could not be saved"));
      toast.success(record ? "Stop updated" : "Stop added"); onSaved(response.data);
    } catch (saveError) { toast.error(saveError instanceof Error ? saveError.message : "Stop could not be saved"); }
    finally { setSaving(false); }
  };
  const remove = async () => {
    if (!record) return; setSaving(true);
    try {
      const response = await transportService.removeStop(routeId, record.id, record.version);
      if (!response.success || !response.data) throw new Error(responseMessage(response, "Stop could not be removed"));
      toast.success("Stop removed"); onSaved(response.data);
    } catch (removeError) { toast.error(removeError instanceof Error ? removeError.message : "Stop could not be removed"); }
    finally { setSaving(false); }
  };
  return <DialogShell onClose={onClose} open={open} panelClassName="max-w-[600px]">
    <DialogHeader onClose={onClose} title={record ? "Edit stop" : "Add stop"} />
    <form className="flex min-h-0 flex-1 flex-col overflow-hidden" onSubmit={(event) => void submit(event)}>
      <DialogBody><div className="space-y-5">
        <div className="grid gap-4 sm:grid-cols-2"><Field label="Stop code"><Input onChange={(event) => setForm({ ...form, code: event.target.value })} required value={form.code} /></Field><Field label="Order"><Input min={1} onChange={(event) => setForm({ ...form, stop_order: Number(event.target.value) })} required type="number" value={form.stop_order} /></Field></div>
        <Field label="Stop name"><Input onChange={(event) => setForm({ ...form, name: event.target.value })} required value={form.name} /></Field>
        <Field label="Planned time"><Input onChange={(event) => setForm({ ...form, planned_time: event.target.value })} required type="time" value={form.planned_time} /></Field>
        <div className="grid gap-4 sm:grid-cols-2"><Field label="Latitude"><Input onChange={(event) => setForm({ ...form, latitude: event.target.value ? Number(event.target.value) : null })} step="any" type="number" value={form.latitude ?? ""} /></Field><Field label="Longitude"><Input onChange={(event) => setForm({ ...form, longitude: event.target.value ? Number(event.target.value) : null })} step="any" type="number" value={form.longitude ?? ""} /></Field></div>
      </div></DialogBody>
      <DialogFooter>{record ? <Button className="mr-auto" disabled={saving} onClick={() => void remove()} type="button" variant="destructive"><Trash2 className="size-4" />Remove</Button> : null}<Button onClick={onClose} type="button" variant="secondary">Cancel</Button><Button disabled={saving || !form.code.trim() || !form.name.trim()} type="submit">{saving ? "Saving…" : "Save stop"}</Button></DialogFooter>
    </form>
  </DialogShell>;
}

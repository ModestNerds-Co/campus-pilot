import { useCallback, useEffect, useState } from "react";
import { useNavigate } from "@tanstack/react-router";
import { MapPinned, Plus, Search } from "lucide-react";
import toast from "react-hot-toast";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Table, TableControlsBar, TableControlsPagination, TableEmpty, TableError, TableLoading, TableScroll, TableWrap, TBody, TD, TH, THead, TR } from "@/components/ui/data-table";
import { DialogBody, DialogFooter, DialogHeader, DialogShell } from "@/components/ui/dialog";
import { Input, Label, Select, Textarea } from "@/components/ui/input";
import { usePageChrome } from "@/modules/admin/layouts/page-chrome";
import { useAuthStore } from "@/stores/auth-store";
import { responseMessage, transportService } from "./service";
import type { RouteDirection, RoutePayload, RouteStatus, RouteSummary } from "./types";
import { allowed, dateTimeLabel, displayValue, statusTone } from "./ui";

export function TransportRoutesWorkspace() {
  const navigate = useNavigate();
  const permissions = useAuthStore((state) => state.user?.permissions ?? []);
  const canConfigure = allowed(permissions, "transport:configure");
  const [records, setRecords] = useState<RouteSummary[]>([]);
  const [search, setSearch] = useState("");
  const [status, setStatus] = useState<RouteStatus | "all">("all");
  const [direction, setDirection] = useState<RouteDirection | "all">("all");
  const [page, setPage] = useState(1); const [totalPages, setTotalPages] = useState(1);
  const [loading, setLoading] = useState(true); const [error, setError] = useState<string | null>(null);
  const [createOpen, setCreateOpen] = useState(false);

  const load = useCallback(async () => {
    setLoading(true); setError(null);
    try {
      const response = await transportService.routes({ page, per_page: 25, search: search.trim() || undefined, status: status === "all" ? undefined : status, direction: direction === "all" ? undefined : direction });
      if (!response.success || !response.data) throw new Error(responseMessage(response, "Routes could not be loaded"));
      setRecords(response.data.routes); setTotalPages(response.pagination?.total_pages ?? 1);
    } catch (loadError) { setError(loadError instanceof Error ? loadError.message : "Routes could not be loaded"); }
    finally { setLoading(false); }
  }, [direction, page, search, status]);

  useEffect(() => { void load(); }, [load]);
  usePageChrome("Routes", canConfigure ? <Button onClick={() => setCreateOpen(true)}><Plus className="size-4" />Add route</Button> : null);

  return <div className="space-y-6">
    <TableControlsBar>
      <Input aria-label="Search routes" className="sm:w-72" leadingIcon={<Search />} onChange={(event) => { setPage(1); setSearch(event.target.value); }} placeholder="Search code or name" value={search} />
      <Select aria-label="Route direction" className="sm:w-40" onChange={(event) => { setPage(1); setDirection(event.target.value as RouteDirection | "all"); }} value={direction}><option value="all">All directions</option><option value="inbound">Inbound</option><option value="outbound">Outbound</option></Select>
      <Select aria-label="Route status" className="sm:w-36" onChange={(event) => { setPage(1); setStatus(event.target.value as RouteStatus | "all"); }} value={status}><option value="all">All statuses</option><option value="active">Active</option><option value="inactive">Inactive</option></Select>
      {!loading && records.length ? <TableControlsPagination onNext={() => setPage((value) => Math.min(totalPages, value + 1))} onPrevious={() => setPage((value) => Math.max(1, value - 1))} page={page} totalPages={totalPages} /> : null}
    </TableControlsBar>
    <TableWrap>{loading ? <TableLoading columns={6} label="Loading routes…" /> : error ? <TableError description={error} onRetry={() => void load()} /> : records.length === 0 ? <TableEmpty description={canConfigure ? "Add a route, then define its ordered stops." : "No Transport routes are available."} icon={<MapPinned />} title="No routes yet" /> : <TableScroll><Table className="min-w-[900px]"><THead><tr><TH>Route</TH><TH>Direction</TH><TH>Stops</TH><TH>Active riders</TH><TH>Status</TH><TH>Updated</TH></tr></THead><TBody>{records.map((record) => <TR className="cursor-pointer" key={record.id} onClick={() => void navigate({ to: "/modules/transport/routes/$routeId", params: { routeId: record.id } })}><TD><p className="font-medium text-[var(--text-strong)]">{record.name}</p><p className="mt-1 font-tabular text-xs text-[var(--text-muted)]">{record.code}</p></TD><TD className="text-[var(--text-muted)]">{displayValue(record.direction)}</TD><TD>{record.stop_count}</TD><TD>{record.active_rider_count}</TD><TD><Badge tone={statusTone(record.status)}>{displayValue(record.status)}</Badge></TD><TD className="whitespace-nowrap text-[var(--text-muted)]">{dateTimeLabel(record.updated_at)}</TD></TR>)}</TBody></Table></TableScroll>}</TableWrap>
    <RouteDrawer onClose={() => setCreateOpen(false)} onSaved={(record) => { setCreateOpen(false); toast.success(`Route ${record.code} added`); void navigate({ to: "/modules/transport/routes/$routeId", params: { routeId: record.id } }); }} open={createOpen} />
  </div>;
}

function RouteDrawer({ open, onClose, onSaved }: { open: boolean; onClose: () => void; onSaved: (record: RouteSummary) => void }) {
  const [form, setForm] = useState<RoutePayload>({ code: "", name: "", direction: "inbound", notes: null }); const [saving, setSaving] = useState(false);
  useEffect(() => { if (open) setForm({ code: "", name: "", direction: "inbound", notes: null }); }, [open]);
  const submit = async (event: React.FormEvent) => { event.preventDefault(); setSaving(true); try { const response = await transportService.createRoute({ ...form, code: form.code.trim(), name: form.name.trim(), notes: form.notes?.trim() || null }); if (!response.success || !response.data) throw new Error(responseMessage(response, "Route could not be added")); onSaved(response.data); } catch (saveError) { toast.error(saveError instanceof Error ? saveError.message : "Route could not be added"); } finally { setSaving(false); } };
  return <DialogShell onClose={onClose} open={open} panelClassName="max-w-[600px]"><DialogHeader onClose={onClose} title="Add route" /><form className="flex min-h-0 flex-1 flex-col overflow-hidden" onSubmit={(event) => void submit(event)}><DialogBody><div className="space-y-5"><Field label="Route code"><Input data-autofocus="true" maxLength={24} onChange={(event) => setForm({ ...form, code: event.target.value })} required value={form.code} /></Field><Field label="Route name"><Input maxLength={160} onChange={(event) => setForm({ ...form, name: event.target.value })} required value={form.name} /></Field><Field label="Direction"><Select onChange={(event) => setForm({ ...form, direction: event.target.value as RouteDirection })} value={form.direction}><option value="inbound">Inbound</option><option value="outbound">Outbound</option></Select></Field><Field label="Notes"><Textarea maxLength={2000} onChange={(event) => setForm({ ...form, notes: event.target.value || null })} rows={5} value={form.notes ?? ""} /></Field></div></DialogBody><DialogFooter><Button onClick={onClose} type="button" variant="secondary">Cancel</Button><Button disabled={saving || !form.code.trim() || !form.name.trim()} type="submit">{saving ? "Adding…" : "Add route"}</Button></DialogFooter></form></DialogShell>;
}

function Field({ label, children }: { label: string; children: React.ReactNode }) { return <div className="space-y-2"><Label>{label}</Label>{children}</div>; }


/** Facilities location hierarchy and manager workflows. */

import { useCallback, useEffect, useState } from "react";
import { Building2, MapPin, Plus, Search } from "lucide-react";
import toast from "react-hot-toast";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Table, TableControlsBar, TableEmpty, TableError, TableLoading, TableScroll, TableWrap, TBody, TD, TH, THead, TR } from "@/components/ui/data-table";
import { DialogBody, DialogFooter, DialogHeader, DialogShell } from "@/components/ui/dialog";
import { Input, Label, Select, Textarea } from "@/components/ui/input";
import { usePageChrome } from "@/modules/admin/layouts/page-chrome";

import { facilitiesService, responseMessage } from "./service";
import type { FacilityLocation, FacilityLocationKind, LocationPayload } from "./types";
import { displayValue, formatDateTime } from "./ui";

const kinds: FacilityLocationKind[] = ["site", "building", "floor", "room", "external_area"];
const emptyPayload: LocationPayload = { parent_id: null, kind: "site", code: "", name: "", capacity: null, notes: null };

export function FacilitiesLocationsWorkspace() {
  const [records, setRecords] = useState<FacilityLocation[]>([]);
  const [search, setSearch] = useState("");
  const [kind, setKind] = useState<FacilityLocationKind | "all">("all");
  const [status, setStatus] = useState<"active" | "archived" | "all">("active");
  const [selected, setSelected] = useState<FacilityLocation | null>(null);
  const [createOpen, setCreateOpen] = useState(false);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const response = await facilitiesService.locations({
        search: search.trim() || undefined,
        kind: kind === "all" ? undefined : kind,
        status: status === "all" ? undefined : status,
      });
      if (!response.success || !response.data) throw new Error(responseMessage(response, "Locations could not be loaded"));
      setRecords(response.data);
    } catch (loadError) {
      setError(loadError instanceof Error ? loadError.message : "Locations could not be loaded");
    } finally {
      setLoading(false);
    }
  }, [kind, search, status]);

  useEffect(() => { void load(); }, [load]);
  usePageChrome("Locations", <Button onClick={() => setCreateOpen(true)}><Plus className="size-4" />Add location</Button>);
  const filtered = search || kind !== "all" || status !== "active";

  return <div className="space-y-6">
    <TableControlsBar>
      <Input aria-label="Search locations" className="sm:w-72" leadingIcon={<Search />} onChange={(event) => setSearch(event.target.value)} placeholder="Search name or code" value={search} />
      <Select aria-label="Location kind" className="sm:w-44" onChange={(event) => setKind(event.target.value as FacilityLocationKind | "all")} value={kind}><option value="all">All location types</option>{kinds.map((value) => <option key={value} value={value}>{displayValue(value)}</option>)}</Select>
      <Select aria-label="Location status" className="sm:w-40" onChange={(event) => setStatus(event.target.value as "active" | "archived" | "all")} value={status}><option value="active">Active</option><option value="archived">Archived</option><option value="all">All statuses</option></Select>
    </TableControlsBar>
    <TableWrap>
      {loading ? <TableLoading columns={6} label="Loading Facilities locations…" /> : error ? <TableError description={error} onRetry={() => void load()} /> : records.length === 0 ? <TableEmpty description={filtered ? "Change the current filters." : "Add the first campus site."} icon={<Building2 />} title={filtered ? "No locations match" : "No locations yet"} /> : <TableScroll><Table className="min-w-[900px]"><THead><tr><TH>Location</TH><TH>Type</TH><TH>Parent</TH><TH>Capacity</TH><TH>Status</TH><TH>Updated</TH></tr></THead><TBody>{records.map((record) => <TR className="cursor-pointer" key={record.id} onClick={() => setSelected(record)}><TD><p className="font-medium text-[var(--text-strong)]">{record.name}</p><p className="mt-1 font-tabular text-xs text-[var(--text-muted)]">{record.code} · {record.child_count} child locations</p></TD><TD className="text-[var(--text-muted)]">{displayValue(record.kind)}</TD><TD className="text-[var(--text-muted)]">{record.parent_name ?? "Campus root"}</TD><TD className="font-tabular text-[var(--text-muted)]">{record.capacity ?? "—"}</TD><TD><Badge tone={record.status === "active" ? "success" : "neutral"}>{displayValue(record.status)}</Badge></TD><TD className="whitespace-nowrap text-[var(--text-muted)]">{formatDateTime(record.updated_at)}</TD></TR>)}</TBody></Table></TableScroll>}
    </TableWrap>
    <LocationDrawer locations={records} onClose={() => setCreateOpen(false)} onSaved={() => { setCreateOpen(false); void load(); }} open={createOpen} />
    <LocationDrawer locations={records} onClose={() => setSelected(null)} onSaved={() => { setSelected(null); void load(); }} open={selected !== null} record={selected} />
  </div>;
}

function LocationDrawer({ locations, onClose, onSaved, open, record }: { locations: FacilityLocation[]; onClose: () => void; onSaved: () => void; open: boolean; record?: FacilityLocation | null }) {
  const [form, setForm] = useState<LocationPayload>(emptyPayload);
  const [saving, setSaving] = useState(false);
  const [archiveMode, setArchiveMode] = useState(false);
  const [archiveReason, setArchiveReason] = useState("");

  useEffect(() => {
    if (!open) return;
    setArchiveMode(false);
    setArchiveReason("");
    setForm(record ? { parent_id: record.parent_id, kind: record.kind, code: record.code, name: record.name, capacity: record.capacity, notes: record.notes } : emptyPayload);
  }, [open, record]);

  const save = async (event: React.FormEvent) => {
    event.preventDefault();
    setSaving(true);
    try {
      const payload = { ...form, code: form.code.trim(), name: form.name.trim(), notes: form.notes?.trim() || null };
      const response = record ? await facilitiesService.updateLocation(record, payload) : await facilitiesService.createLocation(payload);
      if (!response.success) throw new Error(responseMessage(response, "Location could not be saved"));
      toast.success(record ? "Location updated" : "Location added");
      onSaved();
    } catch (saveError) {
      toast.error(saveError instanceof Error ? saveError.message : "Location could not be saved");
    } finally {
      setSaving(false);
    }
  };

  const archive = async () => {
    if (!record) return;
    setSaving(true);
    try {
      const response = await facilitiesService.archiveLocation(record, archiveReason.trim());
      if (!response.success) throw new Error(responseMessage(response, "Location could not be archived"));
      toast.success("Location archived");
      onSaved();
    } catch (archiveError) {
      toast.error(archiveError instanceof Error ? archiveError.message : "Location could not be archived");
    } finally {
      setSaving(false);
    }
  };

  const parents = locations.filter((location) => location.id !== record?.id && location.status === "active");
  return <DialogShell onClose={onClose} open={open}><DialogHeader onClose={saving ? undefined : onClose} title={record ? record.name : "Add Facilities location"} />
    {archiveMode && record ? <div className="flex min-h-0 flex-1 flex-col"><DialogBody><div className="space-y-5"><div className="border border-[var(--tone-warning-bd)] bg-[var(--tone-warning-bg)] p-4 text-sm text-[var(--text-body)]">Archiving removes this location from new requests. Active child locations or work prevent the change.</div><Field label="Reason"><Textarea data-autofocus="true" maxLength={2000} onChange={(event) => setArchiveReason(event.target.value)} required rows={6} value={archiveReason} /></Field></div></DialogBody><DialogFooter><Button onClick={() => setArchiveMode(false)} type="button" variant="secondary">Back</Button><Button disabled={saving || !archiveReason.trim()} onClick={() => void archive()} type="button" variant="destructive">{saving ? "Archiving…" : "Archive location"}</Button></DialogFooter></div> : <form className="flex min-h-0 flex-1 flex-col" onSubmit={(event) => void save(event)}><DialogBody><div className="space-y-5">
      {record ? <div className="flex items-center gap-3 border border-[var(--border)] bg-[var(--surface-muted)] p-4"><MapPin className="size-5 text-[var(--brand-strong)]" /><div><p className="font-medium text-[var(--text-strong)]">{record.code}</p><p className="text-sm text-[var(--text-muted)]">{displayValue(record.status)} · version {record.version}</p></div></div> : null}
      <div className="grid gap-4 sm:grid-cols-2"><Field label="Type"><Select onChange={(event) => { const nextKind = event.target.value as FacilityLocationKind; setForm({ ...form, kind: nextKind, parent_id: nextKind === "site" ? null : form.parent_id }); }} value={form.kind}>{kinds.map((value) => <option key={value} value={value}>{displayValue(value)}</option>)}</Select></Field><Field label="Code"><Input data-autofocus="true" maxLength={40} onChange={(event) => setForm({ ...form, code: event.target.value })} required value={form.code} /></Field></div>
      <Field label="Name"><Input maxLength={160} onChange={(event) => setForm({ ...form, name: event.target.value })} required value={form.name} /></Field>
      {form.kind !== "site" ? <Field label="Parent location"><Select onChange={(event) => setForm({ ...form, parent_id: event.target.value || null })} required value={form.parent_id ?? ""}><option value="">Select a parent</option>{parents.map((location) => <option key={location.id} value={location.id}>{location.name} · {displayValue(location.kind)}</option>)}</Select></Field> : null}
      <Field label="Capacity"><Input min={1} onChange={(event) => setForm({ ...form, capacity: event.target.value ? Number(event.target.value) : null })} type="number" value={form.capacity ?? ""} /></Field>
      <Field label="Notes"><Textarea maxLength={4000} onChange={(event) => setForm({ ...form, notes: event.target.value || null })} rows={5} value={form.notes ?? ""} /></Field>
    </div></DialogBody><DialogFooter>{record?.status === "active" ? <Button className="mr-auto" onClick={() => setArchiveMode(true)} type="button" variant="ghost">Archive</Button> : null}<Button onClick={onClose} type="button" variant="secondary">Cancel</Button><Button disabled={saving || !form.code.trim() || !form.name.trim() || (form.kind !== "site" && !form.parent_id)} type="submit">{saving ? "Saving…" : record ? "Save changes" : "Add location"}</Button></DialogFooter></form>}
  </DialogShell>;
}

function Field({ children, label }: { children: React.ReactNode; label: string }) {
  return <div className="space-y-2"><Label>{label}</Label>{children}</div>;
}

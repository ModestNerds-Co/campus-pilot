/** Residence directory and maintenance drawer for the Hostel module. */

import { useCallback, useEffect, useState } from "react";
import { Building2, Plus, Search } from "lucide-react";
import toast from "react-hot-toast";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Table, TableControlsBar, TableControlsPagination, TableEmpty, TableError, TableLoading, TableScroll, TableWrap, TBody, TD, TH, THead, TR } from "@/components/ui/data-table";
import { DialogBody, DialogFooter, DialogHeader, DialogShell } from "@/components/ui/dialog";
import { Input, Label, Select, Textarea } from "@/components/ui/input";
import { usePageChrome } from "@/modules/admin/layouts/page-chrome";
import { useAuthStore } from "@/stores/auth-store";

import { hostelService, responseMessage } from "./service";
import type { Residence, ResidenceStatus } from "./types";
import { statusTone } from "./ui";

export function HostelResidencesWorkspace() {
  const permissions = useAuthStore((state) => state.user?.permissions ?? []);
  const canCreate = allowed(permissions, "hostel:create");
  const canEdit = allowed(permissions, "hostel:edit");
  const [records, setRecords] = useState<Residence[]>([]);
  const [search, setSearch] = useState("");
  const [status, setStatus] = useState("all");
  const [page, setPage] = useState(1);
  const [totalPages, setTotalPages] = useState(1);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [drawerOpen, setDrawerOpen] = useState(false);
  const [selected, setSelected] = useState<Residence | null>(null);

  const load = useCallback(async () => {
    setLoading(true); setError(null);
    try {
      const response = await hostelService.residences({ page, per_page: 25, search: search.trim() || undefined, status: status === "all" ? undefined : status });
      if (!response.success || !response.data) throw new Error(responseMessage(response, "Residences could not be loaded"));
      setRecords(response.data.residences); setTotalPages(response.pagination?.total_pages ?? 1);
    } catch (loadError) { setError(loadError instanceof Error ? loadError.message : "Residences could not be loaded"); }
    finally { setLoading(false); }
  }, [page, search, status]);
  useEffect(() => { void load(); }, [load]);
  usePageChrome("Residences", canCreate ? <Button onClick={() => { setSelected(null); setDrawerOpen(true); }}><Plus className="size-4" />Add residence</Button> : null);

  return <div className="space-y-6">
    <TableControlsBar>
      <Input aria-label="Search residences" className="sm:w-72" leadingIcon={<Search />} onChange={(event) => { setPage(1); setSearch(event.target.value); }} placeholder="Search code or name" value={search} />
      <Select aria-label="Residence status" className="sm:w-44" onChange={(event) => { setPage(1); setStatus(event.target.value); }} value={status}><option value="all">All statuses</option><option value="active">Active</option><option value="inactive">Inactive</option></Select>
      {!loading && records.length ? <TableControlsPagination onNext={() => setPage((value) => Math.min(totalPages, value + 1))} onPrevious={() => setPage((value) => Math.max(1, value - 1))} page={page} totalPages={totalPages} /> : null}
    </TableControlsBar>
    <TableWrap>{loading ? <TableLoading columns={6} label="Loading residences…" /> : error ? <TableError description={error} onRetry={() => void load()} /> : records.length === 0 ? <TableEmpty description={search || status !== "all" ? "Change the current filters." : "Add the campus boarding residences."} icon={<Building2 />} title={search || status !== "all" ? "No residences match" : "No residences yet"} /> : <TableScroll><Table className="min-w-[820px]"><THead><tr><TH>Residence</TH><TH>Status</TH><TH>Rooms</TH><TH>Capacity</TH><TH>Occupied</TH><TH>Available</TH></tr></THead><TBody>{records.map((record) => <TR className={canEdit ? "cursor-pointer" : undefined} key={record.id} onClick={() => { if (canEdit) { setSelected(record); setDrawerOpen(true); } }}><TD><span className="font-medium text-[var(--text-strong)]">{record.name}</span><p className="mt-1 text-xs text-[var(--text-muted)]">{record.code}</p></TD><TD><Badge tone={statusTone(record.status)}>{record.status === "active" ? "Active" : "Inactive"}</Badge></TD><TD className="font-tabular">{record.room_count}</TD><TD className="font-tabular">{record.bed_capacity}</TD><TD className="font-tabular">{record.occupied_count}</TD><TD className="font-tabular">{record.available_beds}</TD></TR>)}</TBody></Table></TableScroll>}</TableWrap>
    <ResidenceDrawer onClose={() => setDrawerOpen(false)} onSaved={() => { setDrawerOpen(false); void load(); }} open={drawerOpen} record={selected} />
  </div>;
}

function ResidenceDrawer({ open, record, onClose, onSaved }: { open: boolean; record: Residence | null; onClose: () => void; onSaved: () => void }) {
  const [code, setCode] = useState(""); const [name, setName] = useState(""); const [description, setDescription] = useState(""); const [status, setStatus] = useState<ResidenceStatus>("active"); const [saving, setSaving] = useState(false);
  useEffect(() => { if (open) { setCode(record?.code ?? ""); setName(record?.name ?? ""); setDescription(record?.description ?? ""); setStatus(record?.status ?? "active"); } }, [open, record]);
  const save = async (event: React.FormEvent) => {
    event.preventDefault(); setSaving(true);
    try {
      const payload = { code: code.trim(), name: name.trim(), description: description.trim() || null };
      const response = record ? await hostelService.updateResidence(record, { ...payload, status }) : await hostelService.createResidence(payload);
      if (!response.success) throw new Error(responseMessage(response, "Residence could not be saved"));
      toast.success(record ? "Residence updated" : "Residence added"); onSaved();
    } catch (saveError) { toast.error(saveError instanceof Error ? saveError.message : "Residence could not be saved"); }
    finally { setSaving(false); }
  };
  return <DialogShell onClose={onClose} open={open}><form onSubmit={(event) => void save(event)}><DialogHeader onClose={onClose} title={record ? "Edit residence" : "Add residence"} /><DialogBody><div className="space-y-5"><Field label="Code"><Input data-autofocus="true" maxLength={30} onChange={(event) => setCode(event.target.value)} required value={code} /></Field><Field label="Name"><Input maxLength={160} onChange={(event) => setName(event.target.value)} required value={name} /></Field><Field label="Description"><Textarea maxLength={1000} onChange={(event) => setDescription(event.target.value)} rows={5} value={description} /></Field>{record ? <Field label="Status"><Select onChange={(event) => setStatus(event.target.value as ResidenceStatus)} value={status}><option value="active">Active</option><option value="inactive">Inactive</option></Select></Field> : null}</div></DialogBody><DialogFooter><Button onClick={onClose} type="button" variant="secondary">Cancel</Button><Button disabled={saving || !code.trim() || !name.trim()} type="submit">{saving ? "Saving…" : "Save residence"}</Button></DialogFooter></form></DialogShell>;
}

function Field({ label, children }: { label: string; children: React.ReactNode }) { return <div className="space-y-2"><Label>{label}</Label>{children}</div>; }
function allowed(permissions: string[], permission: string) { return permissions.includes("*") || permissions.includes(permission); }

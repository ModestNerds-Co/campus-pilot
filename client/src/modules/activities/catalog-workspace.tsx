/** Activities catalog management and archive workflow. */

import { useCallback, useEffect, useState } from "react";
import { ListChecks, Plus, Search } from "lucide-react";
import toast from "react-hot-toast";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Table, TableControlsBar, TableEmpty, TableError, TableLoading, TableScroll, TableWrap, TBody, TD, TH, THead, TR } from "@/components/ui/data-table";
import { DialogBody, DialogFooter, DialogHeader, DialogShell } from "@/components/ui/dialog";
import { Input, Label, Select, Textarea } from "@/components/ui/input";
import { usePageChrome } from "@/modules/admin/layouts/page-chrome";

import { activitiesService, responseMessage } from "./service";
import type { ActivityCatalogItem, ActivityCatalogStatus, ActivityCategory, CatalogPayload } from "./types";
import { displayValue, formatDateTime, statusTone } from "./ui";

const categories: ActivityCategory[] = ["sport", "club", "arts", "service", "society", "academic_enrichment", "other"];
const emptyForm: CatalogPayload = { code: "", name: "", category: "sport", description: null };

export function ActivitiesCatalogWorkspace() {
  const [records, setRecords] = useState<ActivityCatalogItem[]>([]);
  const [search, setSearch] = useState("");
  const [category, setCategory] = useState<ActivityCategory | "all">("all");
  const [status, setStatus] = useState<ActivityCatalogStatus | "all">("active");
  const [selected, setSelected] = useState<ActivityCatalogItem | null>(null);
  const [createOpen, setCreateOpen] = useState(false);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    setLoading(true); setError(null);
    try {
      const response = await activitiesService.catalog({ search: search.trim() || undefined, category: category === "all" ? undefined : category, status: status === "all" ? undefined : status });
      if (!response.success || !response.data) throw new Error(responseMessage(response, "Activities could not be loaded"));
      setRecords(response.data);
    } catch (loadError) { setError(loadError instanceof Error ? loadError.message : "Activities could not be loaded"); }
    finally { setLoading(false); }
  }, [category, search, status]);

  useEffect(() => { void load(); }, [load]);
  usePageChrome("Catalog", <Button onClick={() => setCreateOpen(true)}><Plus className="size-4" />Add activity</Button>);
  const filtered = search || category !== "all" || status !== "active";

  return <div className="space-y-6">
    <TableControlsBar>
      <Input aria-label="Search activities" className="sm:w-72" leadingIcon={<Search />} onChange={(event) => setSearch(event.target.value)} placeholder="Search name or code" value={search} />
      <Select aria-label="Activity category" className="sm:w-52" onChange={(event) => setCategory(event.target.value as ActivityCategory | "all")} value={category}><option value="all">All categories</option>{categories.map((value) => <option key={value} value={value}>{displayValue(value)}</option>)}</Select>
      <Select aria-label="Activity status" className="sm:w-40" onChange={(event) => setStatus(event.target.value as ActivityCatalogStatus | "all")} value={status}><option value="active">Active</option><option value="archived">Archived</option><option value="all">All statuses</option></Select>
    </TableControlsBar>
    <TableWrap>{loading ? <TableLoading columns={5} label="Loading activities…" /> : error ? <TableError description={error} onRetry={() => void load()} /> : records.length === 0 ? <TableEmpty description={filtered ? "Change the current filters." : "Add the first activity."} icon={<ListChecks />} title={filtered ? "No activities match" : "No activities yet"} /> : <TableScroll><Table className="min-w-[820px]"><THead><tr><TH>Activity</TH><TH>Category</TH><TH>Status</TH><TH>Groups</TH><TH>Updated</TH></tr></THead><TBody>{records.map((record) => <TR className="cursor-pointer" key={record.id} onClick={() => setSelected(record)}><TD><p className="font-medium text-[var(--text-strong)]">{record.name}</p><p className="mt-1 font-tabular text-xs text-[var(--text-muted)]">{record.code}</p></TD><TD className="text-[var(--text-muted)]">{displayValue(record.category)}</TD><TD><Badge tone={statusTone(record.status)}>{displayValue(record.status)}</Badge></TD><TD className="text-[var(--text-muted)]">Managed in Groups</TD><TD className="whitespace-nowrap text-[var(--text-muted)]">{formatDateTime(record.updated_at)}</TD></TR>)}</TBody></Table></TableScroll>}</TableWrap>
    <CatalogDrawer onClose={() => setCreateOpen(false)} onSaved={() => { setCreateOpen(false); void load(); }} open={createOpen} />
    <CatalogDrawer onClose={() => setSelected(null)} onSaved={() => { setSelected(null); void load(); }} open={selected !== null} record={selected} />
  </div>;
}

function CatalogDrawer({ onClose, onSaved, open, record }: { onClose: () => void; onSaved: () => void; open: boolean; record?: ActivityCatalogItem | null }) {
  const [form, setForm] = useState<CatalogPayload>(emptyForm);
  const [archiveMode, setArchiveMode] = useState(false);
  const [reason, setReason] = useState("");
  const [saving, setSaving] = useState(false);

  useEffect(() => { if (!open) return; setArchiveMode(false); setReason(""); setForm(record ? { code: record.code, name: record.name, category: record.category, description: record.description } : emptyForm); }, [open, record]);

  const save = async (event: React.FormEvent) => {
    event.preventDefault(); setSaving(true);
    try {
      const payload = { ...form, code: form.code.trim(), name: form.name.trim(), description: form.description?.trim() || null };
      const response = record ? await activitiesService.updateCatalogItem(record, payload) : await activitiesService.createCatalogItem(payload);
      if (!response.success) throw new Error(responseMessage(response, "Activity could not be saved"));
      toast.success(record ? "Activity updated" : "Activity added"); onSaved();
    } catch (saveError) { toast.error(saveError instanceof Error ? saveError.message : "Activity could not be saved"); }
    finally { setSaving(false); }
  };

  const archive = async () => {
    if (!record) return; setSaving(true);
    try { const response = await activitiesService.archiveCatalogItem(record, reason.trim()); if (!response.success) throw new Error(responseMessage(response, "Activity could not be archived")); toast.success("Activity archived"); onSaved(); }
    catch (archiveError) { toast.error(archiveError instanceof Error ? archiveError.message : "Activity could not be archived"); }
    finally { setSaving(false); }
  };

  return <DialogShell onClose={onClose} open={open}><DialogHeader onClose={saving ? undefined : onClose} title={record ? record.name : "Add activity"} />
    {archiveMode && record ? <div className="flex min-h-0 flex-1 flex-col"><DialogBody><div className="space-y-5"><p className="text-sm text-[var(--text-muted)]">Close or cancel current groups before archiving this activity.</p><Field label="Reason"><Textarea data-autofocus="true" maxLength={1000} onChange={(event) => setReason(event.target.value)} required rows={6} value={reason} /></Field></div></DialogBody><DialogFooter><Button onClick={() => setArchiveMode(false)} type="button" variant="secondary">Back</Button><Button disabled={saving || !reason.trim()} onClick={() => void archive()} type="button" variant="destructive">{saving ? "Archiving…" : "Archive activity"}</Button></DialogFooter></div> : <form className="flex min-h-0 flex-1 flex-col" onSubmit={(event) => void save(event)}><DialogBody><div className="space-y-5">{record?.status === "archived" ? <div className="border border-[var(--border)] bg-[var(--surface-sunken)] p-4 text-sm text-[var(--text-muted)]">Archived activities are read-only.</div> : null}<Field label="Code"><Input data-autofocus="true" disabled={record?.status === "archived"} maxLength={24} onChange={(event) => setForm({ ...form, code: event.target.value })} required value={form.code} /></Field><Field label="Name"><Input disabled={record?.status === "archived"} maxLength={160} onChange={(event) => setForm({ ...form, name: event.target.value })} required value={form.name} /></Field><Field label="Category"><Select disabled={record?.status === "archived"} onChange={(event) => setForm({ ...form, category: event.target.value as ActivityCategory })} value={form.category}>{categories.map((value) => <option key={value} value={value}>{displayValue(value)}</option>)}</Select></Field><Field label="Description"><Textarea disabled={record?.status === "archived"} maxLength={4000} onChange={(event) => setForm({ ...form, description: event.target.value || null })} rows={7} value={form.description ?? ""} /></Field></div></DialogBody><DialogFooter>{record?.status === "active" ? <Button className="mr-auto" onClick={() => setArchiveMode(true)} type="button" variant="ghost">Archive</Button> : null}<Button onClick={onClose} type="button" variant="secondary">Close</Button>{record?.status !== "archived" ? <Button disabled={saving || !form.code.trim() || !form.name.trim()} type="submit">{saving ? "Saving…" : record ? "Save changes" : "Add activity"}</Button> : null}</DialogFooter></form>}
  </DialogShell>;
}

function Field({ children, label }: { children: React.ReactNode; label: string }) { return <div className="space-y-2"><Label>{label}</Label>{children}</div>; }

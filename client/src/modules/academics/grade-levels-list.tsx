import { useCallback, useEffect, useState } from "react";
import { Edit, ListOrdered, Loader2, MoreVertical, Plus, Search, Trash2 } from "lucide-react";
import toast from "react-hot-toast";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { ConfirmDrawer } from "@/components/ui/confirm-drawer";
import { Table, TableControlsBar, TableControlsPagination, TableControlsSearch, TableEmpty, TableError, TableLoading, TableScroll, TableWrap, TBody, TD, TH, THead, TR } from "@/components/ui/data-table";
import { DialogBody, DialogFooter, DialogHeader, DialogShell } from "@/components/ui/dialog";
import { Input, Label, Select } from "@/components/ui/input";
import { usePageChrome } from "@/modules/admin/layouts/page-chrome";
import { useAuthStore } from "@/stores/auth-store";

import { academicsService, responseMessage } from "./service";
import type { AcademicGradeLevel, DirectoryStatus } from "./types";

export function AcademicGradeLevelsList() {
  const permissions = useAuthStore((state) => state.user?.permissions ?? []);
  const canCreate = permissions.includes("*") || permissions.includes("academics:create");
  const canEdit = permissions.includes("*") || permissions.includes("academics:edit");
  const canDelete = permissions.includes("*") || permissions.includes("academics:delete");
  const [records, setRecords] = useState<AcademicGradeLevel[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [search, setSearch] = useState("");
  const [submittedSearch, setSubmittedSearch] = useState("");
  const [status, setStatus] = useState("all");
  const [page, setPage] = useState(1);
  const [totalPages, setTotalPages] = useState(1);
  const [drawerRecord, setDrawerRecord] = useState<AcademicGradeLevel | null | undefined>(undefined);
  const [deleteRecord, setDeleteRecord] = useState<AcademicGradeLevel | null>(null);
  const [menuId, setMenuId] = useState<string | null>(null);
  const [deleting, setDeleting] = useState(false);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const response = await academicsService.listGradeLevels({ page, per_page: 20, search: submittedSearch || undefined, status: status === "all" ? undefined : status });
      if (!response.success || !response.data) throw new Error(responseMessage(response, "Grade levels could not be loaded"));
      setRecords(response.data.grade_levels);
      setTotalPages(response.pagination?.total_pages ?? 1);
    } catch (loadError) {
      setError(loadError instanceof Error ? loadError.message : "Grade levels could not be loaded");
    } finally {
      setLoading(false);
    }
  }, [page, status, submittedSearch]);

  useEffect(() => { void load(); }, [load]);

  const remove = async () => {
    if (!deleteRecord || deleting) return;
    setDeleting(true);
    const response = await academicsService.deleteGradeLevel(deleteRecord.id);
    setDeleting(false);
    if (response.success) {
      toast.success("Grade level removed");
      setDeleteRecord(null);
      void load();
    } else toast.error(responseMessage(response, "Grade level could not be removed"));
  };

  usePageChrome("Grade levels", canCreate ? <Button onClick={() => setDrawerRecord(null)}><Plus className="size-4" />Add grade level</Button> : null);
  const filtered = submittedSearch || status !== "all";

  return <div className="space-y-6">
    <p className="text-sm text-[var(--text-muted)]">Maintain the grade references used by admissions, classes, fees, reporting, and imports.</p>
    <TableControlsBar>
      <TableControlsSearch onSubmit={(event) => { event.preventDefault(); setPage(1); setSubmittedSearch(search.trim()); }}>
        <Input aria-label="Search grade levels" leadingIcon={<Search />} onChange={(event) => setSearch(event.target.value)} placeholder="Search grade levels…" value={search} />
        <Button type="submit" variant="secondary">Search</Button>
      </TableControlsSearch>
      <Select aria-label="Status filter" className="sm:w-40" onChange={(event) => { setPage(1); setStatus(event.target.value); }} value={status}><option value="all">All statuses</option><option value="active">Active</option><option value="inactive">Inactive</option></Select>
      {!loading && records.length > 0 ? <TableControlsPagination onNext={() => setPage((value) => Math.min(totalPages, value + 1))} onPrevious={() => setPage((value) => Math.max(1, value - 1))} page={page} totalPages={totalPages} /> : null}
    </TableControlsBar>
    <TableWrap>
      {loading ? <TableLoading columns={5} label="Loading grade levels…" /> : error ? <TableError description={error} onRetry={() => void load()} /> : records.length === 0 ? <TableEmpty description={filtered ? "Change the current filters." : "No grade levels have been created."} icon={<ListOrdered />} title={filtered ? "No grade levels match these filters" : "No grade levels yet"} /> : <TableScroll><Table><THead><tr><TH>Grade level</TH><TH>Code</TH><TH>Order</TH><TH>Status</TH><TH className="text-right">Actions</TH></tr></THead><TBody>{records.map((record) => <TR key={record.id}><TD><span className="font-medium text-[var(--text-strong)]">{record.name}</span></TD><TD className="font-tabular text-[var(--text-muted)]">{record.code}</TD><TD className="font-tabular text-[var(--text-muted)]">{record.sequence_number}</TD><TD><Badge tone={record.status === "active" ? "success" : "neutral"}>{record.status}</Badge></TD><TD className="text-right">{canEdit || canDelete ? <div className="relative inline-flex"><button aria-label="Grade level actions" className="inline-flex size-8 items-center justify-center rounded-[var(--radius-md)] hover:bg-[var(--surface-muted)]" onClick={() => setMenuId(menuId === record.id ? null : record.id)} type="button"><MoreVertical className="size-4" /></button>{menuId === record.id ? <div className="absolute right-0 top-9 z-10 w-40 rounded-[var(--radius-lg)] border border-[var(--border)] bg-[var(--surface)] py-1 shadow-[var(--shadow-popover)]">{canEdit ? <button className="flex w-full items-center gap-2 px-4 py-2 text-sm hover:bg-[var(--surface-muted)]" onClick={() => { setDrawerRecord(record); setMenuId(null); }} type="button"><Edit className="size-4" />Edit</button> : null}{canDelete ? <button className="flex w-full items-center gap-2 px-4 py-2 text-sm text-[var(--tone-danger)] hover:bg-[var(--tone-danger-bg)]" onClick={() => { setDeleteRecord(record); setMenuId(null); }} type="button"><Trash2 className="size-4" />Remove</button> : null}</div> : null}</div> : <span className="text-[var(--text-subtle)]">—</span>}</TD></TR>)}</TBody></Table></TableScroll>}
    </TableWrap>
    <GradeLevelDrawer onClose={() => setDrawerRecord(undefined)} onSaved={() => { setDrawerRecord(undefined); void load(); }} open={(canCreate || canEdit) && drawerRecord !== undefined} record={drawerRecord ?? null} />
    <ConfirmDrawer confirmLabel="Remove grade level" description={`Remove ${deleteRecord?.name ?? "this grade level"}? Classes that use it must be moved first.`} isPending={deleting} onClose={() => setDeleteRecord(null)} onConfirm={() => void remove()} open={canDelete && deleteRecord !== null} title="Remove grade level?" />
  </div>;
}

function GradeLevelDrawer({ onClose, onSaved, open, record }: { onClose: () => void; onSaved: () => void; open: boolean; record: AcademicGradeLevel | null }) {
  const [code, setCode] = useState("");
  const [name, setName] = useState("");
  const [sequenceNumber, setSequenceNumber] = useState("1");
  const [status, setStatus] = useState<DirectoryStatus>("active");
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    if (!open) return;
    setCode(record?.code ?? "");
    setName(record?.name ?? "");
    setSequenceNumber(String(record?.sequence_number ?? 1));
    setStatus(record?.status ?? "active");
  }, [open, record]);

  const submit = async (event: React.FormEvent) => {
    event.preventDefault();
    setSaving(true);
    try {
      const payload = { code: code.trim(), name: name.trim(), sequence_number: Number(sequenceNumber), status };
      const response = record ? await academicsService.updateGradeLevel(record.id, payload) : await academicsService.createGradeLevel(payload);
      if (!response.success) throw new Error(responseMessage(response, "Grade level could not be saved"));
      toast.success("Grade level saved");
      onSaved();
    } catch (saveError) {
      toast.error(saveError instanceof Error ? saveError.message : "Grade level could not be saved");
    } finally {
      setSaving(false);
    }
  };

  return <DialogShell onClose={onClose} open={open}><DialogHeader onClose={onClose} title={`${record ? "Edit" : "Add"} grade level`} /><form onSubmit={submit}><DialogBody className="space-y-5">
    <div><Label>Code</Label><Input className="mt-1.5" data-autofocus="true" maxLength={40} onChange={(event) => setCode(event.target.value)} placeholder="e.g. FORM-1" required value={code} /></div>
    <div><Label>Name</Label><Input className="mt-1.5" maxLength={120} onChange={(event) => setName(event.target.value)} placeholder="e.g. Form 1" required value={name} /></div>
    <div><Label>Order</Label><Input className="mt-1.5" max={999} min={0} onChange={(event) => setSequenceNumber(event.target.value)} required type="number" value={sequenceNumber} /><p className="mt-2 text-xs text-[var(--text-muted)]">Controls the order used in grade lists and reports.</p></div>
    <div><Label>Status</Label><Select className="mt-1.5" onChange={(event) => setStatus(event.target.value as DirectoryStatus)} value={status}><option value="active">Active</option><option value="inactive">Inactive</option></Select></div>
  </DialogBody><DialogFooter><Button disabled={saving} onClick={onClose} type="button" variant="ghost">Cancel</Button><Button disabled={saving} type="submit">{saving ? <><Loader2 className="size-4 animate-spin" />Saving…</> : "Save"}</Button></DialogFooter></form></DialogShell>;
}

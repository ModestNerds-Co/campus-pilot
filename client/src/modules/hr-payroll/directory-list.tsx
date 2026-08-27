import { useCallback, useEffect, useState } from "react";
import { Building2, Edit, Loader2, MoreVertical, Plus, Search, Trash2 } from "lucide-react";
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

import { hrPayrollService } from "./service";
import type { Department, DirectoryStatus, Position } from "./types";

type DirectoryRecord = Department | Position;
type DirectoryKind = "department" | "position";

export function DirectoryList({ kind }: { kind: DirectoryKind }) {
  const isDepartment = kind === "department";
  const label = isDepartment ? "Department" : "Position";
  const [records, setRecords] = useState<DirectoryRecord[]>([]);
  const [departments, setDepartments] = useState<Department[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [search, setSearch] = useState("");
  const [submittedSearch, setSubmittedSearch] = useState("");
  const [status, setStatus] = useState<"all" | DirectoryStatus>("all");
  const [page, setPage] = useState(1);
  const [totalPages, setTotalPages] = useState(1);
  const [drawerRecord, setDrawerRecord] = useState<DirectoryRecord | null | undefined>(undefined);
  const [deleteRecord, setDeleteRecord] = useState<DirectoryRecord | null>(null);
  const [menuId, setMenuId] = useState<string | null>(null);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const params = { page, per_page: 20, search: submittedSearch || undefined, status: status === "all" ? undefined : status };
      if (isDepartment) {
        const response = await hrPayrollService.listDepartments(params);
        if (!response.success || !response.data) throw new Error(response.message || `${label}s could not be loaded`);
        setRecords(response.data.departments);
        setTotalPages(response.pagination?.total_pages ?? 1);
      } else {
        const response = await hrPayrollService.listPositions(params);
        if (!response.success || !response.data) throw new Error(response.message || `${label}s could not be loaded`);
        setRecords(response.data.positions);
        setTotalPages(response.pagination?.total_pages ?? 1);
      }
    } catch (loadError) {
      setError(loadError instanceof Error ? loadError.message : `${label}s could not be loaded`);
    } finally {
      setLoading(false);
    }
  }, [isDepartment, label, page, status, submittedSearch]);

  useEffect(() => { void load(); }, [load]);
  useEffect(() => {
    if (isDepartment) return;
    void hrPayrollService.listDepartments({ per_page: 100, status: "active" }).then((response) => {
      if (response.success && response.data) setDepartments(response.data.departments);
    });
  }, [isDepartment]);

  const remove = async () => {
    if (!deleteRecord) return;
    const response = isDepartment
      ? await hrPayrollService.deleteDepartment(deleteRecord.id)
      : await hrPayrollService.deletePosition(deleteRecord.id);
    if (response.success) {
      toast.success(`${label} removed`);
      setDeleteRecord(null);
      void load();
    } else toast.error(response.issues?.[0]?.toString() || response.message || `${label} could not be removed`);
  };

  usePageChrome(`${label}s`, <Button onClick={() => setDrawerRecord(null)}><Plus className="size-4" />Add {kind}</Button>);

  return (
    <div className="space-y-6">
      <p className="text-sm text-[var(--text-muted)]">{isDepartment ? "Organize employees by their operational department." : "Maintain the job positions employees may hold."}</p>
      <TableControlsBar>
        <TableControlsSearch onSubmit={(event) => { event.preventDefault(); setPage(1); setSubmittedSearch(search.trim()); }}>
          <Input aria-label={`Search ${kind}s`} leadingIcon={<Search />} onChange={(event) => setSearch(event.target.value)} placeholder={`Search ${kind}s…`} value={search} />
          <Button type="submit" variant="secondary">Search</Button>
        </TableControlsSearch>
        <Select aria-label="Status filter" className="sm:w-44" onChange={(event) => { setPage(1); setStatus(event.target.value as typeof status); }} value={status}>
          <option value="all">All statuses</option><option value="active">Active</option><option value="inactive">Inactive</option>
        </Select>
        {!loading && records.length > 0 ? <TableControlsPagination onNext={() => setPage((value) => Math.min(totalPages, value + 1))} onPrevious={() => setPage((value) => Math.max(1, value - 1))} page={page} totalPages={totalPages} /> : null}
      </TableControlsBar>
      <TableWrap>
        {loading ? <TableLoading columns={4} label={`Loading ${kind}s…`} /> : error ? <TableError description={error} onRetry={() => void load()} /> : records.length === 0 ? (
          <TableEmpty description={submittedSearch || status !== "all" ? "Change the current filters." : `Add the first ${kind}.`} icon={<Building2 />} title={submittedSearch || status !== "all" ? `No ${kind}s match these filters` : `No ${kind}s yet`} />
        ) : <TableScroll><Table><THead><tr><TH>{label}</TH><TH>Code</TH>{!isDepartment ? <TH>Department</TH> : null}<TH>Status</TH><TH className="text-right">Actions</TH></tr></THead><TBody>
          {records.map((record) => <TR key={record.id}>
            <TD><span className="font-medium text-[var(--text-strong)]">{"name" in record ? record.name : record.title}</span></TD>
            <TD className="font-tabular text-[var(--text-muted)]">{record.code}</TD>
            {!isDepartment && "department_id" in record ? <TD className="text-[var(--text-muted)]">{departments.find((item) => item.id === record.department_id)?.name || "—"}</TD> : null}
            <TD><Badge tone={record.status === "active" ? "success" : "neutral"}>{record.status}</Badge></TD>
            <TD className="text-right"><div className="relative inline-flex"><button aria-label={`${label} actions`} className="inline-flex size-8 items-center justify-center rounded-[var(--radius-md)] hover:bg-[var(--surface-muted)]" onClick={() => setMenuId(menuId === record.id ? null : record.id)} type="button"><MoreVertical className="size-4" /></button>
              {menuId === record.id ? <div className="absolute right-0 top-9 z-10 w-40 rounded-[var(--radius-lg)] border border-[var(--border)] bg-[var(--surface)] py-1 shadow-[var(--shadow-popover)]"><button className="flex w-full items-center gap-2 px-4 py-2 text-sm hover:bg-[var(--surface-muted)]" onClick={() => { setDrawerRecord(record); setMenuId(null); }}><Edit className="size-4" />Edit</button><button className="flex w-full items-center gap-2 px-4 py-2 text-sm text-[var(--tone-danger)] hover:bg-[var(--tone-danger-bg)]" onClick={() => { setDeleteRecord(record); setMenuId(null); }}><Trash2 className="size-4" />Remove</button></div> : null}
            </div></TD>
          </TR>)}
        </TBody></Table></TableScroll>}
      </TableWrap>
      <DirectoryDrawer departments={departments} kind={kind} onClose={() => setDrawerRecord(undefined)} onSaved={() => { setDrawerRecord(undefined); void load(); }} open={drawerRecord !== undefined} record={drawerRecord ?? null} />
      <ConfirmDrawer confirmLabel={`Remove ${kind}`} description={`Remove ${deleteRecord ? ("name" in deleteRecord ? deleteRecord.name : deleteRecord.title) : `this ${kind}`}? Records using it must be reassigned first.`} onClose={() => setDeleteRecord(null)} onConfirm={() => void remove()} open={deleteRecord !== null} title={`Remove ${kind}?`} />
    </div>
  );
}

function DirectoryDrawer({ departments, kind, onClose, onSaved, open, record }: { departments: Department[]; kind: DirectoryKind; onClose: () => void; onSaved: () => void; open: boolean; record: DirectoryRecord | null }) {
  const isDepartment = kind === "department";
  const [code, setCode] = useState("");
  const [name, setName] = useState("");
  const [departmentId, setDepartmentId] = useState("");
  const [status, setStatus] = useState<DirectoryStatus>("active");
  const [notes, setNotes] = useState("");
  const [saving, setSaving] = useState(false);
  useEffect(() => {
    if (!open) return;
    setCode(record?.code ?? "");
    setName(record ? ("name" in record ? record.name : record.title) : "");
    setDepartmentId(record && "department_id" in record ? record.department_id ?? "" : "");
    setStatus(record?.status ?? "active");
    setNotes(record?.notes ?? "");
  }, [open, record]);
  const submit = async (event: React.FormEvent) => {
    event.preventDefault();
    if (!code.trim() || !name.trim()) return;
    setSaving(true);
    try {
      const response = isDepartment
        ? record ? await hrPayrollService.updateDepartment(record.id, { code: code.trim(), name: name.trim(), status, notes: notes.trim() || null }) : await hrPayrollService.createDepartment({ code: code.trim(), name: name.trim(), status, notes: notes.trim() || null })
        : record ? await hrPayrollService.updatePosition(record.id, { code: code.trim(), title: name.trim(), department_id: departmentId || null, status, notes: notes.trim() || null }) : await hrPayrollService.createPosition({ code: code.trim(), title: name.trim(), department_id: departmentId || null, status, notes: notes.trim() || null });
      if (!response.success) throw new Error(response.message || `${isDepartment ? "Department" : "Position"} could not be saved`);
      toast.success(`${isDepartment ? "Department" : "Position"} saved`); onSaved();
    } catch (error) { toast.error(error instanceof Error ? error.message : "Record could not be saved"); } finally { setSaving(false); }
  };
  return <DialogShell onClose={onClose} open={open}><DialogHeader onClose={onClose} title={`${record ? "Edit" : "Add"} ${kind}`} /><form onSubmit={submit}><DialogBody className="space-y-5">
    <div><Label>Code</Label><Input className="mt-1.5" maxLength={40} onChange={(event) => setCode(event.target.value)} required value={code} /></div>
    <div><Label>{isDepartment ? "Name" : "Title"}</Label><Input className="mt-1.5" maxLength={160} onChange={(event) => setName(event.target.value)} required value={name} /></div>
    {!isDepartment ? <div><Label>Department</Label><Select className="mt-1.5" onChange={(event) => setDepartmentId(event.target.value)} value={departmentId}><option value="">No department</option>{departments.map((department) => <option key={department.id} value={department.id}>{department.name}</option>)}</Select></div> : null}
    <div><Label>Status</Label><Select className="mt-1.5" onChange={(event) => setStatus(event.target.value as DirectoryStatus)} value={status}><option value="active">Active</option><option value="inactive">Inactive</option></Select></div>
    <div><Label>Notes</Label><Textarea className="mt-1.5" maxLength={2000} onChange={(event) => setNotes(event.target.value)} value={notes} /></div>
  </DialogBody><DialogFooter><Button disabled={saving} onClick={onClose} type="button" variant="ghost">Cancel</Button><Button disabled={saving} type="submit">{saving ? <><Loader2 className="size-4 animate-spin" />Saving…</> : "Save"}</Button></DialogFooter></form></DialogShell>;
}

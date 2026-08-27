// Employee-backed teacher profiles for Academics.

import { useCallback, useEffect, useMemo, useState } from "react";
import { Edit, Loader2, MoreVertical, Plus, Search, Trash2, UserRoundCheck } from "lucide-react";
import toast from "react-hot-toast";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { ConfirmDrawer } from "@/components/ui/confirm-drawer";
import {
  Table, TableControlsBar, TableControlsPagination, TableControlsSearch, TableEmpty,
  TableError, TableLoading, TableScroll, TableWrap, TBody, TD, TH, THead, TR,
} from "@/components/ui/data-table";
import { DialogBody, DialogFooter, DialogHeader, DialogShell } from "@/components/ui/dialog";
import { Input, Label, Select } from "@/components/ui/input";
import { usePageChrome } from "@/modules/admin/layouts/page-chrome";

import { academicsService, responseMessage } from "./service";
import type { DirectoryStatus, EmployeeCandidate, TeacherProfile } from "./types";

export function TeachersList() {
  const [teachers, setTeachers] = useState<TeacherProfile[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [search, setSearch] = useState("");
  const [submittedSearch, setSubmittedSearch] = useState("");
  const [status, setStatus] = useState<"all" | DirectoryStatus>("all");
  const [page, setPage] = useState(1);
  const [totalPages, setTotalPages] = useState(1);
  const [drawerTeacher, setDrawerTeacher] = useState<TeacherProfile | null | undefined>(undefined);
  const [deleteTeacher, setDeleteTeacher] = useState<TeacherProfile | null>(null);
  const [menuId, setMenuId] = useState<string | null>(null);
  const [deleting, setDeleting] = useState(false);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const response = await academicsService.listTeachers({ page, per_page: 20, search: submittedSearch || undefined, status: status === "all" ? undefined : status });
      if (!response.success || !response.data) throw new Error(responseMessage(response, "Teachers could not be loaded"));
      setTeachers(response.data.teachers);
      setTotalPages(response.pagination?.total_pages ?? 1);
    } catch (loadError) {
      setError(loadError instanceof Error ? loadError.message : "Teachers could not be loaded");
    } finally {
      setLoading(false);
    }
  }, [page, status, submittedSearch]);

  useEffect(() => { void load(); }, [load]);
  const remove = async () => {
    if (!deleteTeacher || deleting) return;
    setDeleting(true);
    const response = await academicsService.deleteTeacher(deleteTeacher.id);
    setDeleting(false);
    if (response.success) {
      toast.success("Teacher profile removed");
      setDeleteTeacher(null);
      void load();
    } else toast.error(responseMessage(response, "Teacher profile could not be removed"));
  };

  usePageChrome("Teachers", <Button onClick={() => setDrawerTeacher(null)}><Plus className="size-4" />Add teacher</Button>);
  const filtered = submittedSearch || status !== "all";

  return <div className="space-y-6">
    <p className="text-sm text-[var(--text-muted)]">A teacher is an Academics profile attached to an existing HR employee.</p>
    <TableControlsBar><TableControlsSearch onSubmit={(event) => { event.preventDefault(); setPage(1); setSubmittedSearch(search.trim()); }}><Input aria-label="Search teachers" leadingIcon={<Search />} onChange={(event) => setSearch(event.target.value)} placeholder="Search teacher, employee number, or email…" value={search} /><Button type="submit" variant="secondary">Search</Button></TableControlsSearch><Select aria-label="Status filter" className="sm:w-40" onChange={(event) => { setPage(1); setStatus(event.target.value as typeof status); }} value={status}><option value="all">All statuses</option><option value="active">Active</option><option value="inactive">Inactive</option></Select>{!loading && teachers.length > 0 ? <TableControlsPagination onNext={() => setPage((value) => Math.min(totalPages, value + 1))} onPrevious={() => setPage((value) => Math.max(1, value - 1))} page={page} totalPages={totalPages} /> : null}</TableControlsBar>
    <TableWrap>{loading ? <TableLoading columns={5} label="Loading teachers…" /> : error ? <TableError description={error} onRetry={() => void load()} /> : teachers.length === 0 ? <TableEmpty description={filtered ? "Change the current filters." : "Add a teacher from the employee directory."} icon={<UserRoundCheck />} title={filtered ? "No teachers match these filters" : "No teachers yet"} /> : <TableScroll><Table><THead><tr><TH>Teacher</TH><TH>Employee</TH><TH>Contact</TH><TH>Status</TH><TH className="text-right">Actions</TH></tr></THead><TBody>{teachers.map((teacher) => <TR key={teacher.id}><TD><div className="font-medium text-[var(--text-strong)]">{teacher.display_name}</div><div className="text-xs text-[var(--text-muted)]">Academics profile</div></TD><TD><div className="font-tabular text-[var(--text-strong)]">{teacher.employee_number}</div><div className="text-xs text-[var(--text-muted)]">{teacher.employment_status}</div></TD><TD className="text-[var(--text-muted)]">{teacher.work_email || teacher.phone || "—"}</TD><TD><Badge tone={teacher.status === "active" ? "success" : "neutral"}>{teacher.status}</Badge></TD><TD className="text-right"><div className="relative inline-flex"><button aria-label="Teacher actions" className="inline-flex size-8 items-center justify-center rounded-[var(--radius-md)] hover:bg-[var(--surface-muted)]" onClick={() => setMenuId(menuId === teacher.id ? null : teacher.id)} type="button"><MoreVertical className="size-4" /></button>{menuId === teacher.id ? <div className="absolute right-0 top-9 z-10 w-48 rounded-[var(--radius-lg)] border border-[var(--border)] bg-[var(--surface)] py-1 shadow-[var(--shadow-popover)]"><button className="flex w-full items-center gap-2 px-4 py-2 text-sm hover:bg-[var(--surface-muted)]" onClick={() => { setDrawerTeacher(teacher); setMenuId(null); }} type="button"><Edit className="size-4" />Change status</button><button className="flex w-full items-center gap-2 px-4 py-2 text-sm text-[var(--tone-danger)] hover:bg-[var(--tone-danger-bg)]" onClick={() => { setDeleteTeacher(teacher); setMenuId(null); }} type="button"><Trash2 className="size-4" />Remove profile</button></div> : null}</div></TD></TR>)}</TBody></Table></TableScroll>}</TableWrap>
    <TeacherDrawer onClose={() => setDrawerTeacher(undefined)} onSaved={() => { setDrawerTeacher(undefined); void load(); }} open={drawerTeacher !== undefined} teacher={drawerTeacher ?? null} />
    <ConfirmDrawer confirmLabel="Remove teacher profile" description={`Remove ${deleteTeacher?.display_name || "this teacher"} from Academics? Their HR employee record will not be removed. Teaching assignments must be removed first.`} isPending={deleting} onClose={() => setDeleteTeacher(null)} onConfirm={() => void remove()} open={deleteTeacher !== null} title="Remove teacher profile?" />
  </div>;
}

function TeacherDrawer({ onClose, onSaved, open, teacher }: { onClose: () => void; onSaved: () => void; open: boolean; teacher: TeacherProfile | null }) {
  const [candidates, setCandidates] = useState<EmployeeCandidate[]>([]);
  const [search, setSearch] = useState("");
  const [employeeId, setEmployeeId] = useState("");
  const [status, setStatus] = useState<DirectoryStatus>("active");
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    if (!open) return;
    setSearch("");
    setEmployeeId(teacher?.employee_id ?? "");
    setStatus(teacher?.status ?? "active");
    if (teacher) return;
    setLoading(true);
    void academicsService.listTeacherCandidates().then((response) => {
      if (response.success && response.data) setCandidates(response.data.employees);
    }).finally(() => setLoading(false));
  }, [open, teacher]);

  const filtered = useMemo(() => {
    const query = search.trim().toLowerCase();
    if (!query) return candidates;
    return candidates.filter((employee) => `${employee.display_name} ${employee.employee_number} ${employee.work_email ?? ""}`.toLowerCase().includes(query));
  }, [candidates, search]);

  const submit = async (event: React.FormEvent) => {
    event.preventDefault();
    if (!teacher && !employeeId) return toast.error("Choose an employee");
    setSaving(true);
    const response = teacher ? await academicsService.updateTeacher(teacher.id, status) : await academicsService.createTeacher(employeeId);
    setSaving(false);
    if (!response.success) return toast.error(responseMessage(response, "Teacher profile could not be saved"));
    toast.success("Teacher profile saved");
    onSaved();
  };

  return <DialogShell onClose={onClose} open={open}><DialogHeader onClose={onClose} title={teacher ? "Teacher status" : "Add teacher"} /><form onSubmit={submit}><DialogBody className="space-y-5">
    {teacher ? <div className="rounded-[var(--radius-lg)] bg-[var(--surface-muted)] p-4"><p className="font-medium text-[var(--text-strong)]">{teacher.display_name}</p><p className="mt-1 font-tabular text-xs text-[var(--text-muted)]">{teacher.employee_number}</p></div> : <><div><Label htmlFor="teacher-search">Find an employee</Label><Input className="mt-1.5" id="teacher-search" leadingIcon={<Search />} onChange={(event) => setSearch(event.target.value)} placeholder="Search employee directory…" value={search} /></div><div className="max-h-[50dvh] divide-y divide-[var(--border-subtle)] overflow-y-auto rounded-[var(--radius-lg)] border border-[var(--border)]">{loading ? <p className="p-4 text-sm text-[var(--text-muted)]">Loading employees…</p> : filtered.length === 0 ? <p className="p-4 text-sm text-[var(--text-muted)]">No eligible employees found.</p> : filtered.map((employee) => <label className={`flex cursor-pointer items-start gap-3 p-4 hover:bg-[var(--surface-muted)] ${employeeId === employee.id ? "bg-[var(--brand-soft)]" : ""}`} key={employee.id}><input checked={employeeId === employee.id} className="mt-1" name="employee" onChange={() => setEmployeeId(employee.id)} type="radio" value={employee.id} /><span><span className="block text-sm font-medium text-[var(--text-strong)]">{employee.display_name}</span><span className="mt-0.5 block text-xs text-[var(--text-muted)]">{employee.employee_number}{employee.work_email ? ` · ${employee.work_email}` : ""}</span></span></label>)}</div></>}
    {teacher ? <div><Label>Status</Label><Select className="mt-1.5" onChange={(event) => setStatus(event.target.value as DirectoryStatus)} value={status}><option value="active">Active</option><option value="inactive">Inactive</option></Select></div> : null}
  </DialogBody><DialogFooter><Button disabled={saving} onClick={onClose} type="button" variant="ghost">Cancel</Button><Button disabled={saving || (!teacher && !employeeId)} type="submit">{saving ? <><Loader2 className="size-4 animate-spin" />Saving…</> : "Save teacher"}</Button></DialogFooter></form></DialogShell>;
}

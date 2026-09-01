// Canonical class, subject, and employee-backed teacher assignments.

import { useCallback, useEffect, useMemo, useState } from "react";
import { ClipboardList, Edit, Loader2, MoreVertical, Plus, Trash2 } from "lucide-react";
import toast from "react-hot-toast";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { ConfirmDrawer } from "@/components/ui/confirm-drawer";
import {
  Table, TableControlsBar, TableControlsPagination, TableEmpty, TableError, TableLoading,
  TableScroll, TableWrap, TBody, TD, TH, THead, TR,
} from "@/components/ui/data-table";
import { DialogBody, DialogFooter, DialogHeader, DialogShell } from "@/components/ui/dialog";
import { Input, Label, Select } from "@/components/ui/input";
import { usePageChrome } from "@/modules/admin/layouts/page-chrome";
import { useAuthStore } from "@/stores/auth-store";

import { academicsService, responseMessage } from "./service";
import type { AcademicYear, ClassGroup, DirectoryStatus, Subject, TeacherProfile, TeachingAssignment, TeachingAssignmentInput } from "./types";

export function TeachingAssignmentsList() {
  const permissions = useAuthStore((state) => state.user?.permissions ?? []);
  const canCreate = permissions.includes("*") || permissions.includes("academics:create");
  const canEdit = permissions.includes("*") || permissions.includes("academics:edit");
  const canDelete = permissions.includes("*") || permissions.includes("academics:delete");
  const [assignments, setAssignments] = useState<TeachingAssignment[]>([]);
  const [years, setYears] = useState<AcademicYear[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [status, setStatus] = useState<"all" | DirectoryStatus>("all");
  const [yearFilter, setYearFilter] = useState("all");
  const [page, setPage] = useState(1);
  const [totalPages, setTotalPages] = useState(1);
  const [drawerAssignment, setDrawerAssignment] = useState<TeachingAssignment | null | undefined>(undefined);
  const [deleteAssignment, setDeleteAssignment] = useState<TeachingAssignment | null>(null);
  const [menuId, setMenuId] = useState<string | null>(null);
  const [deleting, setDeleting] = useState(false);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const response = await academicsService.listTeachingAssignments({ page, per_page: 20, status: status === "all" ? undefined : status, academic_year_id: yearFilter === "all" ? undefined : yearFilter });
      if (!response.success || !response.data) throw new Error(responseMessage(response, "Teaching assignments could not be loaded"));
      setAssignments(response.data.assignments);
      setTotalPages(response.pagination?.total_pages ?? 1);
    } catch (loadError) {
      setError(loadError instanceof Error ? loadError.message : "Teaching assignments could not be loaded");
    } finally {
      setLoading(false);
    }
  }, [page, status, yearFilter]);

  useEffect(() => { void load(); }, [load]);
  useEffect(() => {
    void academicsService.listAcademicYears({ per_page: 100 }).then((response) => {
      if (response.success && response.data) setYears(response.data.academic_years);
    });
  }, []);

  const remove = async () => {
    if (!deleteAssignment || deleting) return;
    setDeleting(true);
    const response = await academicsService.deleteTeachingAssignment(deleteAssignment.id);
    setDeleting(false);
    if (response.success) {
      toast.success("Teaching assignment removed");
      setDeleteAssignment(null);
      void load();
    } else toast.error(responseMessage(response, "Teaching assignment could not be removed"));
  };

  usePageChrome("Teaching assignments", canCreate ? <Button onClick={() => setDrawerAssignment(null)}><Plus className="size-4" />Add assignment</Button> : null);
  const filtered = status !== "all" || yearFilter !== "all";

  return <div className="space-y-6">
    <p className="text-sm text-[var(--text-muted)]">Assign an HR-backed teacher to a subject and class. Timetabling uses these records directly.</p>
    <TableControlsBar><Select aria-label="Academic year" className="sm:w-56" onChange={(event) => { setPage(1); setYearFilter(event.target.value); }} value={yearFilter}><option value="all">All academic years</option>{years.map((year) => <option key={year.id} value={year.id}>{year.name}</option>)}</Select><Select aria-label="Status filter" className="sm:w-40" onChange={(event) => { setPage(1); setStatus(event.target.value as typeof status); }} value={status}><option value="all">All statuses</option><option value="active">Active</option><option value="inactive">Inactive</option></Select>{!loading && assignments.length > 0 ? <TableControlsPagination onNext={() => setPage((value) => Math.min(totalPages, value + 1))} onPrevious={() => setPage((value) => Math.max(1, value - 1))} page={page} totalPages={totalPages} /> : null}</TableControlsBar>
    <TableWrap>{loading ? <TableLoading columns={6} label="Loading teaching assignments…" /> : error ? <TableError description={error} onRetry={() => void load()} /> : assignments.length === 0 ? <TableEmpty description={filtered ? "Change the current filters." : "No teaching assignments have been created."} icon={<ClipboardList />} title={filtered ? "No assignments match these filters" : "No teaching assignments yet"} /> : <TableScroll><Table><THead><tr><TH>Class</TH><TH>Subject</TH><TH>Teacher</TH><TH>Academic year</TH><TH>Teaching load</TH><TH>Status</TH><TH className="text-right">Actions</TH></tr></THead><TBody>{assignments.map((assignment) => <TR key={assignment.id}><TD className="font-medium text-[var(--text-strong)]">{assignment.class_group_name}</TD><TD className="text-[var(--text-strong)]">{assignment.subject_name}</TD><TD><div className="text-[var(--text-strong)]">{assignment.teacher_name}</div><div className="text-xs text-[var(--text-muted)]">Employee-backed</div></TD><TD className="text-[var(--text-muted)]">{assignment.academic_year_name}</TD><TD className="font-tabular text-[var(--text-muted)]">{assignment.periods_per_cycle} periods</TD><TD><Badge tone={assignment.status === "active" ? "success" : "neutral"}>{assignment.status}</Badge></TD><TD className="text-right">{canEdit || canDelete ? <div className="relative inline-flex"><button aria-label="Assignment actions" className="inline-flex size-8 items-center justify-center rounded-[var(--radius-md)] hover:bg-[var(--surface-muted)]" onClick={() => setMenuId(menuId === assignment.id ? null : assignment.id)} type="button"><MoreVertical className="size-4" /></button>{menuId === assignment.id ? <div className="absolute right-0 top-9 z-10 w-40 rounded-[var(--radius-lg)] border border-[var(--border)] bg-[var(--surface)] py-1 shadow-[var(--shadow-popover)]">{canEdit ? <button className="flex w-full items-center gap-2 px-4 py-2 text-sm hover:bg-[var(--surface-muted)]" onClick={() => { setDrawerAssignment(assignment); setMenuId(null); }} type="button"><Edit className="size-4" />Edit</button> : null}{canDelete ? <button className="flex w-full items-center gap-2 px-4 py-2 text-sm text-[var(--tone-danger)] hover:bg-[var(--tone-danger-bg)]" onClick={() => { setDeleteAssignment(assignment); setMenuId(null); }} type="button"><Trash2 className="size-4" />Remove</button> : null}</div> : null}</div> : <span className="text-[var(--text-subtle)]">—</span>}</TD></TR>)}</TBody></Table></TableScroll>}</TableWrap>
    <AssignmentDrawer assignment={drawerAssignment ?? null} onClose={() => setDrawerAssignment(undefined)} onSaved={() => { setDrawerAssignment(undefined); void load(); }} open={(canCreate || canEdit) && drawerAssignment !== undefined} years={years} />
    <ConfirmDrawer confirmLabel="Remove assignment" description={`Remove ${deleteAssignment ? `${deleteAssignment.teacher_name}'s ${deleteAssignment.subject_name} assignment for ${deleteAssignment.class_group_name}` : "this teaching assignment"}?`} isPending={deleting} onClose={() => setDeleteAssignment(null)} onConfirm={() => void remove()} open={canDelete && deleteAssignment !== null} title="Remove teaching assignment?" />
  </div>;
}

function AssignmentDrawer({ assignment, onClose, onSaved, open, years }: { assignment: TeachingAssignment | null; onClose: () => void; onSaved: () => void; open: boolean; years: AcademicYear[] }) {
  const [classes, setClasses] = useState<ClassGroup[]>([]);
  const [subjects, setSubjects] = useState<Subject[]>([]);
  const [teachers, setTeachers] = useState<TeacherProfile[]>([]);
  const [form, setForm] = useState<TeachingAssignmentInput>({ academic_year_id: "", class_group_id: "", subject_id: "", teacher_profile_id: "", periods_per_cycle: 1, status: "active" });
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    if (!open) return;
    const yearId = assignment?.academic_year_id ?? years.find((year) => year.status === "active")?.id ?? years[0]?.id ?? "";
    setForm(assignment ? { academic_year_id: assignment.academic_year_id, class_group_id: assignment.class_group_id, subject_id: assignment.subject_id, teacher_profile_id: assignment.teacher_profile_id, periods_per_cycle: assignment.periods_per_cycle, status: assignment.status } : { academic_year_id: yearId, class_group_id: "", subject_id: "", teacher_profile_id: "", periods_per_cycle: 1, status: "active" });
    setLoading(true);
    void Promise.all([
      academicsService.listClasses({ per_page: 100 }),
      academicsService.listSubjects({ per_page: 100, status: "active" }),
      academicsService.listTeachers({ per_page: 100, status: "active" }),
    ]).then(([classResponse, subjectResponse, teacherResponse]) => {
      if (classResponse.success && classResponse.data) setClasses(classResponse.data.classes);
      if (subjectResponse.success && subjectResponse.data) setSubjects(subjectResponse.data.subjects);
      if (teacherResponse.success && teacherResponse.data) setTeachers(teacherResponse.data.teachers);
    }).finally(() => setLoading(false));
  }, [assignment, open, years]);

  const availableClasses = useMemo(() => classes.filter((classGroup) => classGroup.academic_year_id === form.academic_year_id && classGroup.status === "active"), [classes, form.academic_year_id]);
  const field = <K extends keyof TeachingAssignmentInput>(key: K, value: TeachingAssignmentInput[K]) => setForm((current) => ({ ...current, [key]: value }));
  const submit = async (event: React.FormEvent) => {
    event.preventDefault();
    setSaving(true);
    const response = assignment ? await academicsService.updateTeachingAssignment(assignment.id, form) : await academicsService.createTeachingAssignment(form);
    setSaving(false);
    if (!response.success) return toast.error(responseMessage(response, "Teaching assignment could not be saved"));
    toast.success("Teaching assignment saved");
    onSaved();
  };

  return <DialogShell onClose={onClose} open={open}><DialogHeader onClose={onClose} title={assignment ? "Edit teaching assignment" : "Add teaching assignment"} /><form onSubmit={submit}><DialogBody className="space-y-5">
    {loading ? <p className="text-sm text-[var(--text-muted)]">Loading academic records…</p> : null}
    <div><Label>Academic year</Label><Select className="mt-1.5" onChange={(event) => { field("academic_year_id", event.target.value); field("class_group_id", ""); }} required value={form.academic_year_id}><option value="">Choose an academic year</option>{years.map((year) => <option key={year.id} value={year.id}>{year.name} · {year.status}</option>)}</Select></div>
    <div><Label>Class</Label><Select className="mt-1.5" disabled={!form.academic_year_id} onChange={(event) => field("class_group_id", event.target.value)} required value={form.class_group_id}><option value="">Choose a class</option>{availableClasses.map((classGroup) => <option key={classGroup.id} value={classGroup.id}>{classGroup.name}</option>)}</Select></div>
    <div><Label>Subject</Label><Select className="mt-1.5" onChange={(event) => field("subject_id", event.target.value)} required value={form.subject_id}><option value="">Choose a subject</option>{subjects.map((subject) => <option key={subject.id} value={subject.id}>{subject.name} · {subject.code}</option>)}</Select></div>
    <div><Label>Teacher</Label><Select className="mt-1.5" onChange={(event) => field("teacher_profile_id", event.target.value)} required value={form.teacher_profile_id}><option value="">Choose a teacher</option>{teachers.map((teacher) => <option key={teacher.id} value={teacher.id}>{teacher.display_name} · {teacher.employee_number}</option>)}</Select></div>
    <div className="grid gap-4 sm:grid-cols-2"><div><Label>Periods per cycle</Label><Input className="mt-1.5" max={40} min={1} onChange={(event) => field("periods_per_cycle", Number(event.target.value))} required type="number" value={form.periods_per_cycle} /></div><div><Label>Status</Label><Select className="mt-1.5" onChange={(event) => field("status", event.target.value as DirectoryStatus)} value={form.status}><option value="active">Active</option><option value="inactive">Inactive</option></Select></div></div>
  </DialogBody><DialogFooter><Button disabled={saving} onClick={onClose} type="button" variant="ghost">Cancel</Button><Button disabled={saving || loading} type="submit">{saving ? <><Loader2 className="size-4 animate-spin" />Saving…</> : "Save assignment"}</Button></DialogFooter></form></DialogShell>;
}

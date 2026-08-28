// Academics-owned academic years, subjects, and classes.

import { useCallback, useEffect, useState } from "react";
import { BookOpen, CalendarRange, Edit, GraduationCap, Loader2, MoreVertical, Plus, Search, Trash2 } from "lucide-react";
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
import type { AcademicGradeLevel, AcademicYear, AcademicYearStatus, ClassGroup, DirectoryStatus, Subject } from "./types";

type DirectoryKind = "academic-year" | "subject" | "class";
type DirectoryRecord = AcademicYear | Subject | ClassGroup;

const labels = {
  "academic-year": { plural: "Academic years", singular: "academic year" },
  subject: { plural: "Subjects", singular: "subject" },
  class: { plural: "Classes", singular: "class" },
} as const;

export function AcademicDirectoryList({ kind }: { kind: DirectoryKind }) {
  const label = labels[kind];
  const [records, setRecords] = useState<DirectoryRecord[]>([]);
  const [years, setYears] = useState<AcademicYear[]>([]);
  const [gradeLevels, setGradeLevels] = useState<AcademicGradeLevel[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [search, setSearch] = useState("");
  const [submittedSearch, setSubmittedSearch] = useState("");
  const [status, setStatus] = useState("all");
  const [yearFilter, setYearFilter] = useState("all");
  const [gradeFilter, setGradeFilter] = useState("all");
  const [page, setPage] = useState(1);
  const [totalPages, setTotalPages] = useState(1);
  const [drawerRecord, setDrawerRecord] = useState<DirectoryRecord | null | undefined>(undefined);
  const [deleteRecord, setDeleteRecord] = useState<DirectoryRecord | null>(null);
  const [menuId, setMenuId] = useState<string | null>(null);
  const [deleting, setDeleting] = useState(false);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const params = {
        page,
        per_page: 20,
        search: submittedSearch || undefined,
        status: status === "all" ? undefined : status,
        academic_year_id: kind === "class" && yearFilter !== "all" ? yearFilter : undefined,
        grade_level_id: kind === "class" && gradeFilter !== "all" ? gradeFilter : undefined,
      };
      const response = kind === "academic-year"
        ? await academicsService.listAcademicYears(params)
        : kind === "subject"
          ? await academicsService.listSubjects(params)
          : await academicsService.listClasses(params);
      if (!response.success || !response.data) throw new Error(responseMessage(response, `${label.plural} could not be loaded`));
      const next = "academic_years" in response.data
        ? response.data.academic_years
        : "subjects" in response.data
          ? response.data.subjects
          : response.data.classes;
      setRecords(next);
      setTotalPages(response.pagination?.total_pages ?? 1);
    } catch (loadError) {
      setError(loadError instanceof Error ? loadError.message : `${label.plural} could not be loaded`);
    } finally {
      setLoading(false);
    }
  }, [gradeFilter, kind, label.plural, page, status, submittedSearch, yearFilter]);

  useEffect(() => { void load(); }, [load]);
  useEffect(() => {
    if (kind !== "class") return;
    void Promise.all([
      academicsService.listAcademicYears({ per_page: 100 }),
      academicsService.listGradeLevels({ per_page: 100 }),
    ]).then(([yearResponse, gradeResponse]) => {
      if (yearResponse.success && yearResponse.data) setYears(yearResponse.data.academic_years);
      if (gradeResponse.success && gradeResponse.data) setGradeLevels(gradeResponse.data.grade_levels);
    });
  }, [kind]);

  const remove = async () => {
    if (!deleteRecord || deleting) return;
    setDeleting(true);
    const response = kind === "academic-year"
      ? await academicsService.deleteAcademicYear(deleteRecord.id)
      : kind === "subject"
        ? await academicsService.deleteSubject(deleteRecord.id)
        : await academicsService.deleteClass(deleteRecord.id);
    setDeleting(false);
    if (response.success) {
      toast.success(`${capitalise(label.singular)} removed`);
      setDeleteRecord(null);
      void load();
    } else toast.error(responseMessage(response, `${capitalise(label.singular)} could not be removed`));
  };

  usePageChrome(label.plural, <Button onClick={() => setDrawerRecord(null)}><Plus className="size-4" />Add {label.singular}</Button>);
  const filtered = submittedSearch || status !== "all" || (kind === "class" && (yearFilter !== "all" || gradeFilter !== "all"));
  const EmptyIcon = kind === "academic-year" ? CalendarRange : kind === "subject" ? BookOpen : GraduationCap;

  return (
    <div className="space-y-6">
      <p className="text-sm text-[var(--text-muted)]">{descriptionFor(kind)}</p>
      <TableControlsBar>
        <TableControlsSearch onSubmit={(event) => { event.preventDefault(); setPage(1); setSubmittedSearch(search.trim()); }}>
          <Input aria-label={`Search ${label.plural.toLowerCase()}`} leadingIcon={<Search />} onChange={(event) => setSearch(event.target.value)} placeholder={`Search ${label.plural.toLowerCase()}…`} value={search} />
          <Button type="submit" variant="secondary">Search</Button>
        </TableControlsSearch>
        {kind === "class" ? <Select aria-label="Academic year" className="sm:w-48" onChange={(event) => { setPage(1); setYearFilter(event.target.value); }} value={yearFilter}><option value="all">All academic years</option>{years.map((year) => <option key={year.id} value={year.id}>{year.name}</option>)}</Select> : null}
        {kind === "class" ? <Select aria-label="Grade level" className="sm:w-44" onChange={(event) => { setPage(1); setGradeFilter(event.target.value); }} value={gradeFilter}><option value="all">All grade levels</option>{gradeLevels.map((grade) => <option key={grade.id} value={grade.id}>{grade.name}</option>)}</Select> : null}
        <Select aria-label="Status filter" className="sm:w-40" onChange={(event) => { setPage(1); setStatus(event.target.value); }} value={status}>
          <option value="all">All statuses</option>
          {kind === "academic-year" ? <><option value="planned">Planned</option><option value="active">Active</option><option value="closed">Closed</option></> : <><option value="active">Active</option><option value="inactive">Inactive</option></>}
        </Select>
        {!loading && records.length > 0 ? <TableControlsPagination onNext={() => setPage((value) => Math.min(totalPages, value + 1))} onPrevious={() => setPage((value) => Math.max(1, value - 1))} page={page} totalPages={totalPages} /> : null}
      </TableControlsBar>

      <TableWrap>
        {loading ? <TableLoading columns={5} label={`Loading ${label.plural.toLowerCase()}…`} /> : error ? <TableError description={error} onRetry={() => void load()} /> : records.length === 0 ? (
          <TableEmpty description={filtered ? "Change the current filters." : `Add the first ${label.singular}.`} icon={<EmptyIcon />} title={filtered ? `No ${label.plural.toLowerCase()} match these filters` : `No ${label.plural.toLowerCase()} yet`} />
        ) : <TableScroll><Table><THead><tr>{headersFor(kind).map((header) => <TH key={header}>{header}</TH>)}<TH>Status</TH><TH className="text-right">Actions</TH></tr></THead><TBody>
          {records.map((record) => <TR key={record.id}>{cellsFor(kind, record)}<TD><Badge tone={record.status === "active" ? "success" : record.status === "planned" ? "warning" : "neutral"}>{record.status}</Badge></TD><TD className="text-right"><div className="relative inline-flex"><button aria-label={`${capitalise(label.singular)} actions`} className="inline-flex size-8 items-center justify-center rounded-[var(--radius-md)] hover:bg-[var(--surface-muted)]" onClick={() => setMenuId(menuId === record.id ? null : record.id)} type="button"><MoreVertical className="size-4" /></button>{menuId === record.id ? <div className="absolute right-0 top-9 z-10 w-40 rounded-[var(--radius-lg)] border border-[var(--border)] bg-[var(--surface)] py-1 shadow-[var(--shadow-popover)]"><button className="flex w-full items-center gap-2 px-4 py-2 text-sm hover:bg-[var(--surface-muted)]" onClick={() => { setDrawerRecord(record); setMenuId(null); }} type="button"><Edit className="size-4" />Edit</button><button className="flex w-full items-center gap-2 px-4 py-2 text-sm text-[var(--tone-danger)] hover:bg-[var(--tone-danger-bg)]" onClick={() => { setDeleteRecord(record); setMenuId(null); }} type="button"><Trash2 className="size-4" />Remove</button></div> : null}</div></TD></TR>)}
        </TBody></Table></TableScroll>}
      </TableWrap>
      <DirectoryDrawer gradeLevels={gradeLevels} kind={kind} onClose={() => setDrawerRecord(undefined)} onSaved={() => { setDrawerRecord(undefined); void load(); }} open={drawerRecord !== undefined} record={drawerRecord ?? null} years={years} />
      <ConfirmDrawer confirmLabel={`Remove ${label.singular}`} description={`Remove ${recordName(deleteRecord, kind)}? Teaching records that use it must be removed first.`} isPending={deleting} onClose={() => setDeleteRecord(null)} onConfirm={() => void remove()} open={deleteRecord !== null} title={`Remove ${label.singular}?`} />
    </div>
  );
}

function DirectoryDrawer({ gradeLevels, kind, onClose, onSaved, open, record, years }: { gradeLevels: AcademicGradeLevel[]; kind: DirectoryKind; onClose: () => void; onSaved: () => void; open: boolean; record: DirectoryRecord | null; years: AcademicYear[] }) {
  const label = labels[kind];
  const [name, setName] = useState("");
  const [code, setCode] = useState("");
  const [yearId, setYearId] = useState("");
  const [gradeLevelId, setGradeLevelId] = useState("");
  const [startsOn, setStartsOn] = useState("");
  const [endsOn, setEndsOn] = useState("");
  const [status, setStatus] = useState<string>(kind === "academic-year" ? "planned" : "active");
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    if (!open) return;
    setName(record?.name ?? "");
    setCode(record && "code" in record ? record.code : "");
    setYearId(record && "academic_year_id" in record ? record.academic_year_id : years.find((year) => year.status === "active")?.id ?? years[0]?.id ?? "");
    setGradeLevelId(record && "grade_level_id" in record ? record.grade_level_id ?? "" : "");
    setStartsOn(record && "starts_on" in record ? record.starts_on : "");
    setEndsOn(record && "ends_on" in record ? record.ends_on : "");
    setStatus(record?.status ?? (kind === "academic-year" ? "planned" : "active"));
  }, [kind, open, record, years]);

  const submit = async (event: React.FormEvent) => {
    event.preventDefault();
    setSaving(true);
    try {
      const response = kind === "academic-year"
        ? record ? await academicsService.updateAcademicYear(record.id, { name: name.trim(), starts_on: startsOn, ends_on: endsOn, status: status as AcademicYearStatus }) : await academicsService.createAcademicYear({ name: name.trim(), starts_on: startsOn, ends_on: endsOn, status: status as AcademicYearStatus })
        : kind === "subject"
          ? record ? await academicsService.updateSubject(record.id, { code: code.trim(), name: name.trim(), status: status as DirectoryStatus }) : await academicsService.createSubject({ code: code.trim(), name: name.trim(), status: status as DirectoryStatus })
          : record ? await academicsService.updateClass(record.id, { academic_year_id: yearId, code: code.trim(), name: name.trim(), grade_level_id: gradeLevelId || null, status: status as DirectoryStatus }) : await academicsService.createClass({ academic_year_id: yearId, code: code.trim(), name: name.trim(), grade_level_id: gradeLevelId || null, status: status as DirectoryStatus });
      if (!response.success) throw new Error(responseMessage(response, `${capitalise(label.singular)} could not be saved`));
      toast.success(`${capitalise(label.singular)} saved`);
      onSaved();
    } catch (saveError) {
      toast.error(saveError instanceof Error ? saveError.message : `${capitalise(label.singular)} could not be saved`);
    } finally {
      setSaving(false);
    }
  };

  return <DialogShell onClose={onClose} open={open}><DialogHeader onClose={onClose} title={`${record ? "Edit" : "Add"} ${label.singular}`} /><form onSubmit={submit}><DialogBody className="space-y-5">
    {kind === "class" ? <div><Label>Academic year</Label><Select className="mt-1.5" onChange={(event) => setYearId(event.target.value)} required value={yearId}><option value="">Choose an academic year</option>{years.map((year) => <option key={year.id} value={year.id}>{year.name} · {year.status}</option>)}</Select></div> : null}
    {kind !== "academic-year" ? <div><Label>Code</Label><Input className="mt-1.5" maxLength={40} onChange={(event) => setCode(event.target.value)} required value={code} /></div> : null}
    <div><Label>Name</Label><Input className="mt-1.5" data-autofocus="true" maxLength={160} onChange={(event) => setName(event.target.value)} required value={name} /></div>
    {kind === "class" ? <div><Label>Grade level</Label><Select className="mt-1.5" onChange={(event) => setGradeLevelId(event.target.value)} required value={gradeLevelId}><option value="">Choose a grade level</option>{gradeLevels.filter((grade) => grade.status === "active" || (record !== null && "grade_level_id" in record && grade.id === record.grade_level_id)).map((grade) => <option key={grade.id} value={grade.id}>{grade.name} · {grade.code}</option>)}</Select>{gradeLevels.length === 0 ? <p className="mt-2 text-xs text-[var(--text-muted)]">Add a grade level before creating a class.</p> : null}</div> : null}
    {kind === "academic-year" ? <div className="grid gap-4 sm:grid-cols-2"><div><Label>Start date</Label><Input className="mt-1.5" onChange={(event) => setStartsOn(event.target.value)} required type="date" value={startsOn} /></div><div><Label>End date</Label><Input className="mt-1.5" min={startsOn || undefined} onChange={(event) => setEndsOn(event.target.value)} required type="date" value={endsOn} /></div></div> : null}
    <div><Label>Status</Label><Select className="mt-1.5" onChange={(event) => setStatus(event.target.value)} value={status}>{kind === "academic-year" ? <><option value="planned">Planned</option><option value="active">Active</option><option value="closed">Closed</option></> : <><option value="active">Active</option><option value="inactive">Inactive</option></>}</Select></div>
  </DialogBody><DialogFooter><Button disabled={saving} onClick={onClose} type="button" variant="ghost">Cancel</Button><Button disabled={saving} type="submit">{saving ? <><Loader2 className="size-4 animate-spin" />Saving…</> : "Save"}</Button></DialogFooter></form></DialogShell>;
}

function headersFor(kind: DirectoryKind) {
  if (kind === "academic-year") return ["Academic year", "Dates"];
  if (kind === "subject") return ["Subject", "Code"];
  return ["Class", "Code", "Academic year", "Grade level"];
}

function cellsFor(kind: DirectoryKind, record: DirectoryRecord) {
  if (kind === "academic-year" && "starts_on" in record) return <><TD><span className="font-medium text-[var(--text-strong)]">{record.name}</span></TD><TD className="text-[var(--text-muted)]">{formatDate(record.starts_on)} – {formatDate(record.ends_on)}</TD></>;
  if (kind === "subject" && "code" in record && !("academic_year_id" in record)) return <><TD><span className="font-medium text-[var(--text-strong)]">{record.name}</span></TD><TD className="font-tabular text-[var(--text-muted)]">{record.code}</TD></>;
  const classGroup = record as ClassGroup;
  return <><TD><span className="font-medium text-[var(--text-strong)]">{classGroup.name}</span></TD><TD className="font-tabular text-[var(--text-muted)]">{classGroup.code}</TD><TD className="text-[var(--text-muted)]">{classGroup.academic_year_name}</TD><TD className="text-[var(--text-muted)]">{classGroup.grade_level || "—"}</TD></>;
}

function descriptionFor(kind: DirectoryKind) {
  if (kind === "academic-year") return "Define the academic cycles used by classes, teaching assignments, and timetables.";
  if (kind === "subject") return "Maintain the subjects taught across the campus.";
  return "Classes belong to an academic year and are reused by enrolment and timetabling.";
}

function recordName(record: DirectoryRecord | null, kind: DirectoryKind) {
  return record?.name || `this ${labels[kind].singular}`;
}

function formatDate(value: string) {
  return new Intl.DateTimeFormat(undefined, { day: "numeric", month: "short", year: "numeric", timeZone: "UTC" }).format(new Date(`${value}T00:00:00Z`));
}

function capitalise(value: string) {
  return value.charAt(0).toUpperCase() + value.slice(1);
}

import { useCallback, useEffect, useState } from "react";
import { Link } from "@tanstack/react-router";
import { Edit, GraduationCap, Loader2, MoreVertical, Plus, Search } from "lucide-react";
import toast from "react-hot-toast";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Table, TableControlsBar, TableControlsPagination, TableControlsSearch, TableEmpty, TableError, TableLoading, TableScroll, TableWrap, TBody, TD, TH, THead, TR } from "@/components/ui/data-table";
import { DialogBody, DialogFooter, DialogHeader, DialogShell } from "@/components/ui/dialog";
import { Input, Label, Select } from "@/components/ui/input";
import { usePageChrome } from "@/modules/admin/layouts/page-chrome";
import { useAuthStore } from "@/stores/auth-store";
import { academicsService } from "@/modules/academics";
import type { AcademicYear, ClassGroup } from "@/modules/academics";

import { responseMessage, sisService } from "./service";
import type { Application, Enrolment, EnrolmentStatus, Learner } from "./types";

export function EnrolmentsList() {
  const permissions = useAuthStore((state) => state.user?.permissions ?? []);
  const canCreate = permissions.includes("*") || permissions.includes("sis:create");
  const canEdit = permissions.includes("*") || permissions.includes("sis:edit");
  const [records, setRecords] = useState<Enrolment[]>([]); const [years, setYears] = useState<AcademicYear[]>([]);
  const [loading, setLoading] = useState(true); const [error, setError] = useState<string | null>(null);
  const [search, setSearch] = useState(""); const [submittedSearch, setSubmittedSearch] = useState(""); const [status, setStatus] = useState("all"); const [yearId, setYearId] = useState("all");
  const [page, setPage] = useState(1); const [totalPages, setTotalPages] = useState(1); const [drawerRecord, setDrawerRecord] = useState<Enrolment | null | undefined>(undefined); const [menuId, setMenuId] = useState<string | null>(null);

  const load = useCallback(async () => {
    setLoading(true); setError(null);
    try {
      const response = await sisService.listEnrolments({ page, per_page: 20, search: submittedSearch || undefined, status: status === "all" ? undefined : status, academic_year_id: yearId === "all" ? undefined : yearId });
      if (!response.success || !response.data) throw new Error(responseMessage(response, "Enrolments could not be loaded"));
      setRecords(response.data.enrolments); setTotalPages(response.pagination?.total_pages ?? 1);
    } catch (loadError) { setError(loadError instanceof Error ? loadError.message : "Enrolments could not be loaded"); }
    finally { setLoading(false); }
  }, [page, status, submittedSearch, yearId]);

  useEffect(() => { void load(); }, [load]);
  useEffect(() => { void academicsService.listAcademicYears({ per_page: 100 }).then((response) => { if (response.success && response.data) setYears(response.data.academic_years); }); }, []);
  usePageChrome("Enrolments", canCreate ? <Button onClick={() => setDrawerRecord(null)}><Plus className="size-4" />Add enrolment</Button> : null);
  const filtered = submittedSearch || status !== "all" || yearId !== "all";

  return <div className="space-y-6">
    <p className="text-sm text-[var(--text-muted)]">Enrolments place a learner in one Academics class for an academic year.</p>
    <TableControlsBar><TableControlsSearch onSubmit={(event) => { event.preventDefault(); setPage(1); setSubmittedSearch(search.trim()); }}><Input aria-label="Search enrolments" leadingIcon={<Search />} onChange={(event) => setSearch(event.target.value)} placeholder="Search learner or class…" value={search} /><Button type="submit" variant="secondary">Search</Button></TableControlsSearch><Select aria-label="Academic year filter" className="sm:w-48" onChange={(event) => { setPage(1); setYearId(event.target.value); }} value={yearId}><option value="all">All academic years</option>{years.map((year) => <option key={year.id} value={year.id}>{year.name}</option>)}</Select><Select aria-label="Status filter" className="sm:w-40" onChange={(event) => { setPage(1); setStatus(event.target.value); }} value={status}><option value="all">All statuses</option><option value="active">Active</option><option value="completed">Completed</option><option value="withdrawn">Withdrawn</option></Select>{!loading && records.length > 0 ? <TableControlsPagination onNext={() => setPage((value) => Math.min(totalPages, value + 1))} onPrevious={() => setPage((value) => Math.max(1, value - 1))} page={page} totalPages={totalPages} /> : null}</TableControlsBar>
    <TableWrap>{loading ? <TableLoading columns={6} label="Loading enrolments…" /> : error ? <TableError description={error} onRetry={() => void load()} /> : records.length === 0 ? <TableEmpty description={filtered ? "Change the current filters." : "Add the first class enrolment."} icon={<GraduationCap />} title={filtered ? "No enrolments match these filters" : "No enrolments yet"} /> : <TableScroll><Table><THead><tr><TH>Learner</TH><TH>Academic year</TH><TH>Class</TH><TH>Dates</TH><TH>Status</TH><TH className="text-right">Actions</TH></tr></THead><TBody>{records.map((record) => <TR key={record.id}><TD><Link className="font-medium text-[var(--text-strong)] hover:text-[var(--brand-strong)] hover:underline" params={{ learnerId: record.learner_id }} to="/modules/sis/learners/$learnerId">{record.learner_name}</Link><div className="font-tabular text-xs text-[var(--text-muted)]">{record.learner_number}</div></TD><TD className="text-[var(--text-muted)]">{record.academic_year_name}</TD><TD><div className="font-medium text-[var(--text-strong)]">{record.class_group_name}</div>{record.source_application_id && record.application_number ? <Link className="font-tabular text-xs text-[var(--text-muted)] hover:text-[var(--brand-strong)] hover:underline" params={{ applicationId: record.source_application_id }} to="/modules/sis/applications/$applicationId">{record.application_number}</Link> : null}</TD><TD className="text-[var(--text-muted)]">{formatDate(record.starts_on)}{record.ends_on ? ` – ${formatDate(record.ends_on)}` : ""}</TD><TD><Badge tone={record.status === "active" ? "success" : record.status === "withdrawn" ? "danger" : "neutral"}>{record.status}</Badge></TD><TD className="text-right">{canEdit ? <div className="relative inline-flex"><button aria-label="Enrolment actions" className="inline-flex size-8 items-center justify-center rounded-[var(--radius-md)] hover:bg-[var(--surface-muted)]" onClick={() => setMenuId(menuId === record.id ? null : record.id)} type="button"><MoreVertical className="size-4" /></button>{menuId === record.id ? <div className="absolute right-0 top-9 z-10 w-40 rounded-[var(--radius-lg)] border border-[var(--border)] bg-[var(--surface)] py-1 shadow-[var(--shadow-popover)]"><button className="flex w-full items-center gap-2 px-4 py-2 text-sm hover:bg-[var(--surface-muted)]" onClick={() => { setDrawerRecord(record); setMenuId(null); }} type="button"><Edit className="size-4" />Edit</button></div> : null}</div> : <span className="text-[var(--text-subtle)]">—</span>}</TD></TR>)}</TBody></Table></TableScroll>}</TableWrap>
    <EnrolmentDrawer onClose={() => setDrawerRecord(undefined)} onSaved={() => { setDrawerRecord(undefined); void load(); }} open={drawerRecord !== undefined} record={drawerRecord ?? null} />
  </div>;
}

export function EnrolmentDrawer({ initialApplication = null, onClose, onSaved, open, record }: { initialApplication?: Application | null; onClose: () => void; onSaved: () => void; open: boolean; record: Enrolment | null }) {
  const [learners, setLearners] = useState<Learner[]>([]); const [years, setYears] = useState<AcademicYear[]>([]); const [classes, setClasses] = useState<ClassGroup[]>([]); const [applications, setApplications] = useState<Application[]>([]);
  const [learnerId, setLearnerId] = useState(""); const [academicYearId, setAcademicYearId] = useState(""); const [classId, setClassId] = useState(""); const [applicationId, setApplicationId] = useState("");
  const [startsOn, setStartsOn] = useState(""); const [endsOn, setEndsOn] = useState(""); const [status, setStatus] = useState<EnrolmentStatus>("active"); const [loading, setLoading] = useState(false); const [saving, setSaving] = useState(false);

  useEffect(() => {
    if (!open) return;
    setLearnerId(record?.learner_id ?? initialApplication?.learner_id ?? ""); setAcademicYearId(record?.academic_year_id ?? initialApplication?.academic_year_id ?? ""); setClassId(record?.class_group_id ?? ""); setApplicationId(record?.source_application_id ?? initialApplication?.id ?? ""); setStartsOn(record?.starts_on ?? ""); setEndsOn(record?.ends_on ?? ""); setStatus(record?.status ?? "active");
    setLoading(true);
    void Promise.all([sisService.listLearners({ per_page: 100 }), academicsService.listAcademicYears({ per_page: 100 }), academicsService.listClasses({ per_page: 100 }), sisService.listApplications({ per_page: 100, status: "accepted" })]).then(([learnerResponse, yearResponse, classResponse, applicationResponse]) => {
      if (learnerResponse.success && learnerResponse.data) setLearners(learnerResponse.data.learners);
      if (yearResponse.success && yearResponse.data) setYears(yearResponse.data.academic_years);
      if (classResponse.success && classResponse.data) setClasses(classResponse.data.classes);
      if (applicationResponse.success && applicationResponse.data) setApplications(applicationResponse.data.applications);
    }).finally(() => setLoading(false));
  }, [initialApplication, open, record]);

  const submit = async (event: React.FormEvent) => {
    event.preventDefault(); setSaving(true);
    const data = { learner_id: learnerId, academic_year_id: academicYearId, class_group_id: classId, source_application_id: applicationId || null, starts_on: startsOn, ends_on: endsOn || null, status };
    const response = record ? await sisService.updateEnrolment(record.id, data) : await sisService.createEnrolment(data);
    setSaving(false); if (!response.success) return toast.error(responseMessage(response, "Enrolment could not be saved")); toast.success("Enrolment saved"); onSaved();
  };

  const availableClasses = classes.filter((item) => !academicYearId || item.academic_year_id === academicYearId);
  const selectedClass = classes.find((item) => item.id === classId);
  const availableApplications = applications.filter((item) => (!learnerId || item.learner_id === learnerId) && (!academicYearId || item.academic_year_id === academicYearId) && (!classId || !item.target_grade_level_id || item.target_grade_level_id === selectedClass?.grade_level_id));

  return <DialogShell onClose={onClose} open={open}><DialogHeader onClose={onClose} title={`${record ? "Edit" : "Add"} enrolment`} /><form onSubmit={submit}><DialogBody className="space-y-5">
    <div><Label htmlFor="sis-enrolment-learner">Learner</Label><Select className="mt-1.5" data-autofocus="true" disabled={loading || record !== null || initialApplication !== null} id="sis-enrolment-learner" onChange={(event) => { setLearnerId(event.target.value); setApplicationId(""); }} required value={learnerId}><option value="">Choose a learner</option>{learners.map((learner) => <option key={learner.id} value={learner.id}>{learner.display_name} · {learner.learner_number}</option>)}</Select></div>
    <div><Label htmlFor="sis-enrolment-year">Academic year</Label><Select className="mt-1.5" disabled={loading || initialApplication !== null} id="sis-enrolment-year" onChange={(event) => { setAcademicYearId(event.target.value); setClassId(""); setApplicationId(""); }} required value={academicYearId}><option value="">Choose an academic year</option>{years.map((year) => <option key={year.id} value={year.id}>{year.name} · {year.status}</option>)}</Select></div>
    <div><Label htmlFor="sis-enrolment-class">Class</Label><Select className="mt-1.5" disabled={loading || !academicYearId} id="sis-enrolment-class" onChange={(event) => { const nextClass = classes.find((item) => item.id === event.target.value); setClassId(event.target.value); setApplicationId(initialApplication && (!initialApplication.target_grade_level_id || initialApplication.target_grade_level_id === nextClass?.grade_level_id) ? initialApplication.id : ""); }} required value={classId}><option value="">Choose a class</option>{availableClasses.map((item) => <option key={item.id} value={item.id}>{item.name}{item.grade_level ? ` · ${item.grade_level}` : ""}</option>)}</Select></div>
    <div><Label htmlFor="sis-enrolment-application">Accepted application</Label><Select className="mt-1.5" disabled={loading || !learnerId || !academicYearId || !classId || initialApplication !== null} id="sis-enrolment-application" onChange={(event) => setApplicationId(event.target.value)} value={applicationId}><option value="">No source application</option>{availableApplications.map((application) => <option key={application.id} value={application.id}>{application.application_number} · {application.target_grade_level_name}</option>)}</Select></div>
    <div className="grid gap-4 sm:grid-cols-2"><div><Label htmlFor="sis-enrolment-start">Start date</Label><Input className="mt-1.5" id="sis-enrolment-start" onChange={(event) => setStartsOn(event.target.value)} required type="date" value={startsOn} /></div><div><Label htmlFor="sis-enrolment-end">End date</Label><Input className="mt-1.5" id="sis-enrolment-end" min={startsOn || undefined} onChange={(event) => setEndsOn(event.target.value)} type="date" value={endsOn} /></div></div>
    <div><Label htmlFor="sis-enrolment-status">Status</Label><Select className="mt-1.5" id="sis-enrolment-status" onChange={(event) => setStatus(event.target.value as EnrolmentStatus)} value={status}><option value="active">Active</option><option value="completed">Completed</option><option value="withdrawn">Withdrawn</option></Select></div>
  </DialogBody><DialogFooter><Button disabled={saving} onClick={onClose} type="button" variant="ghost">Cancel</Button><Button disabled={saving || loading} type="submit">{saving ? <><Loader2 className="size-4 animate-spin" />Saving…</> : "Save enrolment"}</Button></DialogFooter></form></DialogShell>;
}

function formatDate(value: string) { return new Intl.DateTimeFormat(undefined, { day: "numeric", month: "short", year: "numeric", timeZone: "UTC" }).format(new Date(`${value}T00:00:00Z`)); }

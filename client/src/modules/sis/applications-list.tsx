import { useCallback, useEffect, useState } from "react";
import { ClipboardList, Edit, Loader2, MoreVertical, Plus, Search, Trash2 } from "lucide-react";
import toast from "react-hot-toast";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { ConfirmDrawer } from "@/components/ui/confirm-drawer";
import { Table, TableControlsBar, TableControlsPagination, TableControlsSearch, TableEmpty, TableError, TableLoading, TableScroll, TableWrap, TBody, TD, TH, THead, TR } from "@/components/ui/data-table";
import { DialogBody, DialogFooter, DialogHeader, DialogShell } from "@/components/ui/dialog";
import { Input, Label, Select, Textarea } from "@/components/ui/input";
import { usePageChrome } from "@/modules/admin/layouts/page-chrome";
import { academicsService } from "@/modules/academics";
import type { AcademicYear, ClassGroup } from "@/modules/academics";

import { responseMessage, sisService } from "./service";
import type { Application, ApplicationStatus, Learner } from "./types";

export function ApplicationsList() {
  const [records, setRecords] = useState<Application[]>([]);
  const [years, setYears] = useState<AcademicYear[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [search, setSearch] = useState("");
  const [submittedSearch, setSubmittedSearch] = useState("");
  const [status, setStatus] = useState("all");
  const [yearId, setYearId] = useState("all");
  const [page, setPage] = useState(1);
  const [totalPages, setTotalPages] = useState(1);
  const [drawerRecord, setDrawerRecord] = useState<Application | null | undefined>(undefined);
  const [deleteRecord, setDeleteRecord] = useState<Application | null>(null);
  const [menuId, setMenuId] = useState<string | null>(null);
  const [deleting, setDeleting] = useState(false);

  const load = useCallback(async () => {
    setLoading(true); setError(null);
    try {
      const response = await sisService.listApplications({ page, per_page: 20, search: submittedSearch || undefined, status: status === "all" ? undefined : status, academic_year_id: yearId === "all" ? undefined : yearId });
      if (!response.success || !response.data) throw new Error(responseMessage(response, "Applications could not be loaded"));
      setRecords(response.data.applications); setTotalPages(response.pagination?.total_pages ?? 1);
    } catch (loadError) { setError(loadError instanceof Error ? loadError.message : "Applications could not be loaded"); }
    finally { setLoading(false); }
  }, [page, status, submittedSearch, yearId]);

  useEffect(() => { void load(); }, [load]);
  useEffect(() => { void academicsService.listAcademicYears({ per_page: 100 }).then((response) => { if (response.success && response.data) setYears(response.data.academic_years); }); }, []);

  const remove = async () => {
    if (!deleteRecord || deleting) return;
    setDeleting(true); const response = await sisService.deleteApplication(deleteRecord.id); setDeleting(false);
    if (!response.success) return toast.error(responseMessage(response, "Application could not be removed"));
    toast.success("Draft application removed"); setDeleteRecord(null); void load();
  };

  usePageChrome("Applications", <Button onClick={() => setDrawerRecord(null)}><Plus className="size-4" />Add application</Button>);
  const filtered = submittedSearch || status !== "all" || yearId !== "all";

  return <div className="space-y-6">
    <p className="text-sm text-[var(--text-muted)]">Applications connect a prospective learner to an academic year and optional target class.</p>
    <TableControlsBar><TableControlsSearch onSubmit={(event) => { event.preventDefault(); setPage(1); setSubmittedSearch(search.trim()); }}><Input aria-label="Search applications" leadingIcon={<Search />} onChange={(event) => setSearch(event.target.value)} placeholder="Search number or learner…" value={search} /><Button type="submit" variant="secondary">Search</Button></TableControlsSearch><Select aria-label="Academic year filter" className="sm:w-48" onChange={(event) => { setPage(1); setYearId(event.target.value); }} value={yearId}><option value="all">All academic years</option>{years.map((year) => <option key={year.id} value={year.id}>{year.name}</option>)}</Select><Select aria-label="Status filter" className="sm:w-44" onChange={(event) => { setPage(1); setStatus(event.target.value); }} value={status}><option value="all">All statuses</option>{applicationStatuses.map((item) => <option key={item} value={item}>{displayStatus(item)}</option>)}</Select>{!loading && records.length > 0 ? <TableControlsPagination onNext={() => setPage((value) => Math.min(totalPages, value + 1))} onPrevious={() => setPage((value) => Math.max(1, value - 1))} page={page} totalPages={totalPages} /> : null}</TableControlsBar>
    <TableWrap>{loading ? <TableLoading columns={6} label="Loading applications…" /> : error ? <TableError description={error} onRetry={() => void load()} /> : records.length === 0 ? <TableEmpty description={filtered ? "Change the current filters." : "Add the first learner application."} icon={<ClipboardList />} title={filtered ? "No applications match these filters" : "No applications yet"} /> : <TableScroll><Table><THead><tr><TH>Application</TH><TH>Learner</TH><TH>Academic year</TH><TH>Target class</TH><TH>Status</TH><TH className="text-right">Actions</TH></tr></THead><TBody>{records.map((record) => <TR key={record.id}><TD><div className="font-tabular font-medium text-[var(--text-strong)]">{record.application_number}</div><div className="text-xs text-[var(--text-muted)]">{record.submitted_on ? `Submitted ${formatDate(record.submitted_on)}` : "Not submitted"}</div></TD><TD><div className="font-medium text-[var(--text-strong)]">{record.learner_name}</div><div className="font-tabular text-xs text-[var(--text-muted)]">{record.learner_number}</div></TD><TD className="text-[var(--text-muted)]">{record.academic_year_name}</TD><TD className="text-[var(--text-muted)]">{record.target_class_group_name || "—"}</TD><TD><Badge tone={applicationTone(record.status)}>{displayStatus(record.status)}</Badge></TD><TD className="text-right"><div className="relative inline-flex"><button aria-label="Application actions" className="inline-flex size-8 items-center justify-center rounded-[var(--radius-md)] hover:bg-[var(--surface-muted)]" onClick={() => setMenuId(menuId === record.id ? null : record.id)} type="button"><MoreVertical className="size-4" /></button>{menuId === record.id ? <div className="absolute right-0 top-9 z-10 w-40 rounded-[var(--radius-lg)] border border-[var(--border)] bg-[var(--surface)] py-1 shadow-[var(--shadow-popover)]"><button className="flex w-full items-center gap-2 px-4 py-2 text-sm hover:bg-[var(--surface-muted)]" onClick={() => { setDrawerRecord(record); setMenuId(null); }} type="button"><Edit className="size-4" />Edit</button>{record.status === "draft" ? <button className="flex w-full items-center gap-2 px-4 py-2 text-sm text-[var(--tone-danger)] hover:bg-[var(--tone-danger-bg)]" onClick={() => { setDeleteRecord(record); setMenuId(null); }} type="button"><Trash2 className="size-4" />Remove</button> : null}</div> : null}</div></TD></TR>)}</TBody></Table></TableScroll>}</TableWrap>
    <ApplicationDrawer onClose={() => setDrawerRecord(undefined)} onSaved={() => { setDrawerRecord(undefined); void load(); }} open={drawerRecord !== undefined} record={drawerRecord ?? null} />
    <ConfirmDrawer confirmLabel="Remove draft" description={`Remove application ${deleteRecord?.application_number || ""}?`} isPending={deleting} onClose={() => setDeleteRecord(null)} onConfirm={() => void remove()} open={deleteRecord !== null} title="Remove draft application?" />
  </div>;
}

function ApplicationDrawer({ onClose, onSaved, open, record }: { onClose: () => void; onSaved: () => void; open: boolean; record: Application | null }) {
  const [learners, setLearners] = useState<Learner[]>([]); const [years, setYears] = useState<AcademicYear[]>([]); const [classes, setClasses] = useState<ClassGroup[]>([]);
  const [applicationNumber, setApplicationNumber] = useState(""); const [learnerId, setLearnerId] = useState(""); const [academicYearId, setAcademicYearId] = useState(""); const [classId, setClassId] = useState("");
  const [submittedOn, setSubmittedOn] = useState(""); const [status, setStatus] = useState<ApplicationStatus>("draft"); const [notes, setNotes] = useState(""); const [loading, setLoading] = useState(false); const [saving, setSaving] = useState(false);

  useEffect(() => {
    if (!open) return;
    setApplicationNumber(record?.application_number ?? ""); setLearnerId(record?.learner_id ?? ""); setAcademicYearId(record?.academic_year_id ?? ""); setClassId(record?.target_class_group_id ?? ""); setSubmittedOn(record?.submitted_on ?? ""); setStatus(record?.status ?? "draft"); setNotes(record?.notes ?? "");
    setLoading(true);
    void Promise.all([sisService.listLearners({ per_page: 100 }), academicsService.listAcademicYears({ per_page: 100 }), academicsService.listClasses({ per_page: 100 })]).then(([learnerResponse, yearResponse, classResponse]) => {
      if (learnerResponse.success && learnerResponse.data) setLearners(learnerResponse.data.learners);
      if (yearResponse.success && yearResponse.data) setYears(yearResponse.data.academic_years);
      if (classResponse.success && classResponse.data) setClasses(classResponse.data.classes);
    }).finally(() => setLoading(false));
  }, [open, record]);

  const changeStatus = (next: ApplicationStatus) => { setStatus(next); if (next !== "draft" && !submittedOn) setSubmittedOn(new Date().toISOString().slice(0, 10)); };
  const submit = async (event: React.FormEvent) => {
    event.preventDefault(); setSaving(true);
    const data = { application_number: applicationNumber.trim(), learner_id: learnerId, academic_year_id: academicYearId, target_class_group_id: classId || null, submitted_on: status === "draft" ? null : submittedOn, status, notes: notes.trim() || null };
    const response = record ? await sisService.updateApplication(record.id, data) : await sisService.createApplication(data);
    setSaving(false); if (!response.success) return toast.error(responseMessage(response, "Application could not be saved")); toast.success("Application saved"); onSaved();
  };
  const choicesLocked = record !== null && record.status !== "draft";
  const availableClasses = classes.filter((item) => !academicYearId || item.academic_year_id === academicYearId);

  return <DialogShell onClose={onClose} open={open}><DialogHeader onClose={onClose} title={`${record ? "Edit" : "Add"} application`} /><form onSubmit={submit}><DialogBody className="space-y-5">
    <div><Label>Application number</Label><Input className="mt-1.5" data-autofocus="true" maxLength={80} onChange={(event) => setApplicationNumber(event.target.value)} required value={applicationNumber} /></div>
    <div><Label>Learner</Label><Select className="mt-1.5" disabled={loading || choicesLocked} onChange={(event) => setLearnerId(event.target.value)} required value={learnerId}><option value="">Choose a learner</option>{learners.map((learner) => <option key={learner.id} value={learner.id}>{learner.display_name} · {learner.learner_number}</option>)}</Select></div>
    <div><Label>Academic year</Label><Select className="mt-1.5" disabled={loading || choicesLocked} onChange={(event) => { setAcademicYearId(event.target.value); setClassId(""); }} required value={academicYearId}><option value="">Choose an academic year</option>{years.map((year) => <option key={year.id} value={year.id}>{year.name} · {year.status}</option>)}</Select></div>
    <div><Label>Target class</Label><Select className="mt-1.5" disabled={!academicYearId || loading} onChange={(event) => setClassId(event.target.value)} value={classId}><option value="">No target class</option>{availableClasses.map((item) => <option key={item.id} value={item.id}>{item.name}</option>)}</Select></div>
    <div><Label>Status</Label><Select className="mt-1.5" onChange={(event) => changeStatus(event.target.value as ApplicationStatus)} value={status}>{applicationStatuses.map((item) => <option key={item} value={item}>{displayStatus(item)}</option>)}</Select></div>
    {status !== "draft" ? <div><Label>Submitted on</Label><Input className="mt-1.5" onChange={(event) => setSubmittedOn(event.target.value)} required type="date" value={submittedOn} /></div> : null}
    <div><Label>Notes</Label><Textarea className="mt-1.5 min-h-28" maxLength={4000} onChange={(event) => setNotes(event.target.value)} value={notes} /></div>
  </DialogBody><DialogFooter><Button disabled={saving} onClick={onClose} type="button" variant="ghost">Cancel</Button><Button disabled={saving || loading} type="submit">{saving ? <><Loader2 className="size-4 animate-spin" />Saving…</> : "Save application"}</Button></DialogFooter></form></DialogShell>;
}

const applicationStatuses: ApplicationStatus[] = ["draft", "submitted", "under_review", "offered", "accepted", "rejected", "withdrawn"];
function displayStatus(value: string) { return value.replace(/_/g, " ").replace(/^./, (letter) => letter.toUpperCase()); }
function applicationTone(status: ApplicationStatus): "neutral" | "warning" | "success" | "danger" | "info" { if (status === "accepted") return "success"; if (status === "rejected" || status === "withdrawn") return "danger"; if (status === "submitted" || status === "under_review" || status === "offered") return "info"; return "neutral"; }
function formatDate(value: string) { return new Intl.DateTimeFormat(undefined, { day: "numeric", month: "short", year: "numeric", timeZone: "UTC" }).format(new Date(`${value}T00:00:00Z`)); }

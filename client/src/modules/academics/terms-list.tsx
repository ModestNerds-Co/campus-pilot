import { useCallback, useEffect, useMemo, useState } from "react";
import { CalendarDays, Edit, Loader2, MoreVertical, Plus, Search, Trash2 } from "lucide-react";
import toast from "react-hot-toast";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { ConfirmDrawer } from "@/components/ui/confirm-drawer";
import {
  Table,
  TableControlsBar,
  TableControlsPagination,
  TableControlsSearch,
  TableEmpty,
  TableError,
  TableLoading,
  TableScroll,
  TableWrap,
  TBody,
  TD,
  TH,
  THead,
  TR,
} from "@/components/ui/data-table";
import { DialogBody, DialogFooter, DialogHeader, DialogShell } from "@/components/ui/dialog";
import { Input, Label, Select } from "@/components/ui/input";
import { usePageChrome } from "@/modules/admin/layouts/page-chrome";

import { academicsService, responseMessage } from "./service";
import type { AcademicTerm, AcademicTermInput, AcademicYear, AcademicYearStatus } from "./types";

export function AcademicTermsList() {
  const [terms, setTerms] = useState<AcademicTerm[]>([]);
  const [years, setYears] = useState<AcademicYear[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [search, setSearch] = useState("");
  const [submittedSearch, setSubmittedSearch] = useState("");
  const [status, setStatus] = useState("all");
  const [yearId, setYearId] = useState("all");
  const [page, setPage] = useState(1);
  const [totalPages, setTotalPages] = useState(1);
  const [drawerRecord, setDrawerRecord] = useState<AcademicTerm | null | undefined>(undefined);
  const [deleteRecord, setDeleteRecord] = useState<AcademicTerm | null>(null);
  const [menuId, setMenuId] = useState<string | null>(null);
  const [deleting, setDeleting] = useState(false);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const response = await academicsService.listAcademicTerms({
        page,
        per_page: 20,
        search: submittedSearch || undefined,
        status: status === "all" ? undefined : status,
        academic_year_id: yearId === "all" ? undefined : yearId,
      });
      if (!response.success || !response.data) {
        throw new Error(responseMessage(response, "Academic terms could not be loaded"));
      }
      setTerms(response.data.terms);
      setTotalPages(response.pagination?.total_pages ?? 1);
    } catch (loadError) {
      setError(loadError instanceof Error ? loadError.message : "Academic terms could not be loaded");
    } finally {
      setLoading(false);
    }
  }, [page, status, submittedSearch, yearId]);

  useEffect(() => { void load(); }, [load]);
  useEffect(() => {
    void academicsService.listAcademicYears({ per_page: 100 }).then((response) => {
      if (response.success && response.data) setYears(response.data.academic_years);
    });
  }, []);

  const remove = async () => {
    if (!deleteRecord || deleting) return;
    setDeleting(true);
    const response = await academicsService.deleteAcademicTerm(deleteRecord.id);
    setDeleting(false);
    if (response.success) {
      toast.success("Academic term removed");
      setDeleteRecord(null);
      void load();
    } else {
      toast.error(responseMessage(response, "Academic term could not be removed"));
    }
  };

  usePageChrome("Academic terms", <Button onClick={() => setDrawerRecord(null)}><Plus className="size-4" />Add term</Button>);
  const filtered = submittedSearch || status !== "all" || yearId !== "all";

  return (
    <div className="space-y-6">
      <p className="text-sm text-[var(--text-muted)]">Terms divide an academic year into non-overlapping teaching periods.</p>
      <TableControlsBar>
        <TableControlsSearch onSubmit={(event) => { event.preventDefault(); setPage(1); setSubmittedSearch(search.trim()); }}>
          <Input aria-label="Search academic terms" leadingIcon={<Search />} onChange={(event) => setSearch(event.target.value)} placeholder="Search terms…" value={search} />
          <Button type="submit" variant="secondary">Search</Button>
        </TableControlsSearch>
        <Select aria-label="Academic year filter" className="sm:w-48" onChange={(event) => { setPage(1); setYearId(event.target.value); }} value={yearId}>
          <option value="all">All academic years</option>
          {years.map((year) => <option key={year.id} value={year.id}>{year.name}</option>)}
        </Select>
        <Select aria-label="Status filter" className="sm:w-40" onChange={(event) => { setPage(1); setStatus(event.target.value); }} value={status}>
          <option value="all">All statuses</option>
          <option value="planned">Planned</option>
          <option value="active">Active</option>
          <option value="closed">Closed</option>
        </Select>
        {!loading && terms.length > 0 ? <TableControlsPagination onNext={() => setPage((value) => Math.min(totalPages, value + 1))} onPrevious={() => setPage((value) => Math.max(1, value - 1))} page={page} totalPages={totalPages} /> : null}
      </TableControlsBar>

      <TableWrap>
        {loading ? <TableLoading columns={6} label="Loading academic terms…" /> : error ? <TableError description={error} onRetry={() => void load()} /> : terms.length === 0 ? (
          <TableEmpty description={filtered ? "Change the current filters." : "Add the first term to an academic year."} icon={<CalendarDays />} title={filtered ? "No terms match these filters" : "No academic terms yet"} />
        ) : <TableScroll><Table><THead><tr><TH>Term</TH><TH>Code</TH><TH>Academic year</TH><TH>Dates</TH><TH>Status</TH><TH className="text-right">Actions</TH></tr></THead><TBody>
          {terms.map((term) => <TR key={term.id}>
            <TD className="font-medium text-[var(--text-strong)]">{term.name}</TD>
            <TD className="font-tabular text-[var(--text-muted)]">{term.code}</TD>
            <TD className="text-[var(--text-muted)]">{term.academic_year_name}</TD>
            <TD className="text-[var(--text-muted)]">{formatDate(term.starts_on)} – {formatDate(term.ends_on)}</TD>
            <TD><Badge tone={term.status === "active" ? "success" : term.status === "planned" ? "warning" : "neutral"}>{term.status}</Badge></TD>
            <TD className="text-right">{term.status !== "closed" ? <div className="relative inline-flex"><button aria-label="Academic term actions" className="inline-flex size-8 items-center justify-center rounded-[var(--radius-md)] hover:bg-[var(--surface-muted)]" onClick={() => setMenuId(menuId === term.id ? null : term.id)} type="button"><MoreVertical className="size-4" /></button>{menuId === term.id ? <div className="absolute right-0 top-9 z-10 w-40 rounded-[var(--radius-lg)] border border-[var(--border)] bg-[var(--surface)] py-1 shadow-[var(--shadow-popover)]"><button className="flex w-full items-center gap-2 px-4 py-2 text-sm hover:bg-[var(--surface-muted)]" onClick={() => { setDrawerRecord(term); setMenuId(null); }} type="button"><Edit className="size-4" />Edit</button>{term.status === "planned" ? <button className="flex w-full items-center gap-2 px-4 py-2 text-sm text-[var(--tone-danger)] hover:bg-[var(--tone-danger-bg)]" onClick={() => { setDeleteRecord(term); setMenuId(null); }} type="button"><Trash2 className="size-4" />Remove</button> : null}</div> : null}</div> : <span className="text-[var(--text-subtle)]">—</span>}</TD>
          </TR>)}
        </TBody></Table></TableScroll>}
      </TableWrap>

      <AcademicTermDrawer onClose={() => setDrawerRecord(undefined)} onSaved={() => { setDrawerRecord(undefined); void load(); }} open={drawerRecord !== undefined} record={drawerRecord ?? null} years={years} />
      <ConfirmDrawer confirmLabel="Remove term" description={`Remove ${deleteRecord?.name || "this academic term"}?`} isPending={deleting} onClose={() => setDeleteRecord(null)} onConfirm={() => void remove()} open={deleteRecord !== null} title="Remove academic term?" />
    </div>
  );
}

function AcademicTermDrawer({ onClose, onSaved, open, record, years }: { onClose: () => void; onSaved: () => void; open: boolean; record: AcademicTerm | null; years: AcademicYear[] }) {
  const [academicYearId, setAcademicYearId] = useState("");
  const [code, setCode] = useState("");
  const [name, setName] = useState("");
  const [startsOn, setStartsOn] = useState("");
  const [endsOn, setEndsOn] = useState("");
  const [status, setStatus] = useState<AcademicYearStatus>("planned");
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    if (!open) return;
    setAcademicYearId(record?.academic_year_id ?? years.find((year) => year.status === "active")?.id ?? years[0]?.id ?? "");
    setCode(record?.code ?? "");
    setName(record?.name ?? "");
    setStartsOn(record?.starts_on ?? "");
    setEndsOn(record?.ends_on ?? "");
    setStatus(record?.status ?? "planned");
  }, [open, record, years]);

  const selectedYear = useMemo(() => years.find((year) => year.id === academicYearId), [academicYearId, years]);
  const fixedBoundary = record?.status === "active";

  const submit = async (event: React.FormEvent) => {
    event.preventDefault();
    setSaving(true);
    const data: AcademicTermInput = {
      academic_year_id: academicYearId,
      code: code.trim(),
      name: name.trim(),
      starts_on: startsOn,
      ends_on: endsOn,
      status,
    };
    try {
      const response = record
        ? await academicsService.updateAcademicTerm(record.id, data)
        : await academicsService.createAcademicTerm(data);
      if (!response.success) throw new Error(responseMessage(response, "Academic term could not be saved"));
      toast.success("Academic term saved");
      onSaved();
    } catch (saveError) {
      toast.error(saveError instanceof Error ? saveError.message : "Academic term could not be saved");
    } finally {
      setSaving(false);
    }
  };

  return <DialogShell onClose={onClose} open={open}><DialogHeader onClose={onClose} title={`${record ? "Edit" : "Add"} academic term`} /><form onSubmit={submit}><DialogBody className="space-y-5">
    {fixedBoundary ? <p className="bg-[var(--surface-muted)] p-4 text-sm leading-6 text-[var(--text-muted)]">The year, code, and dates stay fixed after a term becomes active.</p> : null}
    <div><Label>Academic year</Label><Select className="mt-1.5" data-autofocus="true" disabled={fixedBoundary} onChange={(event) => { setAcademicYearId(event.target.value); setStartsOn(""); setEndsOn(""); }} required value={academicYearId}><option value="">Choose an academic year</option>{years.filter((year) => year.status !== "closed" || year.id === record?.academic_year_id).map((year) => <option key={year.id} value={year.id}>{year.name} · {year.status}</option>)}</Select></div>
    <div className="grid gap-4 sm:grid-cols-2"><div><Label>Code</Label><Input className="mt-1.5" disabled={fixedBoundary} maxLength={40} onChange={(event) => setCode(event.target.value)} required value={code} /></div><div><Label>Name</Label><Input className="mt-1.5" maxLength={120} onChange={(event) => setName(event.target.value)} required value={name} /></div></div>
    <div className="grid gap-4 sm:grid-cols-2"><div><Label>Start date</Label><Input className="mt-1.5" disabled={fixedBoundary} max={selectedYear?.ends_on} min={selectedYear?.starts_on} onChange={(event) => setStartsOn(event.target.value)} required type="date" value={startsOn} /></div><div><Label>End date</Label><Input className="mt-1.5" disabled={fixedBoundary} max={selectedYear?.ends_on} min={startsOn || selectedYear?.starts_on} onChange={(event) => setEndsOn(event.target.value)} required type="date" value={endsOn} /></div></div>
    <div><Label>Status</Label><Select className="mt-1.5" onChange={(event) => setStatus(event.target.value as AcademicYearStatus)} value={status}>{record?.status === "active" ? <><option value="active">Active</option><option value="closed">Closed</option></> : <><option value="planned">Planned</option><option value="active">Active</option><option value="closed">Closed</option></>}</Select></div>
  </DialogBody><DialogFooter><Button disabled={saving} onClick={onClose} type="button" variant="ghost">Cancel</Button><Button disabled={saving} type="submit">{saving ? <><Loader2 className="size-4 animate-spin" />Saving…</> : "Save term"}</Button></DialogFooter></form></DialogShell>;
}

function formatDate(value: string) {
  return new Intl.DateTimeFormat(undefined, { day: "numeric", month: "short", year: "numeric", timeZone: "UTC" }).format(new Date(`${value}T00:00:00Z`));
}

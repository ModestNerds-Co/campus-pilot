import { useCallback, useEffect, useState } from "react";
import type { ReactNode } from "react";
import { CalendarRange, Edit, Loader2, LockKeyhole, Plus, RotateCcw, Search, Trash2, UnlockKeyhole } from "lucide-react";
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

import { financeService, responseMessage } from "./service";
import type {
  FinanceAccountingPeriod, FinanceFiscalYear, FiscalYearInput, PeriodCadence,
} from "./types";

type LifecycleAction =
  | { kind: "open-year"; year: FinanceFiscalYear }
  | { kind: "close-year"; year: FinanceFiscalYear }
  | { kind: "close-period"; period: FinanceAccountingPeriod }
  | { kind: "reopen-period"; period: FinanceAccountingPeriod };

export function AccountingPeriods() {
  const [years, setYears] = useState<FinanceFiscalYear[]>([]);
  const [periods, setPeriods] = useState<FinanceAccountingPeriod[]>([]);
  const [loadingYears, setLoadingYears] = useState(true);
  const [loadingPeriods, setLoadingPeriods] = useState(false);
  const [yearError, setYearError] = useState<string | null>(null);
  const [periodError, setPeriodError] = useState<string | null>(null);
  const [page, setPage] = useState(1);
  const [totalPages, setTotalPages] = useState(1);
  const [search, setSearch] = useState("");
  const [submittedSearch, setSubmittedSearch] = useState("");
  const [status, setStatus] = useState("all");
  const [selectedYearId, setSelectedYearId] = useState<string | null>(null);
  const [drawerRecord, setDrawerRecord] = useState<FinanceFiscalYear | null | undefined>(undefined);
  const [deleteRecord, setDeleteRecord] = useState<FinanceFiscalYear | null>(null);
  const [action, setAction] = useState<LifecycleAction | null>(null);
  const [pending, setPending] = useState(false);

  const loadYears = useCallback(async () => {
    setLoadingYears(true);
    setYearError(null);
    try {
      const response = await financeService.listFiscalYears({
        page,
        per_page: 20,
        search: submittedSearch || undefined,
        status: status === "all" ? undefined : status,
      });
      if (!response.success || !response.data) throw new Error(responseMessage(response, "Fiscal years could not be loaded"));
      setYears(response.data.fiscal_years);
      setTotalPages(response.pagination?.total_pages ?? 1);
      setSelectedYearId((current) => response.data!.fiscal_years.some((year) => year.id === current)
        ? current
        : response.data!.fiscal_years[0]?.id ?? null);
    } catch (loadError) {
      setYearError(loadError instanceof Error ? loadError.message : "Fiscal years could not be loaded");
    } finally {
      setLoadingYears(false);
    }
  }, [page, status, submittedSearch]);

  const loadPeriods = useCallback(async () => {
    if (!selectedYearId) {
      setPeriods([]);
      return;
    }
    setLoadingPeriods(true);
    setPeriodError(null);
    try {
      const response = await financeService.listAccountingPeriods(selectedYearId);
      if (!response.success || !response.data) throw new Error(responseMessage(response, "Accounting periods could not be loaded"));
      setPeriods(response.data.periods);
    } catch (loadError) {
      setPeriodError(loadError instanceof Error ? loadError.message : "Accounting periods could not be loaded");
    } finally {
      setLoadingPeriods(false);
    }
  }, [selectedYearId]);

  useEffect(() => { void loadYears(); }, [loadYears]);
  useEffect(() => { void loadPeriods(); }, [loadPeriods]);

  const selectedYear = years.find((year) => year.id === selectedYearId) ?? null;

  const remove = async () => {
    if (!deleteRecord || pending) return;
    setPending(true);
    const response = await financeService.deleteFiscalYear(deleteRecord.id);
    setPending(false);
    if (response.success) {
      toast.success("Fiscal year removed");
      setDeleteRecord(null);
      void loadYears();
    } else toast.error(responseMessage(response, "Fiscal year could not be removed"));
  };

  const runAction = async () => {
    if (!action || pending) return;
    setPending(true);
    const response = action.kind === "open-year"
      ? await financeService.openFiscalYear(action.year.id)
      : action.kind === "close-year"
        ? await financeService.closeFiscalYear(action.year.id)
        : action.kind === "close-period"
          ? await financeService.closeAccountingPeriod(action.period.id)
          : await financeService.reopenAccountingPeriod(action.period.id);
    setPending(false);
    if (!response.success) {
      toast.error(responseMessage(response, "Accounting calendar could not be updated"));
      return;
    }
    toast.success(actionSuccess(action.kind));
    setAction(null);
    await loadYears();
    await loadPeriods();
  };

  usePageChrome("Fiscal years and periods", <Button onClick={() => setDrawerRecord(null)}><Plus className="size-4" />Add fiscal year</Button>);
  const filtered = submittedSearch || status !== "all";

  return <div className="space-y-8">
    <div>
      <p className="text-sm text-[var(--text-muted)]">Define the dated periods Finance will use for journals and posting.</p>
      <p className="mt-1 text-xs text-[var(--text-subtle)]">A fiscal year must be opened before its periods can accept postings.</p>
    </div>

    <section aria-labelledby="fiscal-years-heading" className="space-y-4">
      <div><h2 className="text-base font-semibold text-[var(--text-strong)]" id="fiscal-years-heading">Fiscal years</h2></div>
      <TableControlsBar>
        <TableControlsSearch onSubmit={(event) => { event.preventDefault(); setPage(1); setSubmittedSearch(search.trim()); }}>
          <Input aria-label="Search fiscal years" leadingIcon={<Search />} onChange={(event) => setSearch(event.target.value)} placeholder="Search fiscal years…" value={search} />
          <Button type="submit" variant="secondary">Search</Button>
        </TableControlsSearch>
        <Select aria-label="Fiscal year status" className="sm:w-40" onChange={(event) => { setPage(1); setStatus(event.target.value); }} value={status}>
          <option value="all">All statuses</option><option value="draft">Draft</option><option value="open">Open</option><option value="closed">Closed</option>
        </Select>
        {!loadingYears && years.length > 0 ? <TableControlsPagination onNext={() => setPage((value) => Math.min(totalPages, value + 1))} onPrevious={() => setPage((value) => Math.max(1, value - 1))} page={page} totalPages={totalPages} /> : null}
      </TableControlsBar>
      <TableWrap>
        {loadingYears ? <TableLoading columns={6} label="Loading fiscal years…" /> : yearError ? <TableError description={yearError} onRetry={() => void loadYears()} /> : years.length === 0 ? <TableEmpty description={filtered ? "Change the current filters." : "Add a fiscal year to generate its accounting periods."} icon={<CalendarRange />} title={filtered ? "No fiscal years match these filters" : "No fiscal years yet"} /> : <TableScroll><Table>
          <THead><tr><TH>Fiscal year</TH><TH>Dates</TH><TH>Cadence</TH><TH>Periods</TH><TH>Status</TH><TH className="text-right">Actions</TH></tr></THead>
          <TBody>{years.map((year) => <TR className={selectedYearId === year.id ? "bg-[var(--surface-muted)]" : undefined} key={year.id}>
            <TD><button className="text-left font-medium text-[var(--text-strong)] hover:text-[var(--brand-strong)]" onClick={() => setSelectedYearId(year.id)} type="button">{year.name}</button></TD>
            <TD className="whitespace-nowrap text-[var(--text-muted)]">{formatDate(year.starts_on)} – {formatDate(year.ends_on)}</TD>
            <TD className="capitalize text-[var(--text-muted)]">{year.period_cadence}</TD>
            <TD className="font-tabular text-[var(--text-muted)]">{year.period_count}{year.status === "open" ? ` · ${year.open_period_count} open` : ""}</TD>
            <TD><StatusBadge status={year.status} /></TD>
            <TD className="text-right"><div className="inline-flex flex-wrap justify-end gap-1">
              {year.status === "draft" ? <><IconAction label={`Edit ${year.name}`} onClick={() => setDrawerRecord(year)}><Edit /></IconAction><IconAction label={`Open ${year.name}`} onClick={() => setAction({ kind: "open-year", year })}><UnlockKeyhole /></IconAction><IconAction danger label={`Remove ${year.name}`} onClick={() => setDeleteRecord(year)}><Trash2 /></IconAction></> : null}
              {year.status === "open" ? <IconAction disabled={year.open_period_count > 0} label={`Close ${year.name}`} onClick={() => setAction({ kind: "close-year", year })}><LockKeyhole /></IconAction> : null}
            </div></TD>
          </TR>)}</TBody>
        </Table></TableScroll>}
      </TableWrap>
    </section>

    {selectedYear ? <section aria-labelledby="accounting-periods-heading" className="space-y-4">
      <div className="flex flex-col gap-2 sm:flex-row sm:items-end sm:justify-between">
        <div><p className="text-[11px] font-semibold uppercase tracking-[0.16em] text-[var(--brand-strong)]">{selectedYear.name}</p><h2 className="mt-1 text-base font-semibold text-[var(--text-strong)]" id="accounting-periods-heading">Accounting periods</h2></div>
        {selectedYear.status === "open" && selectedYear.open_period_count > 0 ? <p className="text-xs text-[var(--text-muted)]">Close every period before closing the fiscal year.</p> : null}
      </div>
      <TableWrap>
        {loadingPeriods ? <TableLoading columns={5} label="Loading accounting periods…" /> : periodError ? <TableError description={periodError} onRetry={() => void loadPeriods()} /> : periods.length === 0 ? <TableEmpty description="This fiscal year has no accounting periods." icon={<CalendarRange />} title="No accounting periods" /> : <TableScroll><Table>
          <THead><tr><TH>#</TH><TH>Period</TH><TH>Dates</TH><TH>Status</TH><TH className="text-right">Action</TH></tr></THead>
          <TBody>{periods.map((period) => <TR key={period.id}>
            <TD className="font-tabular text-[var(--text-subtle)]">{period.period_number}</TD>
            <TD className="font-medium text-[var(--text-strong)]">{period.name}</TD>
            <TD className="whitespace-nowrap text-[var(--text-muted)]">{formatDate(period.starts_on)} – {formatDate(period.ends_on)}</TD>
            <TD><StatusBadge status={period.status} /></TD>
            <TD className="text-right">{selectedYear.status === "open" ? period.status === "open" ? <Button onClick={() => setAction({ kind: "close-period", period })} size="sm" variant="secondary"><LockKeyhole className="size-3.5" />Close</Button> : period.status === "closed" ? <Button onClick={() => setAction({ kind: "reopen-period", period })} size="sm" variant="ghost"><RotateCcw className="size-3.5" />Reopen</Button> : null : <span className="text-xs text-[var(--text-subtle)]">—</span>}</TD>
          </TR>)}</TBody>
        </Table></TableScroll>}
      </TableWrap>
    </section> : null}

    <FiscalYearDrawer onClose={() => setDrawerRecord(undefined)} onSaved={() => { setDrawerRecord(undefined); void loadYears(); }} open={drawerRecord !== undefined} record={drawerRecord ?? null} />
    <ConfirmDrawer confirmLabel="Remove fiscal year" description={`Remove ${deleteRecord?.name ?? "this fiscal year"} and its planned accounting periods?`} isPending={pending} onClose={() => setDeleteRecord(null)} onConfirm={() => void remove()} open={deleteRecord !== null} title="Remove fiscal year?" />
    <LifecycleDrawer action={action} isPending={pending} onClose={() => setAction(null)} onConfirm={() => void runAction()} />
  </div>;
}

function FiscalYearDrawer({ onClose, onSaved, open, record }: { onClose: () => void; onSaved: () => void; open: boolean; record: FinanceFiscalYear | null }) {
  const [name, setName] = useState("");
  const [startsOn, setStartsOn] = useState("");
  const [endsOn, setEndsOn] = useState("");
  const [cadence, setCadence] = useState<PeriodCadence>("monthly");
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    if (!open) return;
    setName(record?.name ?? "");
    setStartsOn(record?.starts_on ?? "");
    setEndsOn(record?.ends_on ?? "");
    setCadence(record?.period_cadence ?? "monthly");
  }, [open, record]);

  const submit = async (event: React.FormEvent) => {
    event.preventDefault();
    setSaving(true);
    const payload: FiscalYearInput = { name: name.trim(), starts_on: startsOn, ends_on: endsOn, period_cadence: cadence };
    const response = record
      ? await financeService.updateFiscalYear(record.id, { name: payload.name })
      : await financeService.createFiscalYear(payload);
    setSaving(false);
    if (!response.success) {
      toast.error(responseMessage(response, "Fiscal year could not be saved"));
      return;
    }
    toast.success("Fiscal year saved");
    onSaved();
  };

  return <DialogShell onClose={onClose} open={open}><DialogHeader onClose={onClose} title={`${record ? "Edit" : "Add"} fiscal year`} /><form onSubmit={submit}>
    <DialogBody className="space-y-5">
      <div><Label>Name</Label><Input className="mt-1.5" data-autofocus="true" maxLength={120} onChange={(event) => setName(event.target.value)} placeholder="2026 financial year" required value={name} /></div>
      <div className="grid gap-4 sm:grid-cols-2"><div><Label>Start date</Label><Input className="mt-1.5" disabled={record !== null} onChange={(event) => setStartsOn(event.target.value)} required type="date" value={startsOn} /></div><div><Label>End date</Label><Input className="mt-1.5" disabled={record !== null} min={startsOn || undefined} onChange={(event) => setEndsOn(event.target.value)} required type="date" value={endsOn} /></div></div>
      <div><Label>Accounting periods</Label><Select className="mt-1.5" disabled={record !== null} onChange={(event) => setCadence(event.target.value as PeriodCadence)} value={cadence}><option value="monthly">Monthly</option><option value="quarterly">Quarterly</option></Select><p className="mt-2 text-xs leading-5 text-[var(--text-muted)]">Periods are generated across the full date range and cannot be restructured after the fiscal year is created.</p></div>
      {record ? <div className="rounded-[var(--radius-lg)] border border-[var(--border)] bg-[var(--surface-muted)] p-4 text-sm text-[var(--text-muted)]">Dates and cadence are fixed. Remove this draft and create another fiscal year if those values are wrong.</div> : null}
    </DialogBody>
    <DialogFooter><Button disabled={saving} onClick={onClose} type="button" variant="ghost">Cancel</Button><Button disabled={saving} type="submit">{saving ? <><Loader2 className="size-4 animate-spin" />Saving…</> : "Save fiscal year"}</Button></DialogFooter>
  </form></DialogShell>;
}

function LifecycleDrawer({ action, isPending, onClose, onConfirm }: { action: LifecycleAction | null; isPending: boolean; onClose: () => void; onConfirm: () => void }) {
  if (!action) return null;
  const copy = actionCopy(action);
  return <DialogShell onClose={isPending ? () => undefined : onClose} open={true}><DialogHeader onClose={isPending ? undefined : onClose} title={copy.title} /><DialogBody>
    <div className="flex gap-4"><span className="flex size-10 shrink-0 items-center justify-center rounded-[9px] bg-[var(--badge-info-bg)] text-[var(--badge-info-text)]">{action.kind === "reopen-period" || action.kind === "open-year" ? <UnlockKeyhole className="size-5" /> : <LockKeyhole className="size-5" />}</span><p className="max-w-lg text-sm leading-6 text-[var(--text-muted)]">{copy.description}</p></div>
  </DialogBody><DialogFooter><Button disabled={isPending} onClick={onClose} type="button" variant="secondary">Cancel</Button><Button disabled={isPending} onClick={onConfirm} type="button">{isPending ? <Loader2 className="size-4 animate-spin" /> : null}{isPending ? "Updating…" : copy.confirmLabel}</Button></DialogFooter></DialogShell>;
}

function IconAction({ children, danger = false, disabled = false, label, onClick }: { children: ReactNode; danger?: boolean; disabled?: boolean; label: string; onClick: () => void }) {
  return <button aria-label={label} className={`inline-flex size-9 items-center justify-center rounded-[var(--radius-md)] hover:bg-[var(--surface-muted)] disabled:cursor-not-allowed disabled:opacity-35 [&_svg]:size-4 ${danger ? "text-[var(--tone-danger)] hover:bg-[var(--tone-danger-bg)]" : "text-[var(--text-muted)]"}`} disabled={disabled} onClick={onClick} title={label} type="button">{children}</button>;
}

function StatusBadge({ status }: { status: string }) {
  const tone = status === "open" ? "success" : status === "draft" || status === "planned" ? "warning" : "neutral";
  return <Badge tone={tone}>{status}</Badge>;
}

function actionCopy(action: LifecycleAction) {
  if (action.kind === "open-year") return { title: "Open fiscal year?", description: `Open ${action.year.name} and all ${action.year.period_count} accounting periods for posting?`, confirmLabel: "Open fiscal year" };
  if (action.kind === "close-year") return { title: "Close fiscal year?", description: `Close ${action.year.name}? A closed fiscal year cannot be reopened.`, confirmLabel: "Close fiscal year" };
  if (action.kind === "close-period") return { title: "Close accounting period?", description: `Close ${action.period.name}? New journals will not be allowed in this period.`, confirmLabel: "Close period" };
  return { title: "Reopen accounting period?", description: `Reopen ${action.period.name} for journal posting?`, confirmLabel: "Reopen period" };
}

function actionSuccess(kind: LifecycleAction["kind"]) {
  if (kind === "open-year") return "Fiscal year opened";
  if (kind === "close-year") return "Fiscal year closed";
  if (kind === "close-period") return "Accounting period closed";
  return "Accounting period reopened";
}

function formatDate(value: string) {
  return new Intl.DateTimeFormat(undefined, { day: "numeric", month: "short", year: "numeric", timeZone: "UTC" }).format(new Date(`${value}T00:00:00Z`));
}

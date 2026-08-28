import { useCallback, useEffect, useState } from "react";
import { Coins, Edit, Loader2, Plus, Search, Trash2 } from "lucide-react";
import toast from "react-hot-toast";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { ConfirmDrawer } from "@/components/ui/confirm-drawer";
import { Table, TableControlsBar, TableControlsPagination, TableControlsSearch, TableEmpty, TableError, TableLoading, TableScroll, TableWrap, TBody, TD, TH, THead, TR } from "@/components/ui/data-table";
import { DialogBody, DialogFooter, DialogHeader, DialogShell } from "@/components/ui/dialog";
import { Input, Label, Select } from "@/components/ui/input";
import { usePageChrome } from "@/modules/admin/layouts/page-chrome";

import { financeService, responseMessage } from "./service";
import type { FinanceCurrency, RecordStatus } from "./types";

export function CurrenciesList() {
  const [records, setRecords] = useState<FinanceCurrency[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [page, setPage] = useState(1);
  const [totalPages, setTotalPages] = useState(1);
  const [search, setSearch] = useState("");
  const [submittedSearch, setSubmittedSearch] = useState("");
  const [status, setStatus] = useState("all");
  const [hasReportingCurrency, setHasReportingCurrency] = useState(false);
  const [drawerRecord, setDrawerRecord] = useState<FinanceCurrency | null | undefined>(undefined);
  const [deleteRecord, setDeleteRecord] = useState<FinanceCurrency | null>(null);
  const [deleting, setDeleting] = useState(false);

  const load = useCallback(async () => {
    setLoading(true); setError(null);
    try {
      const [response, reportingResponse] = await Promise.all([
        financeService.listCurrencies({ page, per_page: 25, search: submittedSearch || undefined, status: status === "all" ? undefined : status }),
        financeService.listCurrencies({ page: 1, per_page: 1 }),
      ]);
      if (!response.success || !response.data) throw new Error(responseMessage(response, "Currencies could not be loaded"));
      if (!reportingResponse.success || !reportingResponse.data) throw new Error(responseMessage(reportingResponse, "Reporting currency could not be loaded"));
      setRecords(response.data.currencies);
      setHasReportingCurrency(reportingResponse.data.currencies.some((currency) => currency.is_reporting));
      setTotalPages(response.pagination?.total_pages ?? 1);
    } catch (loadError) {
      setError(loadError instanceof Error ? loadError.message : "Currencies could not be loaded");
    } finally { setLoading(false); }
  }, [page, status, submittedSearch]);

  useEffect(() => { void load(); }, [load]);

  const remove = async () => {
    if (!deleteRecord || deleting) return;
    setDeleting(true);
    const response = await financeService.deleteCurrency(deleteRecord.id);
    setDeleting(false);
    if (response.success) {
      toast.success("Currency removed"); setDeleteRecord(null); void load();
    } else toast.error(responseMessage(response, "Currency could not be removed"));
  };

  usePageChrome("Currencies", <Button onClick={() => setDrawerRecord(null)}><Plus className="size-4" />Add currency</Button>);
  const filtered = submittedSearch || status !== "all";

  return <div className="space-y-6">
    <div>
      <p className="text-sm text-[var(--text-muted)]">Set the reporting currency and the currencies finance accounts may use.</p>
      <p className="mt-1 text-xs text-[var(--text-subtle)]">Amounts and exchange rates are recorded with transactions in the journal slice.</p>
    </div>
    <TableControlsBar>
      <TableControlsSearch onSubmit={(event) => { event.preventDefault(); setPage(1); setSubmittedSearch(search.trim()); }}>
        <Input aria-label="Search currencies" leadingIcon={<Search />} onChange={(event) => setSearch(event.target.value)} placeholder="Search currencies…" value={search} />
        <Button type="submit" variant="secondary">Search</Button>
      </TableControlsSearch>
      <Select aria-label="Status filter" className="sm:w-40" onChange={(event) => { setPage(1); setStatus(event.target.value); }} value={status}>
        <option value="all">All statuses</option><option value="active">Active</option><option value="inactive">Inactive</option>
      </Select>
      {!loading && records.length > 0 ? <TableControlsPagination onNext={() => setPage((value) => Math.min(totalPages, value + 1))} onPrevious={() => setPage((value) => Math.max(1, value - 1))} page={page} totalPages={totalPages} /> : null}
    </TableControlsBar>
    <TableWrap>
      {loading ? <TableLoading columns={6} label="Loading currencies…" /> : error ? <TableError description={error} onRetry={() => void load()} /> : records.length === 0 ? <TableEmpty description={filtered ? "Change the current filters." : "Add the first currency. It will become the reporting currency."} icon={<Coins />} title={filtered ? "No currencies match these filters" : "No currencies yet"} /> : <TableScroll><Table>
        <THead><tr><TH>Currency</TH><TH>Code</TH><TH>Decimals</TH><TH>Use</TH><TH>Status</TH><TH className="text-right">Actions</TH></tr></THead>
        <TBody>{records.map((record) => <TR key={record.id}>
          <TD><span className="font-medium text-[var(--text-strong)]">{record.name}</span>{record.symbol ? <span className="ml-2 text-xs text-[var(--text-subtle)]">{record.symbol}</span> : null}</TD>
          <TD className="font-tabular font-medium">{record.code}</TD>
          <TD className="font-tabular text-[var(--text-muted)]">{record.minor_units}</TD>
          <TD>{record.is_reporting ? <Badge tone="info">Reporting</Badge> : <span className="text-sm text-[var(--text-muted)]">Transaction</span>}</TD>
          <TD><Badge tone={record.status === "active" ? "success" : "neutral"}>{record.status}</Badge></TD>
          <TD className="text-right"><div className="inline-flex gap-1">
            <button aria-label={`Edit ${record.code}`} className="inline-flex size-9 items-center justify-center rounded-[var(--radius-md)] hover:bg-[var(--surface-muted)]" onClick={() => setDrawerRecord(record)} type="button"><Edit className="size-4" /></button>
            <button aria-label={`Remove ${record.code}`} className="inline-flex size-9 items-center justify-center rounded-[var(--radius-md)] text-[var(--tone-danger)] hover:bg-[var(--tone-danger-bg)] disabled:cursor-not-allowed disabled:opacity-35" disabled={record.is_reporting} onClick={() => setDeleteRecord(record)} type="button"><Trash2 className="size-4" /></button>
          </div></TD>
        </TR>)}</TBody>
      </Table></TableScroll>}
    </TableWrap>
    <CurrencyDrawer hasReportingCurrency={hasReportingCurrency} onClose={() => setDrawerRecord(undefined)} onSaved={() => { setDrawerRecord(undefined); void load(); }} open={drawerRecord !== undefined} record={drawerRecord ?? null} />
    <ConfirmDrawer confirmLabel="Remove currency" description={`Remove ${deleteRecord?.code ?? "this currency"}? A currency used by an account cannot be removed.`} isPending={deleting} onClose={() => setDeleteRecord(null)} onConfirm={() => void remove()} open={deleteRecord !== null} title="Remove currency?" />
  </div>;
}

function CurrencyDrawer({ hasReportingCurrency, onClose, onSaved, open, record }: { hasReportingCurrency: boolean; onClose: () => void; onSaved: () => void; open: boolean; record: FinanceCurrency | null }) {
  const [code, setCode] = useState("");
  const [name, setName] = useState("");
  const [symbol, setSymbol] = useState("");
  const [minorUnits, setMinorUnits] = useState("2");
  const [isReporting, setIsReporting] = useState(false);
  const [status, setStatus] = useState<RecordStatus>("active");
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    if (!open) return;
    setCode(record?.code ?? ""); setName(record?.name ?? ""); setSymbol(record?.symbol ?? "");
    setMinorUnits(String(record?.minor_units ?? 2)); setIsReporting(record?.is_reporting ?? !hasReportingCurrency);
    setStatus(record?.status ?? "active");
  }, [hasReportingCurrency, open, record]);

  const submit = async (event: React.FormEvent) => {
    event.preventDefault(); setSaving(true);
    try {
      const payload = { code: code.trim().toUpperCase(), name: name.trim(), symbol: symbol.trim() || null, minor_units: Number(minorUnits), is_reporting: isReporting, status };
      const response = record ? await financeService.updateCurrency(record.id, payload) : await financeService.createCurrency(payload);
      if (!response.success) throw new Error(responseMessage(response, "Currency could not be saved"));
      toast.success("Currency saved"); onSaved();
    } catch (saveError) { toast.error(saveError instanceof Error ? saveError.message : "Currency could not be saved"); }
    finally { setSaving(false); }
  };

  return <DialogShell onClose={onClose} open={open}><DialogHeader onClose={onClose} title={`${record ? "Edit" : "Add"} currency`} /><form onSubmit={submit}>
    <DialogBody className="space-y-5">
      <div><Label>ISO currency code</Label><Input className="mt-1.5 uppercase" data-autofocus="true" maxLength={3} minLength={3} onChange={(event) => setCode(event.target.value)} placeholder="USD" required value={code} /><p className="mt-2 text-xs text-[var(--text-muted)]">Use the three-letter currency code, for example USD, ZWG, or ZAR.</p></div>
      <div><Label>Name</Label><Input className="mt-1.5" maxLength={120} onChange={(event) => setName(event.target.value)} placeholder="US Dollar" required value={name} /></div>
      <div className="grid gap-5 sm:grid-cols-2"><div><Label>Symbol</Label><Input className="mt-1.5" maxLength={8} onChange={(event) => setSymbol(event.target.value)} placeholder="$" value={symbol} /></div><div><Label>Decimal places</Label><Input className="mt-1.5" max={4} min={0} onChange={(event) => setMinorUnits(event.target.value)} required type="number" value={minorUnits} /></div></div>
      <div><Label>Status</Label><Select className="mt-1.5" disabled={isReporting} onChange={(event) => setStatus(event.target.value as RecordStatus)} value={status}><option value="active">Active</option><option value="inactive">Inactive</option></Select></div>
      <label className="flex items-start gap-3 rounded-[var(--radius-lg)] border border-[var(--border)] bg-[var(--surface-muted)] p-4">
        <input checked={isReporting} className="mt-0.5 size-4 accent-[var(--brand-strong)]" disabled={record?.is_reporting || !hasReportingCurrency} onChange={(event) => { setIsReporting(event.target.checked); if (event.target.checked) setStatus("active"); }} type="checkbox" />
        <span><span className="block text-sm font-medium text-[var(--text-strong)]">Reporting currency</span><span className="mt-1 block text-xs leading-5 text-[var(--text-muted)]">Financial statements will be presented in this currency. Choosing it replaces the current reporting currency.</span></span>
      </label>
    </DialogBody>
    <DialogFooter><Button disabled={saving} onClick={onClose} type="button" variant="ghost">Cancel</Button><Button disabled={saving} type="submit">{saving ? <><Loader2 className="size-4 animate-spin" />Saving…</> : "Save currency"}</Button></DialogFooter>
  </form></DialogShell>;
}

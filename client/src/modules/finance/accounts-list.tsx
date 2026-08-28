import { useCallback, useEffect, useState } from "react";
import { Link } from "@tanstack/react-router";
import { Edit, Landmark, Loader2, Plus, Search, Trash2 } from "lucide-react";
import toast from "react-hot-toast";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { ConfirmDrawer } from "@/components/ui/confirm-drawer";
import { Table, TableControlsBar, TableControlsPagination, TableControlsSearch, TableEmpty, TableError, TableLoading, TableScroll, TableWrap, TBody, TD, TH, THead, TR } from "@/components/ui/data-table";
import { DialogBody, DialogFooter, DialogHeader, DialogShell } from "@/components/ui/dialog";
import { Input, Label, Select, Textarea } from "@/components/ui/input";
import { usePageChrome } from "@/modules/admin/layouts/page-chrome";

import { financeService, responseMessage } from "./service";
import type { AccountType, CurrencyMode, FinanceAccount, FinanceCurrency, RecordStatus } from "./types";

const accountTypeLabels: Record<AccountType, string> = {
  asset: "Asset", liability: "Liability", equity: "Equity", income: "Income", expense: "Expense",
};

export function AccountsList() {
  const [records, setRecords] = useState<FinanceAccount[]>([]);
  const [currencies, setCurrencies] = useState<FinanceCurrency[]>([]);
  const [parentCandidates, setParentCandidates] = useState<FinanceAccount[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [page, setPage] = useState(1);
  const [totalPages, setTotalPages] = useState(1);
  const [search, setSearch] = useState("");
  const [submittedSearch, setSubmittedSearch] = useState("");
  const [accountType, setAccountType] = useState("all");
  const [status, setStatus] = useState("all");
  const [drawerRecord, setDrawerRecord] = useState<FinanceAccount | null | undefined>(undefined);
  const [deleteRecord, setDeleteRecord] = useState<FinanceAccount | null>(null);
  const [deleting, setDeleting] = useState(false);

  const load = useCallback(async () => {
    setLoading(true); setError(null);
    try {
      const [accountResponse, currencyResponse, parentResponse] = await Promise.all([
        financeService.listAccounts({ page, per_page: 25, search: submittedSearch || undefined, status: status === "all" ? undefined : status, account_type: accountType === "all" ? undefined : accountType }),
        financeService.listCurrencies({ page: 1, per_page: 100 }),
        financeService.listAccounts({ page: 1, per_page: 100 }),
      ]);
      if (!accountResponse.success || !accountResponse.data) throw new Error(responseMessage(accountResponse, "Chart of accounts could not be loaded"));
      if (!currencyResponse.success || !currencyResponse.data) throw new Error(responseMessage(currencyResponse, "Currencies could not be loaded"));
      if (!parentResponse.success || !parentResponse.data) throw new Error(responseMessage(parentResponse, "Account references could not be loaded"));
      setRecords(accountResponse.data.accounts); setTotalPages(accountResponse.pagination?.total_pages ?? 1);
      setCurrencies(currencyResponse.data.currencies); setParentCandidates(parentResponse.data.accounts);
    } catch (loadError) {
      setError(loadError instanceof Error ? loadError.message : "Chart of accounts could not be loaded");
    } finally { setLoading(false); }
  }, [accountType, page, status, submittedSearch]);

  useEffect(() => { void load(); }, [load]);

  const remove = async () => {
    if (!deleteRecord || deleting) return;
    setDeleting(true);
    const response = await financeService.deleteAccount(deleteRecord.id);
    setDeleting(false);
    if (response.success) {
      toast.success("Account removed"); setDeleteRecord(null); void load();
    } else toast.error(responseMessage(response, "Account could not be removed"));
  };

  usePageChrome("Chart of accounts", <Button disabled={!currencies.some((currency) => currency.is_reporting && currency.status === "active")} onClick={() => setDrawerRecord(null)}><Plus className="size-4" />Add account</Button>);
  const filtered = submittedSearch || accountType !== "all" || status !== "all";
  const reportingCurrency = currencies.find((currency) => currency.is_reporting && currency.status === "active");

  return <div className="space-y-6">
    <div>
      <p className="text-sm text-[var(--text-muted)]">Maintain the account structure Finance will use for journals, budgets, and reports.</p>
      <p className="mt-1 text-xs text-[var(--text-subtle)]">This screen defines structure only. It does not show balances or create postings.</p>
    </div>
    {!loading && !reportingCurrency ? <section className="border border-[var(--tone-warn-bd)] bg-[var(--tone-warn-bg)] p-4">
      <p className="text-sm font-medium text-[var(--text-strong)]">Set a reporting currency before adding accounts.</p>
      <Link className="mt-2 inline-flex text-sm font-semibold text-[var(--brand-strong)] hover:underline" to="/modules/finance/currencies">Open currencies</Link>
    </section> : null}
    <TableControlsBar>
      <TableControlsSearch onSubmit={(event) => { event.preventDefault(); setPage(1); setSubmittedSearch(search.trim()); }}>
        <Input aria-label="Search accounts" leadingIcon={<Search />} onChange={(event) => setSearch(event.target.value)} placeholder="Search account code or name…" value={search} />
        <Button type="submit" variant="secondary">Search</Button>
      </TableControlsSearch>
      <Select aria-label="Account type filter" className="sm:w-40" onChange={(event) => { setPage(1); setAccountType(event.target.value); }} value={accountType}>
        <option value="all">All account types</option>{Object.entries(accountTypeLabels).map(([value, label]) => <option key={value} value={value}>{label}</option>)}
      </Select>
      <Select aria-label="Status filter" className="sm:w-36" onChange={(event) => { setPage(1); setStatus(event.target.value); }} value={status}>
        <option value="all">All statuses</option><option value="active">Active</option><option value="inactive">Inactive</option>
      </Select>
      {!loading && records.length > 0 ? <TableControlsPagination onNext={() => setPage((value) => Math.min(totalPages, value + 1))} onPrevious={() => setPage((value) => Math.max(1, value - 1))} page={page} totalPages={totalPages} /> : null}
    </TableControlsBar>
    <TableWrap>
      {loading ? <TableLoading columns={7} label="Loading chart of accounts…" /> : error ? <TableError description={error} onRetry={() => void load()} /> : records.length === 0 ? <TableEmpty description={filtered ? "Change the current filters." : reportingCurrency ? "Add the first summary or posting account." : "Set a reporting currency first."} icon={<Landmark />} title={filtered ? "No accounts match these filters" : "No finance accounts yet"} /> : <TableScroll><Table>
        <THead><tr><TH>Code</TH><TH>Account</TH><TH>Type</TH><TH>Currency</TH><TH>Posting</TH><TH>Status</TH><TH className="text-right">Actions</TH></tr></THead>
        <TBody>{records.map((record) => <TR key={record.id}>
          <TD className="font-tabular font-medium">{record.code}</TD>
          <TD><span className="font-medium text-[var(--text-strong)]">{record.name}</span>{record.parent_account_code ? <span className="mt-1 block text-xs text-[var(--text-subtle)]">Under {record.parent_account_code}</span> : null}</TD>
          <TD><span className="text-sm">{accountTypeLabels[record.account_type]}</span><span className="mt-1 block text-xs capitalize text-[var(--text-subtle)]">{record.normal_balance}</span></TD>
          <TD><CurrencyUse account={record} reportingCode={reportingCurrency?.code} /></TD>
          <TD>{record.accepts_postings ? <Badge tone="success">Posting</Badge> : <Badge tone="neutral">Summary · {record.child_count}</Badge>}</TD>
          <TD><Badge tone={record.status === "active" ? "success" : "neutral"}>{record.status}</Badge></TD>
          <TD className="text-right"><div className="inline-flex gap-1">
            <button aria-label={`Edit ${record.code}`} className="inline-flex size-9 items-center justify-center rounded-[var(--radius-md)] hover:bg-[var(--surface-muted)]" onClick={() => setDrawerRecord(record)} type="button"><Edit className="size-4" /></button>
            <button aria-label={`Remove ${record.code}`} className="inline-flex size-9 items-center justify-center rounded-[var(--radius-md)] text-[var(--tone-danger)] hover:bg-[var(--tone-danger-bg)] disabled:cursor-not-allowed disabled:opacity-35" disabled={record.child_count > 0} onClick={() => setDeleteRecord(record)} type="button"><Trash2 className="size-4" /></button>
          </div></TD>
        </TR>)}</TBody>
      </Table></TableScroll>}
    </TableWrap>
    <AccountDrawer currencies={currencies} onClose={() => setDrawerRecord(undefined)} onSaved={() => { setDrawerRecord(undefined); void load(); }} open={drawerRecord !== undefined} parents={parentCandidates} record={drawerRecord ?? null} />
    <ConfirmDrawer confirmLabel="Remove account" description={`Remove ${deleteRecord?.code ?? "this account"}? An account with child accounts cannot be removed.`} isPending={deleting} onClose={() => setDeleteRecord(null)} onConfirm={() => void remove()} open={deleteRecord !== null} title="Remove finance account?" />
  </div>;
}

function CurrencyUse({ account, reportingCode }: { account: FinanceAccount; reportingCode?: string }) {
  if (account.currency_mode === "single") return <span className="text-sm font-medium">{account.currency_code}</span>;
  if (account.currency_mode === "multi") return <span className="text-sm text-[var(--text-muted)]">Multiple</span>;
  return <span className="text-sm text-[var(--text-muted)]">{reportingCode ?? "Reporting"}</span>;
}

function AccountDrawer({ currencies, onClose, onSaved, open, parents, record }: { currencies: FinanceCurrency[]; onClose: () => void; onSaved: () => void; open: boolean; parents: FinanceAccount[]; record: FinanceAccount | null }) {
  const [code, setCode] = useState(""); const [name, setName] = useState(""); const [description, setDescription] = useState("");
  const [accountType, setAccountType] = useState<AccountType>("asset"); const [parentId, setParentId] = useState("");
  const [currencyMode, setCurrencyMode] = useState<CurrencyMode>("reporting"); const [currencyId, setCurrencyId] = useState("");
  const [acceptsPostings, setAcceptsPostings] = useState(true); const [status, setStatus] = useState<RecordStatus>("active");
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    if (!open) return;
    setCode(record?.code ?? ""); setName(record?.name ?? ""); setDescription(record?.description ?? "");
    setAccountType(record?.account_type ?? "asset"); setParentId(record?.parent_account_id ?? "");
    setCurrencyMode(record?.currency_mode ?? "reporting"); setCurrencyId(record?.currency_id ?? "");
    setAcceptsPostings(record?.accepts_postings ?? true); setStatus(record?.status ?? "active");
  }, [open, record]);

  const availableParents = parents.filter((parent) => parent.id !== record?.id && parent.account_type === accountType && !parent.accepts_postings && parent.status === "active");
  const activeCurrencies = currencies.filter((currency) => currency.status === "active");

  const submit = async (event: React.FormEvent) => {
    event.preventDefault(); setSaving(true);
    try {
      const payload = {
        code: code.trim(), name: name.trim(), description: description.trim() || null, account_type: accountType,
        parent_account_id: parentId || null, currency_mode: currencyMode,
        currency_id: currencyMode === "single" ? currencyId || null : null,
        accepts_postings: acceptsPostings, status,
      };
      const response = record ? await financeService.updateAccount(record.id, payload) : await financeService.createAccount(payload);
      if (!response.success) throw new Error(responseMessage(response, "Finance account could not be saved"));
      toast.success("Finance account saved"); onSaved();
    } catch (saveError) { toast.error(saveError instanceof Error ? saveError.message : "Finance account could not be saved"); }
    finally { setSaving(false); }
  };

  return <DialogShell onClose={onClose} open={open}><DialogHeader onClose={onClose} title={`${record ? "Edit" : "Add"} finance account`} /><form onSubmit={submit}>
    <DialogBody className="space-y-5">
      <div className="grid gap-5 sm:grid-cols-[0.7fr_1.3fr]"><div><Label>Account code</Label><Input className="mt-1.5" data-autofocus="true" maxLength={40} onChange={(event) => setCode(event.target.value)} placeholder="1000" required value={code} /></div><div><Label>Account name</Label><Input className="mt-1.5" maxLength={160} onChange={(event) => setName(event.target.value)} placeholder="Cash and bank" required value={name} /></div></div>
      <div><Label>Description</Label><Textarea className="mt-1.5" maxLength={1000} onChange={(event) => setDescription(event.target.value)} placeholder="Optional operational description" value={description} /></div>
      <div className="grid gap-5 sm:grid-cols-2"><div><Label>Account type</Label><Select className="mt-1.5" onChange={(event) => { setAccountType(event.target.value as AccountType); setParentId(""); }} value={accountType}>{Object.entries(accountTypeLabels).map(([value, label]) => <option key={value} value={value}>{label}</option>)}</Select></div><div><Label>Parent account</Label><Select className="mt-1.5" onChange={(event) => setParentId(event.target.value)} value={parentId}><option value="">No parent</option>{availableParents.map((parent) => <option key={parent.id} value={parent.id}>{parent.code} · {parent.name}</option>)}</Select><p className="mt-2 text-xs text-[var(--text-muted)]">Only summary accounts of the same type can be parents.</p></div></div>
      <div><Label>Currency use</Label><Select className="mt-1.5" onChange={(event) => { const mode = event.target.value as CurrencyMode; setCurrencyMode(mode); if (mode !== "single") setCurrencyId(""); }} value={currencyMode}><option value="reporting">Reporting currency only</option><option value="single">One selected currency</option><option value="multi">Multiple transaction currencies</option></Select></div>
      {currencyMode === "single" ? <div><Label>Currency</Label><Select className="mt-1.5" onChange={(event) => setCurrencyId(event.target.value)} required value={currencyId}><option value="">Choose a currency</option>{activeCurrencies.map((currency) => <option key={currency.id} value={currency.id}>{currency.code} · {currency.name}</option>)}</Select></div> : null}
      <label className="flex items-start gap-3 rounded-[var(--radius-lg)] border border-[var(--border)] bg-[var(--surface-muted)] p-4"><input checked={acceptsPostings} className="mt-0.5 size-4 accent-[var(--brand-strong)]" disabled={(record?.child_count ?? 0) > 0} onChange={(event) => setAcceptsPostings(event.target.checked)} type="checkbox" /><span><span className="block text-sm font-medium text-[var(--text-strong)]">Posting account</span><span className="mt-1 block text-xs leading-5 text-[var(--text-muted)]">Posting accounts can receive journal lines. Summary accounts can contain child accounts instead.</span></span></label>
      <div><Label>Status</Label><Select className="mt-1.5" onChange={(event) => setStatus(event.target.value as RecordStatus)} value={status}><option value="active">Active</option><option value="inactive">Inactive</option></Select></div>
    </DialogBody>
    <DialogFooter><Button disabled={saving} onClick={onClose} type="button" variant="ghost">Cancel</Button><Button disabled={saving || (currencyMode === "single" && !currencyId)} type="submit">{saving ? <><Loader2 className="size-4 animate-spin" />Saving…</> : "Save account"}</Button></DialogFooter>
  </form></DialogShell>;
}

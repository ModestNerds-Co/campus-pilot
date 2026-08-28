/**
 * Owns journal preparation, review, posting, and reversal UI.
 * Variable-length journal entry stays on the full page; focused lifecycle decisions use drawers.
 */

import { useCallback, useEffect, useMemo, useState } from "react";
import type { FormEvent, ReactNode } from "react";
import {
  ArrowLeft, CheckCircle2, ClipboardCheck, Eye, FileCheck2, FilePlus2, Loader2,
  Plus, RotateCcw, Search, Send, Trash2, XCircle,
} from "lucide-react";
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
import { useAuthStore } from "@/stores/auth-store";

import { financeService, responseMessage } from "./service";
import type {
  FinanceAccount, FinanceCurrency, FinanceJournal, FinanceJournalLine, FinanceJournalSummary,
  JournalInput, JournalLineInput, JournalStatus, JournalValidation,
} from "./types";

type View = { kind: "list" } | { kind: "editor"; journal: FinanceJournal | null } | { kind: "detail"; journal: FinanceJournal };
type LifecycleAction = { kind: "submit" | "approve" | "reject" | "post" | "reverse"; journal: FinanceJournal };

type EditableLine = {
  key: string;
  accountId: string;
  currencyId: string;
  description: string;
  debit: string;
  credit: string;
  reportingDebit: string;
  reportingCredit: string;
  exchangeRate: string;
};

export function JournalsWorkspace() {
  const userId = useAuthStore((state) => state.user?.id);
  const [view, setView] = useState<View>({ kind: "list" });
  const [journals, setJournals] = useState<FinanceJournalSummary[]>([]);
  const [accounts, setAccounts] = useState<FinanceAccount[]>([]);
  const [currencies, setCurrencies] = useState<FinanceCurrency[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [page, setPage] = useState(1);
  const [totalPages, setTotalPages] = useState(1);
  const [search, setSearch] = useState("");
  const [submittedSearch, setSubmittedSearch] = useState("");
  const [status, setStatus] = useState("all");
  const [action, setAction] = useState<LifecycleAction | null>(null);
  const [deleteJournal, setDeleteJournal] = useState<FinanceJournal | null>(null);
  const [pending, setPending] = useState(false);

  const loadJournals = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const response = await financeService.listJournals({
        page,
        per_page: 20,
        search: submittedSearch || undefined,
        status: status === "all" ? undefined : status,
      });
      if (!response.success || !response.data) throw new Error(responseMessage(response, "Journals could not be loaded"));
      setJournals(response.data.journals);
      setTotalPages(response.pagination?.total_pages ?? 1);
    } catch (loadError) {
      setError(loadError instanceof Error ? loadError.message : "Journals could not be loaded");
    } finally {
      setLoading(false);
    }
  }, [page, status, submittedSearch]);

  const loadReferences = useCallback(async () => {
    const [accountResponse, currencyResponse] = await Promise.all([
      financeService.listAccounts({ page: 1, per_page: 100, status: "active" }),
      financeService.listCurrencies({ page: 1, per_page: 100, status: "active" }),
    ]);
    if (accountResponse.success && accountResponse.data) {
      setAccounts(accountResponse.data.accounts.filter((account) => account.accepts_postings));
    }
    if (currencyResponse.success && currencyResponse.data) setCurrencies(currencyResponse.data.currencies);
  }, []);

  useEffect(() => { void loadJournals(); }, [loadJournals]);
  useEffect(() => { void loadReferences(); }, [loadReferences]);

  const openJournal = async (id: string) => {
    const response = await financeService.getJournal(id);
    if (!response.success || !response.data) {
      toast.error(responseMessage(response, "Journal could not be opened"));
      return;
    }
    setView({ kind: "detail", journal: response.data });
  };

  const refreshSelected = async (id: string) => {
    const response = await financeService.getJournal(id);
    if (response.success && response.data) setView({ kind: "detail", journal: response.data });
    await loadJournals();
  };

  const remove = async () => {
    if (!deleteJournal || pending) return;
    setPending(true);
    const response = await financeService.deleteJournal(deleteJournal.id, deleteJournal.version);
    setPending(false);
    if (!response.success) {
      toast.error(responseMessage(response, "Journal could not be removed"));
      return;
    }
    toast.success("Journal removed");
    setDeleteJournal(null);
    setView({ kind: "list" });
    await loadJournals();
  };

  const pageTitle = view.kind === "list" ? "Journals" : view.kind === "editor" ? `${view.journal ? "Edit" : "New"} journal` : view.journal.journal_number;
  usePageChrome(pageTitle, view.kind === "list" ? <Button onClick={() => setView({ kind: "editor", journal: null })}><Plus className="size-4" />New journal</Button> : null);

  if (view.kind === "editor") {
    return <JournalEditor accounts={accounts} currencies={currencies} journal={view.journal} onCancel={() => setView(view.journal ? { kind: "detail", journal: view.journal } : { kind: "list" })} onSaved={(journal) => { setView({ kind: "detail", journal }); void loadJournals(); }} />;
  }

  if (view.kind === "detail") {
    return <>
      <JournalDetail
        journal={view.journal}
        onAction={(kind) => setAction({ kind, journal: view.journal })}
        onBack={() => setView({ kind: "list" })}
        onDelete={() => setDeleteJournal(view.journal)}
        onEdit={() => setView({ kind: "editor", journal: view.journal })}
        userId={userId}
      />
      <JournalActionDrawer action={action} onClose={() => setAction(null)} onDone={async (journal) => { setAction(null); await refreshSelected(journal.id); }} />
      <ConfirmDrawer confirmLabel="Remove journal" description={`Remove ${deleteJournal?.journal_number ?? "this journal"}? Only draft and rejected journals can be removed.`} isPending={pending} onClose={() => setDeleteJournal(null)} onConfirm={() => void remove()} open={deleteJournal !== null} title="Remove journal?" />
    </>;
  }

  const filtered = Boolean(submittedSearch) || status !== "all";
  return <div className="space-y-5">
    <p className="text-sm text-[var(--text-muted)]">Prepare, review, post, and reverse balanced entries.</p>
    <TableControlsBar>
      <TableControlsSearch onSubmit={(event) => { event.preventDefault(); setPage(1); setSubmittedSearch(search.trim()); }}>
        <Input aria-label="Search journals" leadingIcon={<Search />} onChange={(event) => setSearch(event.target.value)} placeholder="Search number, reference, or description…" value={search} />
        <Button type="submit" variant="secondary">Search</Button>
      </TableControlsSearch>
      <Select aria-label="Journal status" className="sm:w-44" onChange={(event) => { setPage(1); setStatus(event.target.value); }} value={status}>
        <option value="all">All statuses</option><option value="draft">Draft</option><option value="submitted">Submitted</option><option value="approved">Approved</option><option value="rejected">Rejected</option><option value="posted">Posted</option><option value="reversed">Reversed</option>
      </Select>
      {!loading && journals.length > 0 ? <TableControlsPagination onNext={() => setPage((value) => Math.min(totalPages, value + 1))} onPrevious={() => setPage((value) => Math.max(1, value - 1))} page={page} totalPages={totalPages} /> : null}
    </TableControlsBar>
    <TableWrap>
      {loading ? <TableLoading columns={7} label="Loading journals…" /> : error ? <TableError description={error} onRetry={() => void loadJournals()} /> : journals.length === 0 ? <TableEmpty description={filtered ? "Change the current filters." : "Create a balanced draft to begin."} icon={<FileCheck2 />} title={filtered ? "No journals match these filters" : "No journals yet"} /> : <TableScroll><Table>
        <THead><tr><TH>Journal</TH><TH>Date</TH><TH>Description</TH><TH>Period</TH><TH>Debit</TH><TH>Status</TH><TH className="text-right">Open</TH></tr></THead>
        <TBody>{journals.map((journal) => <TR key={journal.id}>
          <TD><p className="font-medium text-[var(--text-strong)]">{journal.journal_number}</p>{journal.reference ? <p className="mt-1 text-xs text-[var(--text-subtle)]">{journal.reference}</p> : null}</TD>
          <TD className="whitespace-nowrap text-[var(--text-muted)]">{formatDate(journal.journal_date)}</TD>
          <TD className="max-w-[320px]"><p className="truncate text-[var(--text-body)]">{journal.description}</p><p className="mt-1 text-xs text-[var(--text-subtle)]">{journal.line_count} lines</p></TD>
          <TD className="whitespace-nowrap text-[var(--text-muted)]">{journal.accounting_period_name}</TD>
          <TD className="whitespace-nowrap font-tabular text-[var(--text-strong)]">{formatMinor(journal.reporting_debit_minor, journal.reporting_currency_minor_units, journal.reporting_currency_code)}</TD>
          <TD><JournalStatusBadge status={journal.status} /></TD>
          <TD className="text-right"><Button aria-label={`Open ${journal.journal_number}`} onClick={() => void openJournal(journal.id)} size="icon-sm" variant="ghost"><Eye className="size-4" /></Button></TD>
        </TR>)}</TBody>
      </Table></TableScroll>}
    </TableWrap>
  </div>;
}

function JournalDetail({ journal, onAction, onBack, onDelete, onEdit, userId }: { journal: FinanceJournal; onAction: (kind: LifecycleAction["kind"]) => void; onBack: () => void; onDelete: () => void; onEdit: () => void; userId?: string }) {
  const [validation, setValidation] = useState<JournalValidation | null>(null);
  const [validationError, setValidationError] = useState<string | null>(null);
  const preparer = journal.created_by === userId || journal.submitted_by === userId;
  const editable = journal.status === "draft" || journal.status === "rejected";

  useEffect(() => {
    if (journal.status === "posted" || journal.status === "reversed") return;
    let active = true;
    void financeService.validateJournal(journal.id).then((response) => {
      if (!active) return;
      if (response.success && response.data) setValidation(response.data);
      else setValidationError(responseMessage(response, "Journal validation could not be loaded"));
    });
    return () => { active = false; };
  }, [journal.id, journal.status]);

  return <div className="space-y-6">
    <div className="flex flex-col gap-4 rounded-[var(--radius-xl)] border border-[var(--border)] bg-[var(--surface)] p-5 shadow-[var(--shadow-card)] sm:flex-row sm:items-start sm:justify-between sm:p-6">
      <div className="min-w-0"><Button onClick={onBack} size="sm" variant="ghost"><ArrowLeft className="size-4" />Back to journals</Button><div className="mt-4 flex flex-wrap items-center gap-3"><h2 className="text-xl font-semibold text-[var(--text-strong)]">{journal.journal_number}</h2><JournalStatusBadge status={journal.status} /></div><p className="mt-2 max-w-3xl text-sm leading-6 text-[var(--text-muted)]">{journal.description}</p></div>
      <div className="flex flex-wrap gap-2">
        {editable ? <><Button onClick={onEdit} variant="secondary">Edit</Button><Button onClick={onDelete} variant="ghost"><Trash2 className="size-4" />Remove</Button><Button onClick={() => onAction("submit")}><Send className="size-4" />Submit</Button></> : null}
        {journal.status === "submitted" && !preparer ? <><Button onClick={() => onAction("reject")} variant="secondary"><XCircle className="size-4" />Reject</Button><Button onClick={() => onAction("approve")}><ClipboardCheck className="size-4" />Approve</Button></> : null}
        {journal.status === "approved" && !preparer ? <Button onClick={() => onAction("post")}><CheckCircle2 className="size-4" />Post journal</Button> : null}
        {journal.status === "posted" && !journal.reversal_journal_id && !journal.reversal_of_journal_id ? <Button onClick={() => onAction("reverse")} variant="secondary"><RotateCcw className="size-4" />Create reversal</Button> : null}
      </div>
    </div>

    {journal.status === "submitted" && preparer ? <Notice icon={<ClipboardCheck />} text="Another Finance operator must approve or reject this journal." /> : null}
    {journal.status === "approved" && preparer ? <Notice icon={<ClipboardCheck />} text="Another Finance operator must post this approved journal." /> : null}
    {validationError ? <Notice danger icon={<XCircle />} text={validationError} /> : validation ? <Notice danger={!validation.valid} icon={validation.valid ? <CheckCircle2 /> : <XCircle />} text={validation.valid ? "This journal is balanced and ready for its next lifecycle step." : validation.issues.join(" ")} /> : null}
    {journal.rejection_reason ? <Notice danger icon={<XCircle />} text={`Rejected: ${journal.rejection_reason}`} /> : null}

    <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
      <Fact label="Journal date" value={formatDate(journal.journal_date)} />
      <Fact label="Accounting period" value={`${journal.fiscal_year_name} · ${journal.accounting_period_name}`} />
      <Fact label="Reference" value={journal.reference || "—"} />
      <Fact label="Reporting total" value={formatMinor(journal.reporting_debit_minor, journal.reporting_currency_minor_units, journal.reporting_currency_code)} />
    </div>
    {journal.source_module_key ? <Notice icon={<FileCheck2 />} text={`Source: ${journal.source_module_key} · ${journal.source_record_type} · ${journal.source_record_id}`} /> : null}
    <TableWrap><TableScroll><Table>
      <THead><tr><TH>#</TH><TH>Account</TH><TH>Description</TH><TH>Currency</TH><TH className="text-right">Debit</TH><TH className="text-right">Credit</TH><TH className="text-right">Reporting debit</TH><TH className="text-right">Reporting credit</TH></tr></THead>
      <TBody>{journal.lines.map((line) => <TR key={line.id}><TD className="text-[var(--text-subtle)]">{line.line_number}</TD><TD><p className="font-medium text-[var(--text-strong)]">{line.account_code}</p><p className="mt-1 text-xs text-[var(--text-muted)]">{line.account_name}</p></TD><TD className="text-[var(--text-muted)]">{line.description || "—"}</TD><TD>{line.transaction_currency_code}{line.exchange_rate ? <p className="mt-1 text-xs text-[var(--text-subtle)]">Rate {line.exchange_rate}</p> : null}</TD><MoneyCell amount={line.debit_minor} line={line} /><MoneyCell amount={line.credit_minor} line={line} /><TD className="text-right font-tabular">{minorValue(line.reporting_debit_minor, journal.reporting_currency_minor_units)}</TD><TD className="text-right font-tabular">{minorValue(line.reporting_credit_minor, journal.reporting_currency_minor_units)}</TD></TR>)}</TBody>
      <tfoot className="border-t border-[var(--border)] bg-[var(--surface-muted)]"><tr><TD colSpan={6} className="text-right font-semibold text-[var(--text-strong)]">Reporting totals</TD><TD className="text-right font-tabular font-semibold">{minorValue(journal.reporting_debit_minor, journal.reporting_currency_minor_units)}</TD><TD className="text-right font-tabular font-semibold">{minorValue(journal.reporting_credit_minor, journal.reporting_currency_minor_units)}</TD></tr></tfoot>
    </Table></TableScroll></TableWrap>
  </div>;
}

function JournalEditor({ accounts, currencies, journal, onCancel, onSaved }: { accounts: FinanceAccount[]; currencies: FinanceCurrency[]; journal: FinanceJournal | null; onCancel: () => void; onSaved: (journal: FinanceJournal) => void }) {
  const reporting = currencies.find((currency) => currency.is_reporting) ?? null;
  const [date, setDate] = useState(journal?.journal_date ?? new Date().toISOString().slice(0, 10));
  const [description, setDescription] = useState(journal?.description ?? "");
  const [reference, setReference] = useState(journal?.reference ?? "");
  const [lines, setLines] = useState<EditableLine[]>(() => journal ? journal.lines.map((line) => editableLine(line, journal.reporting_currency_minor_units)) : [emptyLine(), emptyLine()]);
  const [saving, setSaving] = useState(false);

  const totals = useMemo(() => lines.reduce((value, line) => ({ debit: addDecimal(value.debit, line.reportingDebit), credit: addDecimal(value.credit, line.reportingCredit) }), { debit: 0, credit: 0 }), [lines]);

  const setLine = (index: number, patch: Partial<EditableLine>) => setLines((current) => current.map((line, lineIndex) => lineIndex === index ? { ...line, ...patch } : line));
  const setAmount = (index: number, field: "debit" | "credit", value: string) => {
    const line = lines[index];
    const selectedCurrency = currencies.find((currency) => currency.id === line.currencyId);
    const reportingMatch = Boolean(reporting && selectedCurrency?.id === reporting.id);
    if (field === "debit") setLine(index, { debit: value, credit: value ? "" : line.credit, reportingDebit: reportingMatch ? value : line.reportingDebit, reportingCredit: value ? "" : line.reportingCredit });
    else setLine(index, { credit: value, debit: value ? "" : line.debit, reportingCredit: reportingMatch ? value : line.reportingCredit, reportingDebit: value ? "" : line.reportingDebit });
  };

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    if (!reporting) {
      toast.error("Set an active reporting currency before creating journals");
      return;
    }
    let payload: JournalInput;
    try {
      payload = {
        journal_date: date,
        description: requiredText(description, "Description"),
        reference: reference.trim() || null,
        source: journal?.source_module_key ? { module_key: journal.source_module_key, record_type: journal.source_record_type!, record_id: journal.source_record_id! } : null,
        lines: lines.map((line, index) => linePayload(line, index, currencies, reporting)),
      };
    } catch (formError) {
      toast.error(formError instanceof Error ? formError.message : "Journal lines are invalid");
      return;
    }
    setSaving(true);
    const response = journal
      ? await financeService.updateJournal(journal.id, { ...payload, expected_version: journal.version })
      : await financeService.createJournal({ ...payload, idempotency_key: crypto.randomUUID() });
    setSaving(false);
    if (!response.success || !response.data) {
      toast.error(responseMessage(response, "Journal could not be saved"));
      return;
    }
    toast.success("Journal saved as draft");
    onSaved(response.data);
  };

  return <form className="space-y-6" onSubmit={submit}>
    <div className="flex flex-wrap items-center justify-between gap-3"><Button onClick={onCancel} type="button" variant="ghost"><ArrowLeft className="size-4" />Cancel</Button><Button disabled={saving} type="submit">{saving ? <Loader2 className="size-4 animate-spin" /> : <FilePlus2 className="size-4" />}{saving ? "Saving…" : "Save draft"}</Button></div>
    <div className="grid gap-5 rounded-[var(--radius-xl)] border border-[var(--border)] bg-[var(--surface)] p-5 shadow-[var(--shadow-card)] lg:grid-cols-[200px_1fr_280px]">
      <div><Label htmlFor="journal-date">Journal date</Label><Input className="mt-1.5" id="journal-date" onChange={(event) => setDate(event.target.value)} required type="date" value={date} /></div>
      <div><Label htmlFor="journal-description">Description</Label><Input className="mt-1.5" id="journal-description" maxLength={1000} onChange={(event) => setDescription(event.target.value)} placeholder="What this entry records" required value={description} /></div>
      <div><Label htmlFor="journal-reference">Reference</Label><Input className="mt-1.5" id="journal-reference" maxLength={160} onChange={(event) => setReference(event.target.value)} placeholder="Optional document reference" value={reference} /></div>
    </div>
    {!reporting ? <Notice danger icon={<XCircle />} text="Set one active reporting currency before preparing journals." /> : null}
    {accounts.length === 0 ? <Notice danger icon={<XCircle />} text="Add at least two active posting accounts before preparing a journal." /> : null}
    <div className="space-y-4">
      <div className="flex flex-wrap items-end justify-between gap-3"><div><h2 className="text-base font-semibold text-[var(--text-strong)]">Journal lines</h2><p className="mt-1 text-sm text-[var(--text-muted)]">Enter transaction amounts. Foreign-currency lines also require the reporting amount and exchange rate.</p></div><Button onClick={() => setLines((current) => [...current, emptyLine()])} type="button" variant="secondary"><Plus className="size-4" />Add line</Button></div>
      <div className="space-y-3">{lines.map((line, index) => {
        const currency = currencies.find((item) => item.id === line.currencyId);
        const reportingMatch = Boolean(reporting && currency?.id === reporting.id);
        return <div className="rounded-[var(--radius-xl)] border border-[var(--border)] bg-[var(--surface)] p-4 shadow-[var(--shadow-card)]" key={line.key}>
          <div className="mb-4 flex items-center justify-between"><p className="text-sm font-semibold text-[var(--text-strong)]">Line {index + 1}</p><Button aria-label={`Remove line ${index + 1}`} disabled={lines.length <= 2} onClick={() => setLines((current) => current.filter((_, lineIndex) => lineIndex !== index))} size="icon-sm" type="button" variant="ghost"><Trash2 className="size-4" /></Button></div>
          <div className="grid gap-4 lg:grid-cols-12">
            <div className="lg:col-span-4"><Label>Account</Label><Select className="mt-1.5" onChange={(event) => setLine(index, { accountId: event.target.value })} required value={line.accountId}><option value="">Select account</option>{accounts.map((account) => <option key={account.id} value={account.id}>{account.code} · {account.name}</option>)}</Select></div>
            <div className="lg:col-span-2"><Label>Currency</Label><Select className="mt-1.5" onChange={(event) => { const isReporting = event.target.value === reporting?.id; setLine(index, { currencyId: event.target.value, reportingDebit: isReporting ? line.debit : "", reportingCredit: isReporting ? line.credit : "", exchangeRate: "" }); }} required value={line.currencyId}><option value="">Select currency</option>{currencies.map((item) => <option key={item.id} value={item.id}>{item.code}</option>)}</Select></div>
            <div className="lg:col-span-6"><Label>Description</Label><Input className="mt-1.5" maxLength={500} onChange={(event) => setLine(index, { description: event.target.value })} placeholder="Optional line description" value={line.description} /></div>
            <div className="lg:col-span-2"><Label>Debit</Label><Input className="mt-1.5 font-tabular" inputMode="decimal" onChange={(event) => setAmount(index, "debit", event.target.value)} placeholder="0.00" value={line.debit} /></div>
            <div className="lg:col-span-2"><Label>Credit</Label><Input className="mt-1.5 font-tabular" inputMode="decimal" onChange={(event) => setAmount(index, "credit", event.target.value)} placeholder="0.00" value={line.credit} /></div>
            {reportingMatch ? <div className="flex items-end lg:col-span-8"><p className="pb-2 text-sm text-[var(--text-muted)]">Reporting amount matches {reporting?.code} transaction amount.</p></div> : <><div className="lg:col-span-2"><Label>Exchange rate</Label><Input className="mt-1.5 font-tabular" inputMode="decimal" onChange={(event) => setLine(index, { exchangeRate: event.target.value })} placeholder="1.000000" value={line.exchangeRate} /></div><div className="lg:col-span-3"><Label>Reporting debit ({reporting?.code ?? "—"})</Label><Input className="mt-1.5 font-tabular" inputMode="decimal" onChange={(event) => setLine(index, { reportingDebit: event.target.value, reportingCredit: event.target.value ? "" : line.reportingCredit })} placeholder="0.00" value={line.reportingDebit} /></div><div className="lg:col-span-3"><Label>Reporting credit ({reporting?.code ?? "—"})</Label><Input className="mt-1.5 font-tabular" inputMode="decimal" onChange={(event) => setLine(index, { reportingCredit: event.target.value, reportingDebit: event.target.value ? "" : line.reportingDebit })} placeholder="0.00" value={line.reportingCredit} /></div></>}
          </div>
        </div>;
      })}</div>
      <div className="flex flex-col gap-2 rounded-[var(--radius-xl)] border border-[var(--border)] bg-[var(--surface-muted)] p-4 text-sm sm:flex-row sm:items-center sm:justify-end sm:gap-8"><span className="text-[var(--text-muted)]">Reporting totals</span><span className="font-tabular font-semibold text-[var(--text-strong)]">Debit {totals.debit.toFixed(reporting?.minor_units ?? 2)}</span><span className="font-tabular font-semibold text-[var(--text-strong)]">Credit {totals.credit.toFixed(reporting?.minor_units ?? 2)}</span></div>
    </div>
  </form>;
}

function JournalActionDrawer({ action, onClose, onDone }: { action: LifecycleAction | null; onClose: () => void; onDone: (journal: FinanceJournal) => void }) {
  const [reason, setReason] = useState("");
  const [date, setDate] = useState(new Date().toISOString().slice(0, 10));
  const [pending, setPending] = useState(false);
  useEffect(() => { if (action) { setReason(""); setDate(new Date().toISOString().slice(0, 10)); } }, [action]);
  if (!action) return null;
  const copy = actionCopy(action);
  const run = async () => {
    if ((action.kind === "reject" || action.kind === "reverse") && !reason.trim()) {
      toast.error(`${action.kind === "reject" ? "Rejection" : "Reversal"} reason is required`);
      return;
    }
    setPending(true);
    const response = action.kind === "submit" ? await financeService.submitJournal(action.journal.id, action.journal.version)
      : action.kind === "approve" ? await financeService.approveJournal(action.journal.id, action.journal.version)
        : action.kind === "reject" ? await financeService.rejectJournal(action.journal.id, action.journal.version, reason.trim())
          : action.kind === "post" ? await financeService.postJournal(action.journal.id, action.journal.version)
            : await financeService.reverseJournal(action.journal.id, action.journal.version, date, reason.trim());
    setPending(false);
    if (!response.success || !response.data) {
      toast.error(responseMessage(response, copy.failure));
      return;
    }
    toast.success(copy.success);
    onDone(response.data);
  };
  return <DialogShell onClose={pending ? () => undefined : onClose} open={true}><DialogHeader onClose={pending ? undefined : onClose} title={copy.title} /><DialogBody className="space-y-5"><Notice icon={copy.icon} text={copy.description} />{action.kind === "reverse" ? <div><Label htmlFor="reversal-date">Reversal date</Label><Input className="mt-1.5" id="reversal-date" onChange={(event) => setDate(event.target.value)} required type="date" value={date} /></div> : null}{action.kind === "reject" || action.kind === "reverse" ? <div><Label htmlFor="journal-reason">Reason</Label><Textarea className="mt-1.5" data-autofocus="true" id="journal-reason" maxLength={1000} onChange={(event) => setReason(event.target.value)} placeholder={action.kind === "reject" ? "What must be corrected" : "Why the posted entry must be reversed"} required value={reason} /></div> : null}</DialogBody><DialogFooter><Button disabled={pending} onClick={onClose} type="button" variant="secondary">Cancel</Button><Button disabled={pending} onClick={() => void run()} type="button" variant={action.kind === "reject" ? "destructive" : "default"}>{pending ? <Loader2 className="size-4 animate-spin" /> : null}{pending ? "Updating…" : copy.confirm}</Button></DialogFooter></DialogShell>;
}

function actionCopy(action: LifecycleAction) {
  if (action.kind === "submit") return { title: "Submit journal?", description: `${action.journal.journal_number} will become read-only while another Finance operator reviews it.`, confirm: "Submit journal", success: "Journal submitted", failure: "Journal could not be submitted", icon: <Send className="size-5" /> };
  if (action.kind === "approve") return { title: "Approve journal?", description: `Approve ${action.journal.journal_number} for posting? You cannot change its lines after approval.`, confirm: "Approve journal", success: "Journal approved", failure: "Journal could not be approved", icon: <ClipboardCheck className="size-5" /> };
  if (action.kind === "reject") return { title: "Reject journal?", description: `Return ${action.journal.journal_number} to its preparer with a correction reason.`, confirm: "Reject journal", success: "Journal rejected", failure: "Journal could not be rejected", icon: <XCircle className="size-5" /> };
  if (action.kind === "post") return { title: "Post journal?", description: `Post ${action.journal.journal_number} to the ledger. The entry becomes immutable and corrections require a reversal.`, confirm: "Post journal", success: "Journal posted", failure: "Journal could not be posted", icon: <CheckCircle2 className="size-5" /> };
  return { title: "Create reversal?", description: `Create a new draft that reverses ${action.journal.journal_number}. The reversal must be submitted, approved, and posted separately.`, confirm: "Create reversal draft", success: "Reversal draft created", failure: "Reversal could not be created", icon: <RotateCcw className="size-5" /> };
}

function emptyLine(): EditableLine { return { key: crypto.randomUUID(), accountId: "", currencyId: "", description: "", debit: "", credit: "", reportingDebit: "", reportingCredit: "", exchangeRate: "" }; }
function editableLine(line: FinanceJournalLine, reportingMinorUnits: number): EditableLine { return { key: line.id, accountId: line.account_id, currencyId: line.transaction_currency_id, description: line.description ?? "", debit: minorValue(line.debit_minor, line.transaction_currency_minor_units, true), credit: minorValue(line.credit_minor, line.transaction_currency_minor_units, true), reportingDebit: minorValue(line.reporting_debit_minor, reportingMinorUnits, true), reportingCredit: minorValue(line.reporting_credit_minor, reportingMinorUnits, true), exchangeRate: line.exchange_rate ?? "" }; }

function linePayload(line: EditableLine, index: number, currencies: FinanceCurrency[], reporting: FinanceCurrency): JournalLineInput {
  const currency = currencies.find((item) => item.id === line.currencyId);
  if (!line.accountId || !currency) throw new Error(`Choose an account and currency for line ${index + 1}`);
  const debit = majorToMinor(line.debit, currency.minor_units);
  const credit = majorToMinor(line.credit, currency.minor_units);
  const reportingDebit = majorToMinor(currency.id === reporting.id ? line.debit : line.reportingDebit, reporting.minor_units);
  const reportingCredit = majorToMinor(currency.id === reporting.id ? line.credit : line.reportingCredit, reporting.minor_units);
  if ((debit > 0) === (credit > 0)) throw new Error(`Line ${index + 1} needs one debit or one credit`);
  if ((reportingDebit > 0) !== (debit > 0) || (reportingCredit > 0) !== (credit > 0)) throw new Error(`Line ${index + 1} reporting amount must use the same side`);
  if (currency.id !== reporting.id && !line.exchangeRate.trim()) throw new Error(`Line ${index + 1} needs an exchange rate`);
  return { account_id: line.accountId, transaction_currency_id: currency.id, description: line.description.trim() || null, debit_minor: debit, credit_minor: credit, reporting_debit_minor: reportingDebit, reporting_credit_minor: reportingCredit, exchange_rate: currency.id === reporting.id ? null : line.exchangeRate.trim() };
}

function majorToMinor(value: string, minorUnits: number) {
  const normalized = value.trim();
  if (!normalized) return 0;
  if (!/^\d+(\.\d+)?$/.test(normalized)) throw new Error(`Amount ${value} is invalid`);
  const [whole, fraction = ""] = normalized.split(".");
  if (fraction.length > minorUnits) throw new Error(`${value} has too many decimal places`);
  const result = Number(`${whole}${fraction.padEnd(minorUnits, "0")}`);
  if (!Number.isSafeInteger(result) || result > 9_000_000_000_000_000) throw new Error("Amount is too large");
  return result;
}

function minorValue(value: number, minorUnits: number, blankZero = false) { if (blankZero && value === 0) return ""; if (minorUnits === 0) return String(value); return `${Math.trunc(value / 10 ** minorUnits)}.${String(value % 10 ** minorUnits).padStart(minorUnits, "0")}`; }
function formatMinor(value: number, minorUnits: number, code: string) { return `${code} ${minorValue(value, minorUnits)}`; }
function addDecimal(total: number, value: string) { const parsed = Number(value); return Number.isFinite(parsed) ? total + parsed : total; }
function requiredText(value: string, label: string) { const result = value.trim(); if (!result) throw new Error(`${label} is required`); return result; }
function formatDate(value: string) { return new Intl.DateTimeFormat(undefined, { day: "numeric", month: "short", year: "numeric", timeZone: "UTC" }).format(new Date(`${value}T00:00:00Z`)); }

function JournalStatusBadge({ status }: { status: JournalStatus }) { const tone = status === "posted" ? "success" : status === "approved" ? "info" : status === "submitted" || status === "draft" ? "warning" : status === "rejected" ? "danger" : "neutral"; return <Badge tone={tone}>{status}</Badge>; }
function Fact({ label, value }: { label: string; value: string }) { return <div className="rounded-[var(--radius-xl)] border border-[var(--border)] bg-[var(--surface)] p-4 shadow-[var(--shadow-card)]"><p className="text-[11px] font-semibold uppercase tracking-[0.14em] text-[var(--text-subtle)]">{label}</p><p className="mt-2 text-sm font-medium text-[var(--text-strong)]">{value}</p></div>; }
function Notice({ danger = false, icon, text }: { danger?: boolean; icon: ReactNode; text: string }) { return <div className={`flex gap-3 rounded-[var(--radius-xl)] border p-4 ${danger ? "border-[var(--tone-danger-bd)] bg-[var(--tone-danger-bg)] text-[var(--tone-danger)]" : "border-[var(--border)] bg-[var(--surface-muted)] text-[var(--text-muted)]"}`}><span className="mt-0.5 shrink-0 [&_svg]:size-5">{icon}</span><p className="text-sm leading-6">{text}</p></div>; }
function MoneyCell({ amount, line }: { amount: number; line: FinanceJournalLine }) { return <TD className="text-right font-tabular">{amount ? minorValue(amount, line.transaction_currency_minor_units) : "—"}</TD>; }

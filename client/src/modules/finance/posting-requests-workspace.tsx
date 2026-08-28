import { useCallback, useEffect, useState } from "react";
import { ArrowRight, Eye, FileInput, Loader2, Search, XCircle } from "lucide-react";
import toast from "react-hot-toast";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Table, TableControlsBar, TableControlsPagination, TableControlsSearch, TableEmpty, TableError, TableLoading, TableScroll, TableWrap, TBody, TD, TH, THead, TR } from "@/components/ui/data-table";
import { DialogBody, DialogFooter, DialogHeader, DialogShell } from "@/components/ui/dialog";
import { Input, Label, Select, Textarea } from "@/components/ui/input";
import { usePageChrome } from "@/modules/admin/layouts/page-chrome";
import { hasPermission } from "@/modules/users/access-control";
import { useAuthStore } from "@/stores/auth-store";

import { financeService, responseMessage } from "./service";
import type { FinanceCurrency, FinancePostingRequest, FinancePostingRequestSummary, PostingRequestConversionLine } from "./types";

type Action = { kind: "convert" | "reject"; request: FinancePostingRequest };

export function PostingRequestsWorkspace() {
  const permissions = useAuthStore((state) => state.user?.permissions);
  const canConvert = hasPermission(permissions, "finance:create");
  const canReject = hasPermission(permissions, "finance:edit");
  const [records, setRecords] = useState<FinancePostingRequestSummary[]>([]);
  const [currencies, setCurrencies] = useState<FinanceCurrency[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [page, setPage] = useState(1);
  const [totalPages, setTotalPages] = useState(1);
  const [search, setSearch] = useState("");
  const [submittedSearch, setSubmittedSearch] = useState("");
  const [status, setStatus] = useState("pending");
  const [detail, setDetail] = useState<FinancePostingRequest | null>(null);
  const [action, setAction] = useState<Action | null>(null);

  const load = useCallback(async () => {
    setLoading(true); setError(null);
    try {
      const [requestResponse, currencyResponse] = await Promise.all([
        financeService.listPostingRequests({ page, per_page: 25, search: submittedSearch || undefined, status: status === "all" ? undefined : status }),
        financeService.listCurrencies({ page: 1, per_page: 100, status: "active" }),
      ]);
      if (!requestResponse.success || !requestResponse.data) throw new Error(responseMessage(requestResponse, "Posting requests could not be loaded"));
      if (!currencyResponse.success || !currencyResponse.data) throw new Error(responseMessage(currencyResponse, "Finance currencies could not be loaded"));
      setRecords(requestResponse.data.posting_requests);
      setTotalPages(requestResponse.pagination?.total_pages ?? 1);
      setCurrencies(currencyResponse.data.currencies);
    } catch (loadError) {
      setError(loadError instanceof Error ? loadError.message : "Posting requests could not be loaded");
    } finally { setLoading(false); }
  }, [page, status, submittedSearch]);

  useEffect(() => { void load(); }, [load]);
  usePageChrome("Posting requests");

  const open = async (id: string) => {
    const response = await financeService.getPostingRequest(id);
    if (!response.success || !response.data) { toast.error(responseMessage(response, "Posting request could not be loaded")); return; }
    setDetail(response.data);
  };
  const beginAction = (kind: Action["kind"], request: FinancePostingRequest) => { setDetail(null); setAction({ kind, request }); };
  const actionDone = (request: FinancePostingRequest) => { setAction(null); setDetail(request); void load(); };
  const filtered = Boolean(submittedSearch || status !== "all");

  return <div className="space-y-6">
    <p className="text-sm text-[var(--text-muted)]">Review balanced requests from Fees and other operational modules before creating Finance drafts.</p>
    <TableControlsBar>
      <TableControlsSearch onSubmit={(event) => { event.preventDefault(); setPage(1); setSubmittedSearch(search.trim()); }}><Input aria-label="Search posting requests" leadingIcon={<Search />} onChange={(event) => setSearch(event.target.value)} placeholder="Search source, reference, or description…" value={search} /><Button type="submit" variant="secondary">Search</Button></TableControlsSearch>
      <Select aria-label="Posting request status" className="sm:w-44" onChange={(event) => { setPage(1); setStatus(event.target.value); }} value={status}><option value="all">All statuses</option><option value="pending">Pending</option><option value="converted">Converted</option><option value="rejected">Rejected</option><option value="cancelled">Cancelled</option></Select>
      {!loading && records.length > 0 ? <TableControlsPagination onNext={() => setPage((value) => Math.min(totalPages, value + 1))} onPrevious={() => setPage((value) => Math.max(1, value - 1))} page={page} totalPages={totalPages} /> : null}
    </TableControlsBar>
    <TableWrap>{loading ? <TableLoading columns={7} label="Loading posting requests…" /> : error ? <TableError description={error} onRetry={() => void load()} /> : records.length === 0 ? <TableEmpty description={filtered ? "Change the current filters." : "No operational posting requests are available."} icon={<FileInput />} title={filtered ? "No posting requests match these filters" : "No posting requests"} /> : <TableScroll><Table>
      <THead><tr><TH>Source</TH><TH>Date</TH><TH>Description</TH><TH>Currency</TH><TH>Total</TH><TH>Status</TH><TH className="text-right">Open</TH></tr></THead>
      <TBody>{records.map((record) => <TR key={record.id}>
        <TD><p className="font-medium text-[var(--text-strong)]">{sourceLabel(record.source_module_key)}</p><p className="mt-1 max-w-48 truncate text-xs text-[var(--text-subtle)]">{record.source_record_type} · {record.source_event_key}</p></TD>
        <TD className="whitespace-nowrap">{formatDate(record.posting_date)}</TD><TD className="max-w-72"><p className="truncate">{record.description}</p>{record.reference ? <p className="mt-1 truncate text-xs text-[var(--text-subtle)]">{record.reference}</p> : null}</TD>
        <TD>{record.transaction_currency_code}</TD><TD className="whitespace-nowrap font-tabular font-semibold">{formatMinor(record.debit_minor, record.transaction_currency_minor_units, record.transaction_currency_code)}</TD><TD><StatusBadge status={record.status} /></TD>
        <TD className="text-right"><Button aria-label="Open posting request" onClick={() => void open(record.id)} size="icon-sm" variant="ghost"><Eye className="size-4" /></Button></TD>
      </TR>)}</TBody>
    </Table></TableScroll>}</TableWrap>
    <PostingRequestDetail canConvert={canConvert} canReject={canReject} onAction={beginAction} onClose={() => setDetail(null)} request={detail} />
    <PostingRequestAction action={action} currencies={currencies} onClose={() => { setAction(null); setDetail(action?.request ?? null); }} onDone={actionDone} />
  </div>;
}

function PostingRequestDetail({ canConvert, canReject, onAction, onClose, request }: { canConvert: boolean; canReject: boolean; onAction: (kind: Action["kind"], request: FinancePostingRequest) => void; onClose: () => void; request: FinancePostingRequest | null }) {
  return <DialogShell onClose={onClose} open={request !== null}><DialogHeader onClose={onClose} title="Posting request" />{request ? <><DialogBody className="space-y-5">
    <div className="flex flex-wrap items-center gap-3"><StatusBadge status={request.status} /><span className="font-tabular text-lg font-semibold text-[var(--text-strong)]">{formatMinor(request.debit_minor, request.transaction_currency_minor_units, request.transaction_currency_code)}</span></div>
    <div className="grid gap-3 sm:grid-cols-2"><Fact label="Source" value={`${sourceLabel(request.source_module_key)} · ${request.source_record_type}`} /><Fact label="Source event" value={request.source_event_key} /><Fact label="Posting date" value={formatDate(request.posting_date)} /><Fact label="Reference" value={request.reference || "—"} /></div>
    <p className="text-sm leading-6 text-[var(--text-muted)]">{request.description}</p>
    <TableWrap><TableScroll><Table><THead><tr><TH>#</TH><TH>Account</TH><TH>Description</TH><TH className="text-right">Debit</TH><TH className="text-right">Credit</TH></tr></THead><TBody>{request.lines.map((line) => <TR key={line.id}><TD>{line.line_number}</TD><TD><p className="font-medium">{line.account_code}</p><p className="mt-1 text-xs text-[var(--text-subtle)]">{line.account_name}</p></TD><TD>{line.description || "—"}</TD><TD className="text-right font-tabular">{minorValue(line.debit_minor, request.transaction_currency_minor_units)}</TD><TD className="text-right font-tabular">{minorValue(line.credit_minor, request.transaction_currency_minor_units)}</TD></TR>)}</TBody></Table></TableScroll></TableWrap>
    {request.journal_id ? <Fact label="Finance journal" value={request.journal_id} /> : null}{request.resolution_reason ? <Fact label="Resolution" value={request.resolution_reason} /> : null}
  </DialogBody>{request.status === "pending" && (canConvert || canReject) ? <DialogFooter className="justify-between"><div>{canReject ? <Button onClick={() => onAction("reject", request)} variant="ghost"><XCircle className="size-4" />Reject</Button> : null}</div>{canConvert ? <Button onClick={() => onAction("convert", request)}><ArrowRight className="size-4" />Create journal draft</Button> : null}</DialogFooter> : null}</> : null}</DialogShell>;
}

function PostingRequestAction({ action, currencies, onClose, onDone }: { action: Action | null; currencies: FinanceCurrency[]; onClose: () => void; onDone: (request: FinancePostingRequest) => void }) {
  const [reason, setReason] = useState("");
  const [values, setValues] = useState<Record<string, { amount: string; rate: string }>>({});
  const [saving, setSaving] = useState(false);
  const request = action?.request;
  const reporting = currencies.find((currency) => currency.is_reporting) ?? null;
  const sameCurrency = Boolean(request && reporting && request.transaction_currency_id === reporting.id);

  useEffect(() => {
    if (!action) return;
    setReason("");
    setValues(Object.fromEntries(action.request.lines.map((line) => [line.id, { amount: sameCurrency ? exactMinor(line.debit_minor || line.credit_minor, action.request.transaction_currency_minor_units) : "", rate: sameCurrency ? "" : "" }])));
  }, [action, sameCurrency]);

  const submit = async (event: React.FormEvent) => {
    event.preventDefault(); if (!action || !request || saving) return;
    setSaving(true);
    if (action.kind === "reject") {
      const response = await financeService.rejectPostingRequest(request.id, request.version, reason.trim());
      setSaving(false);
      if (!response.success || !response.data) { toast.error(responseMessage(response, "Posting request could not be rejected")); return; }
      toast.success("Posting request rejected"); onDone(response.data); return;
    }
    if (!reporting) { setSaving(false); toast.error("Set an active reporting currency first"); return; }
    const lines: PostingRequestConversionLine[] = [];
    for (const line of request.lines) {
      const value = values[line.id];
      const amount = parseMinor(value?.amount ?? "", reporting.minor_units);
      if (amount === null || amount <= 0) { setSaving(false); toast.error(`Enter a reporting amount for line ${line.line_number}`); return; }
      if (!sameCurrency && !value?.rate.trim()) { setSaving(false); toast.error(`Enter an exchange rate for line ${line.line_number}`); return; }
      lines.push({ line_id: line.id, reporting_debit_minor: line.debit_minor > 0 ? amount : 0, reporting_credit_minor: line.credit_minor > 0 ? amount : 0, exchange_rate: sameCurrency ? null : value.rate.trim() });
    }
    const response = await financeService.convertPostingRequest(request.id, request.version, lines);
    setSaving(false);
    if (!response.success || !response.data) { toast.error(responseMessage(response, "Journal draft could not be created")); return; }
    toast.success("Journal draft created"); onDone(response.data);
  };

  return <DialogShell onClose={onClose} open={action !== null}><DialogHeader onClose={onClose} title={action?.kind === "reject" ? "Reject posting request" : "Create journal draft"} />{action ? <form onSubmit={submit}><DialogBody className="space-y-5">
    {action.kind === "reject" ? <div><Label htmlFor="posting-reject-reason">Reason</Label><Textarea className="mt-1.5" data-autofocus="true" id="posting-reject-reason" maxLength={1000} onChange={(event) => setReason(event.target.value)} required rows={5} value={reason} /></div> : <>{!reporting ? <p className="border border-[var(--tone-danger-bd)] bg-[var(--tone-danger-bg)] p-4 text-sm text-[var(--tone-danger)]">An active reporting currency is required.</p> : null}<p className="text-sm leading-6 text-[var(--text-muted)]">Confirm each line in {reporting?.code ?? "the reporting currency"}. Finance will create a draft; normal journal review still applies.</p>{request?.lines.map((line) => <div className="border border-[var(--border)] p-4" key={line.id}><p className="font-medium text-[var(--text-strong)]">{line.line_number}. {line.account_code} · {line.account_name}</p><p className="mt-1 text-xs text-[var(--text-muted)]">Transaction amount {formatMinor(line.debit_minor || line.credit_minor, request.transaction_currency_minor_units, request.transaction_currency_code)}</p><div className={`mt-4 grid gap-4 ${sameCurrency ? "" : "sm:grid-cols-2"}`}><div><Label htmlFor={`posting-amount-${line.id}`}>Reporting amount</Label><Input className="mt-1.5" data-autofocus={line.line_number === 1 ? "true" : undefined} id={`posting-amount-${line.id}`} inputMode="decimal" onChange={(event) => setValues((current) => ({ ...current, [line.id]: { ...current[line.id], amount: event.target.value } }))} required value={values[line.id]?.amount ?? ""} /></div>{!sameCurrency ? <div><Label htmlFor={`posting-rate-${line.id}`}>Exchange rate</Label><Input className="mt-1.5" id={`posting-rate-${line.id}`} inputMode="decimal" onChange={(event) => setValues((current) => ({ ...current, [line.id]: { ...current[line.id], rate: event.target.value } }))} placeholder="1.000000" required value={values[line.id]?.rate ?? ""} /></div> : null}</div></div>)}</>}
  </DialogBody><DialogFooter><Button disabled={saving || (action.kind === "convert" && !reporting)} type="submit">{saving ? <Loader2 className="size-4 animate-spin" /> : null}{action.kind === "reject" ? "Reject request" : "Create draft"}</Button></DialogFooter></form> : null}</DialogShell>;
}

function StatusBadge({ status }: { status: FinancePostingRequestSummary["status"] }) { return <Badge tone={status === "converted" ? "success" : status === "pending" ? "warning" : status === "rejected" ? "danger" : "neutral"}>{status}</Badge>; }
function Fact({ label, value }: { label: string; value: string }) { return <div className="border border-[var(--border)] bg-[var(--surface-muted)] p-3"><p className="text-[11px] font-semibold uppercase tracking-[0.12em] text-[var(--text-subtle)]">{label}</p><p className="mt-1 break-words text-sm text-[var(--text-strong)]">{value}</p></div>; }
function sourceLabel(value: string) { return value.split("_").map((part) => part.charAt(0).toUpperCase() + part.slice(1)).join(" "); }
function formatDate(value: string) { return new Intl.DateTimeFormat(undefined, { day: "2-digit", month: "short", year: "numeric", timeZone: "UTC" }).format(new Date(`${value}T00:00:00Z`)); }
function formatMinor(amount: number, minorUnits: number, currency: string) { return new Intl.NumberFormat(undefined, { style: "currency", currency, minimumFractionDigits: minorUnits, maximumFractionDigits: minorUnits }).format(amount / (10 ** minorUnits)); }
function minorValue(amount: number, minorUnits: number) { return amount ? (amount / (10 ** minorUnits)).toFixed(minorUnits) : "—"; }
function exactMinor(amount: number, minorUnits: number) { return (amount / (10 ** minorUnits)).toFixed(minorUnits); }
function parseMinor(value: string, minorUnits: number) { if (!/^\d+(?:\.\d+)?$/.test(value.trim())) return null; const [whole, fraction = ""] = value.trim().split("."); if (fraction.length > minorUnits) return null; const result = Number(whole) * (10 ** minorUnits) + Number(fraction.padEnd(minorUnits, "0")); return Number.isSafeInteger(result) ? result : null; }

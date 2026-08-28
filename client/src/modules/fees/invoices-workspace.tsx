import { useCallback, useEffect, useMemo, useState } from "react";
import { Eye, FileText, Loader2, Plus, Search, Send, Trash2 } from "lucide-react";
import toast from "react-hot-toast";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { ConfirmDrawer } from "@/components/ui/confirm-drawer";
import { Table, TableControlsBar, TableControlsPagination, TableControlsSearch, TableEmpty, TableError, TableLoading, TableScroll, TableWrap, TBody, TD, TH, THead, TR } from "@/components/ui/data-table";
import { DialogBody, DialogFooter, DialogHeader, DialogShell } from "@/components/ui/dialog";
import { Input, Label, Select, Textarea } from "@/components/ui/input";
import { usePageChrome } from "@/modules/admin/layouts/page-chrome";
import { hasPermission } from "@/modules/users/access-control";
import { useAuthStore } from "@/stores/auth-store";

import { feesService, responseMessage } from "./service";
import type { BillingAccount, FeeStructure, FeesReferenceData, Invoice, InvoiceInput, InvoiceSummary } from "./types";

export function InvoicesWorkspace() {
  const permissions = useAuthStore((state) => state.user?.permissions);
  const canCreate = hasPermission(permissions, "fees:create");
  const canEdit = hasPermission(permissions, "fees:edit");
  const canDelete = hasPermission(permissions, "fees:delete");
  const [records, setRecords] = useState<InvoiceSummary[]>([]);
  const [references, setReferences] = useState<FeesReferenceData | null>(null);
  const [billingAccounts, setBillingAccounts] = useState<BillingAccount[]>([]);
  const [structures, setStructures] = useState<FeeStructure[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [page, setPage] = useState(1);
  const [totalPages, setTotalPages] = useState(1);
  const [search, setSearch] = useState("");
  const [submittedSearch, setSubmittedSearch] = useState("");
  const [status, setStatus] = useState("all");
  const [createOpen, setCreateOpen] = useState(false);
  const [detail, setDetail] = useState<Invoice | null>(null);
  const [issueRecord, setIssueRecord] = useState<Invoice | null>(null);
  const [deleteRecord, setDeleteRecord] = useState<Invoice | null>(null);
  const [pending, setPending] = useState(false);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const [invoiceResponse, referenceResponse, billingResponse, structureResponse] = await Promise.all([
        feesService.listInvoices({ page, per_page: 25, search: submittedSearch || undefined, status: status === "all" ? undefined : status }),
        feesService.referenceData(),
        feesService.listBillingAccounts({ page: 1, per_page: 100, status: "active" }),
        feesService.listFeeStructures({ page: 1, per_page: 100, status: "active" }),
      ]);
      if (!invoiceResponse.success || !invoiceResponse.data) throw new Error(responseMessage(invoiceResponse, "Invoices could not be loaded"));
      if (!referenceResponse.success || !referenceResponse.data) throw new Error(responseMessage(referenceResponse, "Fees reference data could not be loaded"));
      if (!billingResponse.success || !billingResponse.data) throw new Error(responseMessage(billingResponse, "Billing accounts could not be loaded"));
      if (!structureResponse.success || !structureResponse.data) throw new Error(responseMessage(structureResponse, "Fee structures could not be loaded"));
      setRecords(invoiceResponse.data.invoices);
      setTotalPages(invoiceResponse.pagination?.total_pages ?? 1);
      setReferences(referenceResponse.data);
      setBillingAccounts(billingResponse.data.billing_accounts);
      setStructures(structureResponse.data.fee_structures);
    } catch (loadError) {
      setError(loadError instanceof Error ? loadError.message : "Invoices could not be loaded");
    } finally {
      setLoading(false);
    }
  }, [page, status, submittedSearch]);

  useEffect(() => { void load(); }, [load]);

  const openInvoice = async (id: string) => {
    const response = await feesService.readInvoice(id);
    if (!response.success || !response.data) { toast.error(responseMessage(response, "Invoice could not be loaded")); return; }
    setDetail(response.data);
  };

  const issue = async () => {
    if (!issueRecord || pending) return;
    setPending(true);
    const response = await feesService.issueInvoice(issueRecord.id, issueRecord.version);
    setPending(false);
    if (!response.success || !response.data) { toast.error(responseMessage(response, "Invoice could not be issued")); return; }
    toast.success("Invoice issued");
    setIssueRecord(null);
    setDetail(response.data);
    void load();
  };

  const remove = async () => {
    if (!deleteRecord || pending) return;
    setPending(true);
    const response = await feesService.deleteInvoice(deleteRecord.id, deleteRecord.version);
    setPending(false);
    if (!response.success) { toast.error(responseMessage(response, "Invoice could not be removed")); return; }
    toast.success("Invoice removed");
    setDeleteRecord(null);
    setDetail(null);
    void load();
  };

  const canStart = billingAccounts.length > 0 && structures.length > 0 && Boolean(references?.academic_years.length);
  usePageChrome("Invoices", canCreate ? <Button disabled={!canStart} onClick={() => setCreateOpen(true)}><Plus className="size-4" />New invoice</Button> : undefined);
  const filtered = Boolean(submittedSearch || status !== "all");

  return <div className="space-y-6">
    <p className="text-sm text-[var(--text-muted)]">Create learner invoices and send issued amounts to Finance for review.</p>
    {!loading && canCreate && !canStart ? <section className="border border-[var(--tone-warn-bd)] bg-[var(--tone-warn-bg)] p-4 text-sm leading-6 text-[var(--text-body)]">An active billing account, active fee structure, and academic year are required before an invoice can be created.</section> : null}
    <TableControlsBar>
      <TableControlsSearch onSubmit={(event) => { event.preventDefault(); setPage(1); setSubmittedSearch(search.trim()); }}>
        <Input aria-label="Search invoices" leadingIcon={<Search />} onChange={(event) => setSearch(event.target.value)} placeholder="Search invoice, learner, or account…" value={search} />
        <Button type="submit" variant="secondary">Search</Button>
      </TableControlsSearch>
      <Select aria-label="Invoice status" className="sm:w-40" onChange={(event) => { setPage(1); setStatus(event.target.value); }} value={status}><option value="all">All statuses</option><option value="draft">Draft</option><option value="issued">Issued</option></Select>
      {!loading && records.length > 0 ? <TableControlsPagination onNext={() => setPage((value) => Math.min(totalPages, value + 1))} onPrevious={() => setPage((value) => Math.max(1, value - 1))} page={page} totalPages={totalPages} /> : null}
    </TableControlsBar>
    <TableWrap>
      {loading ? <TableLoading columns={7} label="Loading invoices…" /> : error ? <TableError description={error} onRetry={() => void load()} /> : records.length === 0 ? <TableEmpty description={filtered ? "Change the current filters." : canCreate ? "Create the first learner invoice." : "No invoices are available."} icon={<FileText />} title={filtered ? "No invoices match these filters" : "No invoices"} /> : <TableScroll><Table>
        <THead><tr><TH>Invoice</TH><TH>Learner</TH><TH>Date</TH><TH>Due</TH><TH>Total</TH><TH>Status</TH><TH className="text-right">Open</TH></tr></THead>
        <TBody>{records.map((record) => <TR key={record.id}>
          <TD><p className="font-tabular font-semibold text-[var(--text-strong)]">{record.invoice_number}</p><p className="mt-1 text-xs text-[var(--text-subtle)]">{record.line_count} {record.line_count === 1 ? "line" : "lines"}</p></TD>
          <TD><p className="font-medium text-[var(--text-strong)]">{record.learner_name}</p><p className="mt-1 text-xs text-[var(--text-subtle)]">{record.learner_number} · {record.billing_account_number}</p></TD>
          <TD className="whitespace-nowrap">{formatDate(record.invoice_date)}</TD><TD className="whitespace-nowrap">{formatDate(record.due_date)}</TD>
          <TD className="whitespace-nowrap font-tabular font-semibold">{formatMinor(record.total_minor, record.currency_minor_units, record.currency_code)}</TD>
          <TD><InvoiceStatus record={record} /></TD>
          <TD className="text-right"><Button aria-label={`Open ${record.invoice_number}`} onClick={() => void openInvoice(record.id)} size="icon-sm" variant="ghost"><Eye className="size-4" /></Button></TD>
        </TR>)}</TBody>
      </Table></TableScroll>}
    </TableWrap>
    <CreateInvoiceDrawer billingAccounts={billingAccounts} onClose={() => setCreateOpen(false)} onSaved={(invoice) => { setCreateOpen(false); setDetail(invoice); void load(); }} open={createOpen} references={references} structures={structures} />
    <InvoiceDetailDrawer canDelete={canDelete} canEdit={canEdit} invoice={detail} onClose={() => setDetail(null)} onDelete={(invoice) => setDeleteRecord(invoice)} onIssue={(invoice) => setIssueRecord(invoice)} />
    <ConfirmDrawer confirmLabel="Issue invoice" description={`Issue ${issueRecord?.invoice_number ?? "this invoice"}? Its lines become immutable and a balanced posting request is sent to Finance.`} isPending={pending} onClose={() => setIssueRecord(null)} onConfirm={() => void issue()} open={issueRecord !== null} title="Issue invoice?" />
    <ConfirmDrawer confirmLabel="Remove invoice" description={`Remove ${deleteRecord?.invoice_number ?? "this draft invoice"}?`} isPending={pending} onClose={() => setDeleteRecord(null)} onConfirm={() => void remove()} open={deleteRecord !== null} title="Remove draft invoice?" />
  </div>;
}

function CreateInvoiceDrawer({ billingAccounts, onClose, onSaved, open, references, structures }: { billingAccounts: BillingAccount[]; onClose: () => void; onSaved: (invoice: Invoice) => void; open: boolean; references: FeesReferenceData | null; structures: FeeStructure[] }) {
  const today = new Date().toISOString().slice(0, 10);
  const [billingAccountId, setBillingAccountId] = useState("");
  const [academicYearId, setAcademicYearId] = useState("");
  const [academicTermId, setAcademicTermId] = useState("");
  const [invoiceDate, setInvoiceDate] = useState(today);
  const [dueDate, setDueDate] = useState(today);
  const [description, setDescription] = useState("");
  const [reference, setReference] = useState("");
  const [selectedIds, setSelectedIds] = useState<string[]>([]);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    if (!open) return;
    const date = new Date(); date.setDate(date.getDate() + 30);
    setBillingAccountId(billingAccounts[0]?.id ?? "");
    setAcademicYearId(references?.academic_years[0]?.id ?? "");
    setAcademicTermId(""); setInvoiceDate(today); setDueDate(date.toISOString().slice(0, 10));
    setDescription(""); setReference(""); setSelectedIds([]);
  }, [billingAccounts, open, references, today]);

  const terms = references?.academic_terms.filter((term) => term.academic_year_id === academicYearId) ?? [];
  const eligible = structures.filter((structure) => structure.academic_year_id === academicYearId && (!structure.academic_term_id || structure.academic_term_id === academicTermId));
  const selected = structures.filter((structure) => selectedIds.includes(structure.id));
  const selectedCurrency = selected[0]?.currency_id;

  const toggle = (structure: FeeStructure) => setSelectedIds((current) => current.includes(structure.id) ? current.filter((id) => id !== structure.id) : [...current, structure.id]);
  const submit = async (event: React.FormEvent) => {
    event.preventDefault();
    if (selectedIds.length === 0) { toast.error("Choose at least one fee structure"); return; }
    const payload: InvoiceInput = {
      billing_account_id: billingAccountId,
      academic_year_id: academicYearId,
      academic_term_id: academicTermId || null,
      invoice_date: invoiceDate,
      due_date: dueDate,
      description: description.trim() || null,
      reference: reference.trim() || null,
      fee_structure_ids: selectedIds,
    };
    setSaving(true);
    const response = await feesService.createInvoice({ ...payload, idempotency_key: crypto.randomUUID() });
    setSaving(false);
    if (!response.success || !response.data) { toast.error(responseMessage(response, "Invoice could not be created")); return; }
    toast.success("Invoice created"); onSaved(response.data);
  };

  return <DialogShell onClose={onClose} open={open}>
    <DialogHeader onClose={onClose} title="New invoice" />
    <form onSubmit={submit}>
      <DialogBody className="space-y-5">
        <div><Label htmlFor="invoice-billing">Billing account</Label><Select className="mt-1.5" data-autofocus="true" id="invoice-billing" onChange={(event) => setBillingAccountId(event.target.value)} required value={billingAccountId}>{billingAccounts.map((account) => <option key={account.id} value={account.id}>{account.learner_name} · {account.account_number}</option>)}</Select></div>
        <div className="grid gap-5 sm:grid-cols-2"><div><Label htmlFor="invoice-year">Academic year</Label><Select className="mt-1.5" id="invoice-year" onChange={(event) => { setAcademicYearId(event.target.value); setAcademicTermId(""); setSelectedIds([]); }} required value={academicYearId}>{references?.academic_years.map((year) => <option key={year.id} value={year.id}>{year.name}</option>)}</Select></div><div><Label htmlFor="invoice-term">Term</Label><Select className="mt-1.5" id="invoice-term" onChange={(event) => { setAcademicTermId(event.target.value); setSelectedIds([]); }} value={academicTermId}><option value="">Whole year</option>{terms.map((term) => <option key={term.id} value={term.id}>{term.name}</option>)}</Select></div></div>
        <div className="grid gap-5 sm:grid-cols-2"><div><Label htmlFor="invoice-date">Invoice date</Label><Input className="mt-1.5" id="invoice-date" onChange={(event) => setInvoiceDate(event.target.value)} required type="date" value={invoiceDate} /></div><div><Label htmlFor="invoice-due">Due date</Label><Input className="mt-1.5" id="invoice-due" min={invoiceDate} onChange={(event) => setDueDate(event.target.value)} required type="date" value={dueDate} /></div></div>
        <div><Label>Fee structures</Label><div className="mt-2 space-y-2">{eligible.length === 0 ? <p className="border border-[var(--border)] bg-[var(--surface-muted)] p-4 text-sm text-[var(--text-muted)]">No active fee structures match this academic scope.</p> : eligible.map((structure) => {
          const currency = references?.currencies.find((item) => item.id === structure.currency_id);
          const disabled = Boolean(selectedCurrency && selectedCurrency !== structure.currency_id && !selectedIds.includes(structure.id));
          return <label className={`flex items-start gap-3 border p-3 ${disabled ? "opacity-50" : "cursor-pointer"}`} key={structure.id}><input checked={selectedIds.includes(structure.id)} className="mt-1 size-4" disabled={disabled} onChange={() => toggle(structure)} type="checkbox" /><span className="min-w-0 flex-1"><span className="block font-medium text-[var(--text-strong)]">{structure.code} · {structure.name}</span><span className="mt-1 block text-xs text-[var(--text-muted)]">{currency ? formatMinor(structure.amount_minor, currency.minor_units, currency.code) : structure.amount_minor}</span></span></label>;
        })}</div></div>
        <div><Label htmlFor="invoice-reference">Reference</Label><Input className="mt-1.5" id="invoice-reference" maxLength={160} onChange={(event) => setReference(event.target.value)} value={reference} /></div>
        <div><Label htmlFor="invoice-description">Description</Label><Textarea className="mt-1.5" id="invoice-description" maxLength={1000} onChange={(event) => setDescription(event.target.value)} rows={3} value={description} /></div>
      </DialogBody>
      <DialogFooter><Button disabled={saving} type="submit">{saving ? <Loader2 className="size-4 animate-spin" /> : null}Create draft</Button></DialogFooter>
    </form>
  </DialogShell>;
}

function InvoiceDetailDrawer({ canDelete, canEdit, invoice, onClose, onDelete, onIssue }: { canDelete: boolean; canEdit: boolean; invoice: Invoice | null; onClose: () => void; onDelete: (invoice: Invoice) => void; onIssue: (invoice: Invoice) => void }) {
  return <DialogShell onClose={onClose} open={invoice !== null}>
    <DialogHeader onClose={onClose} title={invoice?.invoice_number ?? "Invoice"} />
    {invoice ? <><DialogBody className="space-y-5">
      <div className="flex flex-wrap items-center gap-3"><InvoiceStatus record={invoice} /><span className="font-tabular text-lg font-semibold text-[var(--text-strong)]">{formatMinor(invoice.total_minor, invoice.currency_minor_units, invoice.currency_code)}</span></div>
      <div className="grid gap-3 sm:grid-cols-2"><Fact label="Learner" value={`${invoice.learner_name} · ${invoice.learner_number}`} /><Fact label="Billing account" value={invoice.billing_account_number} /><Fact label="Invoice date" value={formatDate(invoice.invoice_date)} /><Fact label="Due date" value={formatDate(invoice.due_date)} /><Fact label="Academic scope" value={`${invoice.academic_year_name}${invoice.academic_term_name ? ` · ${invoice.academic_term_name}` : ""}`} /><Fact label="Reference" value={invoice.reference || "—"} /></div>
      {invoice.description ? <p className="text-sm leading-6 text-[var(--text-muted)]">{invoice.description}</p> : null}
      <TableWrap><TableScroll><Table><THead><tr><TH>Fee</TH><TH>Description</TH><TH className="text-right">Amount</TH></tr></THead><TBody>{invoice.lines.map((line) => <TR key={line.id}><TD className="font-tabular font-semibold">{line.fee_code}</TD><TD>{line.description}</TD><TD className="text-right font-tabular">{formatMinor(line.amount_minor, invoice.currency_minor_units, invoice.currency_code)}</TD></TR>)}</TBody></Table></TableScroll></TableWrap>
      {invoice.posting_request_id ? <Fact label="Finance posting request" value={`${invoice.posting_request_status ?? "pending"} · ${invoice.posting_request_id}`} /> : null}
    </DialogBody><DialogFooter className="justify-between"><div>{canDelete && invoice.status === "draft" ? <Button onClick={() => onDelete(invoice)} variant="ghost"><Trash2 className="size-4" />Remove</Button> : null}</div>{canEdit && invoice.status === "draft" ? <Button onClick={() => onIssue(invoice)}><Send className="size-4" />Issue invoice</Button> : null}</DialogFooter></> : null}
  </DialogShell>;
}

function InvoiceStatus({ record }: { record: InvoiceSummary }) {
  return <div className="flex flex-wrap gap-2"><Badge tone={record.status === "issued" ? "success" : "warning"}>{record.status}</Badge>{record.posting_request_status ? <Badge tone={record.posting_request_status === "converted" ? "success" : record.posting_request_status === "rejected" ? "danger" : "neutral"}>Finance {record.posting_request_status}</Badge> : null}</div>;
}

function Fact({ label, value }: { label: string; value: string }) { return <div className="border border-[var(--border)] bg-[var(--surface-muted)] p-3"><p className="text-[11px] font-semibold uppercase tracking-[0.12em] text-[var(--text-subtle)]">{label}</p><p className="mt-1 break-words text-sm text-[var(--text-strong)]">{value}</p></div>; }
function formatDate(value: string) { return new Intl.DateTimeFormat(undefined, { day: "2-digit", month: "short", year: "numeric", timeZone: "UTC" }).format(new Date(`${value}T00:00:00Z`)); }
function formatMinor(amount: number, minorUnits: number, currency: string) { return new Intl.NumberFormat(undefined, { style: "currency", currency, minimumFractionDigits: minorUnits, maximumFractionDigits: minorUnits }).format(amount / (10 ** minorUnits)); }

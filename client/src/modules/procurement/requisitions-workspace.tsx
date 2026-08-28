/**
 * Requisition list, direct-load detail, and version-safe lifecycle workflows.
 * Amounts stay exact as integer minor units at the API boundary.
 */

import { useCallback, useEffect, useMemo, useState } from "react";
import type { FormEvent, ReactNode } from "react";
import { Link, useNavigate } from "@tanstack/react-router";
import {
  ArrowLeft, CheckCircle2, ClipboardCheck, Eye, FileText, Loader2, Plus,
  Search, Send, Trash2, XCircle,
} from "lucide-react";
import toast from "react-hot-toast";

import { SearchableSelect } from "@/components/searchable-select";
import { Badge } from "@/components/ui/badge";
import { Button, buttonVariants } from "@/components/ui/button";
import { ConfirmDrawer } from "@/components/ui/confirm-drawer";
import {
  Table, TableControlsBar, TableControlsPagination, TableControlsSearch, TableEmpty,
  TableError, TableLoading, TableScroll, TableWrap, TBody, TD, TH, THead, TR,
} from "@/components/ui/data-table";
import { DialogBody, DialogFooter, DialogHeader, DialogShell } from "@/components/ui/dialog";
import { Input, Label, Select, Textarea } from "@/components/ui/input";
import { usePageChrome } from "@/modules/admin/layouts/page-chrome";
import { hasPermission } from "@/modules/users/access-control";
import { useAuthStore } from "@/stores/auth-store";

import { procurementService, responseMessage } from "./service";
import type {
  ProcurementCurrency, ProcurementReferenceData, Requisition, RequisitionInput,
  RequisitionLine, RequisitionLineInput, RequisitionStatus, RequisitionSummary,
  RequesterCandidate, Supplier,
} from "./types";

type DrawerState = { mode: "create"; requisition: null } | { mode: "edit"; requisition: Requisition };
type LifecycleKind = "submit" | "approve" | "reject" | "cancel";
type LifecycleAction = { kind: LifecycleKind; requisition: Requisition };

export function RequisitionsWorkspace() {
  const permissions = useAuthStore((state) => state.user?.permissions);
  const canCreate = hasPermission(permissions, "procurement:create");
  const [requisitions, setRequisitions] = useState<RequisitionSummary[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [page, setPage] = useState(1);
  const [totalPages, setTotalPages] = useState(1);
  const [search, setSearch] = useState("");
  const [submittedSearch, setSubmittedSearch] = useState("");
  const [status, setStatus] = useState("all");
  const [drawer, setDrawer] = useState<DrawerState | null>(null);
  const references = useProcurementReferences(drawer !== null);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const response = await procurementService.listRequisitions({
        page,
        per_page: 20,
        search: submittedSearch || undefined,
        status: status === "all" ? undefined : status,
      });
      if (!response.success || !response.data) throw new Error(responseMessage(response, "Requisitions could not be loaded"));
      setRequisitions(response.data.requisitions);
      setTotalPages(response.pagination?.total_pages ?? 1);
    } catch (loadError) {
      setError(loadError instanceof Error ? loadError.message : "Requisitions could not be loaded");
    } finally {
      setLoading(false);
    }
  }, [page, status, submittedSearch]);

  useEffect(() => { void load(); }, [load]);
  usePageChrome("Requisitions", canCreate ? <Button onClick={() => setDrawer({ mode: "create", requisition: null })}><Plus className="size-4" />New requisition</Button> : null);
  const filtered = Boolean(submittedSearch) || status !== "all";

  return <div className="space-y-5">
    <p className="text-sm text-[var(--text-muted)]">Prepare and approve purchasing requests before ordering.</p>
    <TableControlsBar>
      <TableControlsSearch onSubmit={(event) => { event.preventDefault(); setPage(1); setSubmittedSearch(search.trim()); }}>
        <Input aria-label="Search requisitions" leadingIcon={<Search />} onChange={(event) => setSearch(event.target.value)} placeholder="Search number, title, or requester…" value={search} />
        <Button type="submit" variant="secondary">Search</Button>
      </TableControlsSearch>
      <Select aria-label="Requisition status" className="sm:w-44" onChange={(event) => { setPage(1); setStatus(event.target.value); }} value={status}>
        <option value="all">All statuses</option>
        <option value="draft">Draft</option>
        <option value="submitted">Submitted</option>
        <option value="approved">Approved</option>
        <option value="rejected">Rejected</option>
        <option value="cancelled">Cancelled</option>
      </Select>
      {!loading && requisitions.length > 0 ? <TableControlsPagination onNext={() => setPage((value) => Math.min(totalPages, value + 1))} onPrevious={() => setPage((value) => Math.max(1, value - 1))} page={page} totalPages={totalPages} /> : null}
    </TableControlsBar>

    <TableWrap>
      {loading ? <TableLoading columns={7} label="Loading requisitions…" /> : error ? <TableError description={error} onRetry={() => void load()} /> : requisitions.length === 0 ? <TableEmpty description={filtered ? "Change the current filters." : "Create a draft purchasing request to begin."} icon={<FileText />} title={filtered ? "No requisitions match these filters" : "No requisitions yet"} /> : <TableScroll><Table>
        <THead><tr><TH>Requisition</TH><TH>Requester</TH><TH>Needed by</TH><TH>Lines</TH><TH>Estimate</TH><TH>Status</TH><TH className="text-right">Open</TH></tr></THead>
        <TBody>{requisitions.map((requisition) => <TR key={requisition.id}>
          <TD><p className="font-tabular font-semibold text-[var(--text-strong)]">{requisition.requisition_number}</p><p className="mt-1 max-w-72 truncate text-xs text-[var(--text-muted)]">{requisition.title}</p></TD>
          <TD><p className="text-[var(--text-strong)]">{requisition.requester_name}</p><p className="mt-1 font-tabular text-xs text-[var(--text-subtle)]">{requisition.requester_employee_number}</p></TD>
          <TD className="whitespace-nowrap text-[var(--text-muted)]">{requisition.needed_by ? formatDate(requisition.needed_by) : "—"}</TD>
          <TD className="font-tabular text-[var(--text-muted)]">{requisition.line_count}</TD>
          <TD className="whitespace-nowrap font-tabular font-semibold text-[var(--text-strong)]">{formatMinor(requisition.total_minor, requisition.currency_minor_units, requisition.currency_code)}</TD>
          <TD><RequisitionStatusBadge status={requisition.status} /></TD>
          <TD className="text-right"><Link aria-label={`Open ${requisition.requisition_number}`} className={buttonVariants({ variant: "ghost", size: "icon-sm" })} params={{ requisitionId: requisition.id }} to="/modules/procurement/requisitions/$requisitionId"><Eye className="size-4" /></Link></TD>
        </TR>)}</TBody>
      </Table></TableScroll>}
    </TableWrap>

    <RequisitionDrawer drawer={drawer} onClose={() => setDrawer(null)} onSaved={() => { setDrawer(null); void load(); }} references={references} />
  </div>;
}

export function RequisitionDetail({ requisitionId }: { requisitionId: string }) {
  const navigate = useNavigate();
  const permissions = useAuthStore((state) => state.user?.permissions);
  const userId = useAuthStore((state) => state.user?.id);
  const canEdit = hasPermission(permissions, "procurement:edit");
  const canDelete = hasPermission(permissions, "procurement:delete");
  const [requisition, setRequisition] = useState<Requisition | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [editOpen, setEditOpen] = useState(false);
  const [action, setAction] = useState<LifecycleAction | null>(null);
  const [deleteOpen, setDeleteOpen] = useState(false);
  const [deleting, setDeleting] = useState(false);
  const references = useProcurementReferences(editOpen);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const response = await procurementService.readRequisition(requisitionId);
      if (!response.success || !response.data) throw new Error(responseMessage(response, "Requisition could not be loaded"));
      setRequisition(response.data);
    } catch (loadError) {
      setError(loadError instanceof Error ? loadError.message : "Requisition could not be loaded");
    } finally {
      setLoading(false);
    }
  }, [requisitionId]);

  useEffect(() => { void load(); }, [load]);
  usePageChrome(requisition?.requisition_number ?? "Requisition");

  const remove = async () => {
    if (!requisition || deleting) return;
    setDeleting(true);
    try {
      const response = await procurementService.deleteRequisition(requisition.id, requisition.version);
      if (!response.success) throw new Error(responseMessage(response, "Requisition could not be removed"));
      toast.success("Requisition removed");
      await navigate({ to: "/modules/procurement/requisitions", replace: true });
    } catch (deleteError) {
      toast.error(deleteError instanceof Error ? deleteError.message : "Requisition could not be removed");
      setDeleting(false);
    }
  };

  if (loading) return <div className="space-y-4" aria-label="Loading requisition" role="status"><div className="h-36 animate-pulse rounded-[var(--radius-xl)] bg-[var(--surface-sunken)]" /><div className="h-72 animate-pulse rounded-[var(--radius-xl)] bg-[var(--surface-sunken)]" /></div>;
  if (error || !requisition) return <TableWrap><TableError description={error ?? "Requisition was not found"} onRetry={() => void load()} title="Requisition could not be opened" /></TableWrap>;

  const requester = requisition.created_by === userId || requisition.requester_account_id === userId;
  const mayDecide = canEdit && requisition.status === "submitted" && !requester;
  return <div className="space-y-6">
    <section className="rounded-[var(--radius-xl)] border border-[var(--border)] bg-[var(--surface)] p-5 shadow-[var(--shadow-card)] sm:p-6">
      <div className="flex flex-col gap-5 lg:flex-row lg:items-start lg:justify-between">
        <div className="min-w-0">
          <Link className={buttonVariants({ variant: "ghost", size: "sm" })} to="/modules/procurement/requisitions"><ArrowLeft className="size-4" />Back to requisitions</Link>
          <div className="mt-4 flex flex-wrap items-center gap-3"><h1 className="font-tabular text-xl font-semibold text-[var(--text-strong)]">{requisition.requisition_number}</h1><RequisitionStatusBadge status={requisition.status} /></div>
          <p className="mt-2 text-base font-medium text-[var(--text-body)]">{requisition.title}</p>
          {requisition.purpose ? <p className="mt-2 max-w-3xl text-sm leading-6 text-[var(--text-muted)]">{requisition.purpose}</p> : null}
        </div>
        <div className="flex flex-wrap gap-2">
          {canEdit && requisition.status === "draft" ? <Button onClick={() => setEditOpen(true)} variant="secondary">Edit</Button> : null}
          {canDelete && requisition.status === "draft" ? <Button onClick={() => setDeleteOpen(true)} variant="ghost"><Trash2 className="size-4" />Remove</Button> : null}
          {canEdit && requisition.status === "draft" ? <Button onClick={() => setAction({ kind: "submit", requisition })}><Send className="size-4" />Submit</Button> : null}
          {mayDecide ? <><Button onClick={() => setAction({ kind: "reject", requisition })} variant="secondary"><XCircle className="size-4" />Reject</Button><Button onClick={() => setAction({ kind: "approve", requisition })}><ClipboardCheck className="size-4" />Approve</Button></> : null}
          {canEdit && requisition.status === "submitted" && requester ? <Button onClick={() => setAction({ kind: "cancel", requisition })} variant="secondary"><XCircle className="size-4" />Cancel request</Button> : null}
        </div>
      </div>
    </section>

    {requisition.status === "submitted" && requester ? <Notice icon={<ClipboardCheck />} text="Another Procurement operator must approve or reject this request." /> : null}
    {requisition.decision_note ? <Notice danger={requisition.status === "rejected"} icon={requisition.status === "approved" ? <CheckCircle2 /> : <XCircle />} text={`${requisition.status === "approved" ? "Decision" : "Rejected"}: ${requisition.decision_note}`} /> : null}
    {requisition.cancellation_note ? <Notice icon={<XCircle />} text={`Cancelled: ${requisition.cancellation_note}`} /> : null}

    <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
      <Fact label="Requester" value={`${requisition.requester_name} · ${requisition.requester_employee_number}`} />
      <Fact label="Needed by" value={requisition.needed_by ? formatDate(requisition.needed_by) : "Not set"} />
      <Fact label="Currency" value={requisition.currency_code} />
      <Fact label="Estimated total" value={formatMinor(requisition.total_minor, requisition.currency_minor_units, requisition.currency_code)} />
    </div>

    <TableWrap><TableScroll><Table>
      <THead><tr><TH>#</TH><TH>Item</TH><TH>Preferred supplier</TH><TH className="text-right">Quantity</TH><TH className="text-right">Unit estimate</TH><TH className="text-right">Line estimate</TH></tr></THead>
      <TBody>{requisition.lines.map((line) => <TR key={line.id}><TD className="font-tabular text-[var(--text-subtle)]">{line.line_number}</TD><TD className="min-w-64 font-medium text-[var(--text-strong)]">{line.description}</TD><TD>{line.preferred_supplier_name ? <><p className="text-[var(--text-body)]">{line.preferred_supplier_name}</p><p className="mt-1 font-tabular text-xs text-[var(--text-subtle)]">{line.preferred_supplier_number}</p></> : "—"}</TD><TD className="whitespace-nowrap text-right font-tabular">{line.quantity} {line.unit_label || "units"}</TD><TD className="whitespace-nowrap text-right font-tabular">{formatMinor(line.estimated_unit_amount_minor, requisition.currency_minor_units, requisition.currency_code)}</TD><TD className="whitespace-nowrap text-right font-tabular font-semibold">{formatMinor(line.estimated_line_amount_minor, requisition.currency_minor_units, requisition.currency_code)}</TD></TR>)}</TBody>
      <tfoot className="border-t border-[var(--border)] bg-[var(--surface-muted)]"><tr><TD className="text-right font-semibold text-[var(--text-strong)]" colSpan={5}>Estimated total</TD><TD className="whitespace-nowrap text-right font-tabular font-semibold text-[var(--text-strong)]">{formatMinor(requisition.total_minor, requisition.currency_minor_units, requisition.currency_code)}</TD></tr></tfoot>
    </Table></TableScroll></TableWrap>

    <RequisitionDrawer drawer={editOpen ? { mode: "edit", requisition } : null} onClose={() => setEditOpen(false)} onSaved={() => { setEditOpen(false); void load(); }} references={references} />
    <LifecycleDrawer action={action} onClose={() => setAction(null)} onDone={() => { setAction(null); void load(); }} />
    <ConfirmDrawer confirmLabel="Remove requisition" description={`Remove ${requisition.requisition_number}? Only this draft and its lines will be removed.`} isPending={deleting} onClose={() => setDeleteOpen(false)} onConfirm={() => void remove()} open={deleteOpen} title="Remove draft requisition?" />
  </div>;
}

function RequisitionDrawer({ drawer, onClose, onSaved, references }: { drawer: DrawerState | null; onClose: () => void; onSaved: () => void; references: ReferenceState }) {
  const requisition = drawer?.requisition ?? null;
  const [requesterId, setRequesterId] = useState<string | null>(null);
  const [currencyId, setCurrencyId] = useState("");
  const [title, setTitle] = useState("");
  const [purpose, setPurpose] = useState("");
  const [neededBy, setNeededBy] = useState("");
  const [lines, setLines] = useState<EditableLine[]>([emptyLine()]);
  const [saving, setSaving] = useState(false);
  const selectedCurrency = references.data?.currencies.find((item) => item.id === currencyId) ?? null;

  useEffect(() => {
    if (!drawer) return;
    setRequesterId(requisition?.requester_employee_id ?? null);
    setCurrencyId(requisition?.currency_id ?? "");
    setTitle(requisition?.title ?? "");
    setPurpose(requisition?.purpose ?? "");
    setNeededBy(requisition?.needed_by ?? "");
    setLines(requisition ? requisition.lines.map((line) => editableLine(line, requisition.currency_minor_units)) : [emptyLine()]);
  }, [drawer, requisition]);

  useEffect(() => {
    if (!drawer || currencyId || !references.data) return;
    const fallbackCurrency = references.data.currencies.find((item) => item.is_reporting) ?? references.data.currencies[0];
    setCurrencyId(fallbackCurrency?.id ?? "");
  }, [currencyId, drawer, references.data]);

  const supplierOptions = useMemo(() => {
    const options = references.suppliers.map((supplier) => ({ id: supplier.id, value: supplier.supplier_number, label: supplier.legal_name }));
    for (const line of requisition?.lines ?? []) {
      if (line.preferred_supplier_id && !options.some((item) => item.id === line.preferred_supplier_id)) {
        options.push({ id: line.preferred_supplier_id, value: line.preferred_supplier_number ?? "Supplier", label: line.preferred_supplier_name ?? "Unavailable supplier" });
      }
    }
    return options;
  }, [references.suppliers, requisition]);

  const updateLine = (index: number, patch: Partial<EditableLine>) => setLines((current) => current.map((line, lineIndex) => lineIndex === index ? { ...line, ...patch } : line));
  const removeLine = (index: number) => setLines((current) => current.filter((_, lineIndex) => lineIndex !== index));

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    if (!requesterId || !selectedCurrency) { toast.error("Choose a requester and currency"); return; }
    let linePayload: RequisitionLineInput[];
    try {
      linePayload = lines.map((line, index) => lineInput(line, index, selectedCurrency.minor_units));
    } catch (inputError) {
      toast.error(inputError instanceof Error ? inputError.message : "Check the requisition lines");
      return;
    }
    const payload: RequisitionInput = {
      requester_employee_id: requesterId,
      currency_id: selectedCurrency.id,
      title: title.trim(),
      purpose: purpose.trim() || null,
      needed_by: neededBy || null,
      lines: linePayload,
    };
    setSaving(true);
    try {
      const response = requisition
        ? await procurementService.updateRequisition(requisition.id, { ...payload, expected_version: requisition.version })
        : await procurementService.createRequisition({ ...payload, idempotency_key: crypto.randomUUID() });
      if (!response.success) throw new Error(responseMessage(response, "Requisition could not be saved"));
      toast.success("Requisition saved");
      onSaved();
    } catch (saveError) {
      toast.error(saveError instanceof Error ? saveError.message : "Requisition could not be saved");
    } finally {
      setSaving(false);
    }
  };

  return <DialogShell onClose={saving ? () => undefined : onClose} open={drawer !== null} panelClassName="sm:max-w-[760px]">
    <DialogHeader onClose={saving ? undefined : onClose} title={requisition ? `Edit ${requisition.requisition_number}` : "New requisition"} />
    <form onSubmit={submit}>
      <DialogBody className="space-y-6">
        {references.error ? <div className="space-y-3"><Notice danger icon={<XCircle />} text={references.error} /><Button onClick={references.retry} type="button" variant="secondary">Try again</Button></div> : null}
        <section className="space-y-5" aria-labelledby="request-details-heading">
          <h3 className="text-sm font-semibold text-[var(--text-strong)]" id="request-details-heading">Request details</h3>
          <div><Label htmlFor="requisition-title">Title</Label><Input className="mt-1.5" data-autofocus="true" id="requisition-title" maxLength={180} onChange={(event) => setTitle(event.target.value)} required value={title} /></div>
          <div><Label htmlFor="requisition-requester">Requester</Label><SearchableSelect<string> allowClear={false} className="mt-1.5" disabled={references.loading} id="requisition-requester" loading={references.loading} onChange={setRequesterId} options={references.requesters.map((employee) => ({ id: employee.id, value: employee.display_name, label: employee.employee_number, description: employee.work_email ?? undefined }))} placeholder="Choose an employee" value={requesterId} /></div>
          <div className="grid gap-5 sm:grid-cols-2"><div><Label htmlFor="requisition-currency">Currency</Label><Select className="mt-1.5" disabled={references.loading || Boolean(requisition)} id="requisition-currency" onChange={(event) => setCurrencyId(event.target.value)} required value={currencyId}><option value="">Choose a currency</option>{references.data?.currencies.map((currency) => <option key={currency.id} value={currency.id}>{currency.code} · {currency.name}{currency.is_reporting ? " · Reporting" : ""}</option>)}</Select>{requisition ? <p className="mt-1.5 text-xs leading-5 text-[var(--text-muted)]">Currency cannot be changed after the request is created.</p> : null}</div><div><Label htmlFor="requisition-needed-by">Needed by</Label><Input className="mt-1.5" id="requisition-needed-by" onChange={(event) => setNeededBy(event.target.value)} type="date" value={neededBy} /></div></div>
          <div><Label htmlFor="requisition-purpose">Purpose</Label><Textarea className="mt-1.5 min-h-24" id="requisition-purpose" maxLength={2000} onChange={(event) => setPurpose(event.target.value)} value={purpose} /></div>
        </section>

        <section className="space-y-4 border-t border-[var(--border)] pt-6" aria-labelledby="request-lines-heading">
          <div className="flex items-center justify-between gap-4"><div><h3 className="text-sm font-semibold text-[var(--text-strong)]" id="request-lines-heading">Items</h3><p className="mt-1 text-xs text-[var(--text-muted)]">Enter whole quantities and estimates in {selectedCurrency?.code ?? "the selected currency"}.</p></div><Button disabled={lines.length >= 200} onClick={() => setLines((current) => [...current, emptyLine()])} size="sm" type="button" variant="secondary"><Plus className="size-4" />Add item</Button></div>
          {lines.map((line, index) => <div className="space-y-4 rounded-[var(--radius-lg)] border border-[var(--border)] bg-[var(--surface-muted)] p-4" key={line.key}>
            <div className="flex items-center justify-between"><p className="text-xs font-semibold uppercase tracking-[0.12em] text-[var(--text-subtle)]">Item {index + 1}</p>{lines.length > 1 ? <Button aria-label={`Remove item ${index + 1}`} onClick={() => removeLine(index)} size="icon-sm" type="button" variant="ghost"><Trash2 className="size-4" /></Button> : null}</div>
            <div><Label htmlFor={`line-description-${line.key}`}>Description</Label><Input className="mt-1.5" id={`line-description-${line.key}`} maxLength={500} onChange={(event) => updateLine(index, { description: event.target.value })} required value={line.description} /></div>
            <div className="grid gap-4 sm:grid-cols-3"><div><Label htmlFor={`line-quantity-${line.key}`}>Quantity</Label><Input className="mt-1.5 font-tabular" id={`line-quantity-${line.key}`} inputMode="numeric" min={1} onChange={(event) => updateLine(index, { quantity: event.target.value })} required type="number" value={line.quantity} /></div><div><Label htmlFor={`line-unit-${line.key}`}>Unit</Label><Input className="mt-1.5" id={`line-unit-${line.key}`} maxLength={40} onChange={(event) => updateLine(index, { unitLabel: event.target.value })} placeholder="items" value={line.unitLabel} /></div><div><Label htmlFor={`line-amount-${line.key}`}>Unit estimate</Label><Input className="mt-1.5 font-tabular" id={`line-amount-${line.key}`} inputMode="decimal" onChange={(event) => updateLine(index, { amount: event.target.value })} placeholder={exactAmount(0, selectedCurrency?.minor_units ?? 2)} required value={line.amount} /></div></div>
            <div><Label htmlFor={`line-supplier-${line.key}`}>Preferred supplier</Label><SearchableSelect<string> className="mt-1.5" id={`line-supplier-${line.key}`} onChange={(value) => updateLine(index, { supplierId: value })} options={supplierOptions} placeholder="No preferred supplier" value={line.supplierId} /></div>
          </div>)}
          <div className="flex items-center justify-between border-t border-[var(--border)] pt-4 text-sm"><span className="text-[var(--text-muted)]">Estimated total</span><span className="font-tabular font-semibold text-[var(--text-strong)]">{selectedCurrency ? formatMinor(estimatedTotal(lines, selectedCurrency.minor_units), selectedCurrency.minor_units, selectedCurrency.code) : "—"}</span></div>
        </section>
      </DialogBody>
      <DialogFooter><Button disabled={saving} onClick={onClose} type="button" variant="secondary">Cancel</Button><Button disabled={saving || references.loading || Boolean(references.error)} type="submit">{saving ? <Loader2 className="size-4 animate-spin" /> : null}{saving ? "Saving…" : "Save draft"}</Button></DialogFooter>
    </form>
  </DialogShell>;
}

function LifecycleDrawer({ action, onClose, onDone }: { action: LifecycleAction | null; onClose: () => void; onDone: () => void }) {
  const [note, setNote] = useState("");
  const [pending, setPending] = useState(false);
  useEffect(() => { if (action) setNote(""); }, [action]);
  const labels: Record<LifecycleKind, { title: string; action: string; message: string }> = {
    submit: { title: "Submit requisition?", action: "Submit", message: "The request will be locked for review. Another Procurement operator must make the decision." },
    approve: { title: "Approve requisition?", action: "Approve", message: "The approval will be recorded. This does not create an order or Finance posting." },
    reject: { title: "Reject requisition?", action: "Reject", message: "The rejection and reason will be recorded against the request." },
    cancel: { title: "Cancel requisition?", action: "Cancel request", message: "The submitted request will leave the approval queue and cannot be reopened." },
  };
  const current = action ? labels[action.kind] : labels.submit;

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    if (!action || pending) return;
    if ((action.kind === "reject" || action.kind === "cancel") && !note.trim()) { toast.error("Enter a reason"); return; }
    setPending(true);
    try {
      const response = action.kind === "submit"
        ? await procurementService.submitRequisition(action.requisition.id, action.requisition.version)
        : action.kind === "approve"
          ? await procurementService.approveRequisition(action.requisition.id, action.requisition.version, note.trim() || null)
          : action.kind === "reject"
            ? await procurementService.rejectRequisition(action.requisition.id, action.requisition.version, note.trim() || null)
            : await procurementService.cancelRequisition(action.requisition.id, action.requisition.version, note.trim() || null);
      const failureAction = action.kind === "submit" ? "submitted" : action.kind === "approve" ? "approved" : action.kind === "reject" ? "rejected" : "cancelled";
      if (!response.success) throw new Error(responseMessage(response, `Requisition could not be ${failureAction}`));
      toast.success(action.kind === "submit" ? "Requisition submitted" : action.kind === "approve" ? "Requisition approved" : action.kind === "reject" ? "Requisition rejected" : "Requisition cancelled");
      onDone();
    } catch (actionError) {
      toast.error(actionError instanceof Error ? actionError.message : "Requisition could not be updated");
    } finally {
      setPending(false);
    }
  };

  return <DialogShell onClose={pending ? () => undefined : onClose} open={action !== null}>
    <DialogHeader onClose={pending ? undefined : onClose} title={current.title} />
    <form onSubmit={submit}>
      <DialogBody className="space-y-5">
        <Notice icon={action?.kind === "approve" ? <CheckCircle2 /> : action?.kind === "submit" ? <Send /> : <XCircle />} text={current.message} />
        {action?.kind !== "submit" ? <div><Label htmlFor="requisition-decision-note">{action?.kind === "approve" ? "Decision note" : "Reason"}</Label><Textarea className="mt-1.5 min-h-28" data-autofocus="true" id="requisition-decision-note" maxLength={1000} onChange={(event) => setNote(event.target.value)} required={action?.kind === "reject" || action?.kind === "cancel"} value={note} /></div> : null}
      </DialogBody>
      <DialogFooter><Button disabled={pending} onClick={onClose} type="button" variant="secondary">Keep request</Button><Button disabled={pending} type="submit" variant={action?.kind === "reject" || action?.kind === "cancel" ? "destructive" : "default"}>{pending ? <Loader2 className="size-4 animate-spin" /> : null}{pending ? "Saving…" : current.action}</Button></DialogFooter>
    </form>
  </DialogShell>;
}

type ReferenceState = {
  data: ProcurementReferenceData | null;
  suppliers: Supplier[];
  requesters: RequesterCandidate[];
  loading: boolean;
  error: string | null;
  retry: () => void;
};

function useProcurementReferences(enabled: boolean): ReferenceState {
  const [retryKey, setRetryKey] = useState(0);
  const [state, setState] = useState<Omit<ReferenceState, "retry">>({ data: null, suppliers: [], requesters: [], loading: false, error: null });
  useEffect(() => {
    if (!enabled || state.data) return;
    let active = true;
    setState((current) => ({ ...current, loading: true, error: null }));
    void Promise.all([
      procurementService.referenceData(),
      procurementService.requesterCandidates(),
      procurementService.listSuppliers({ page: 1, per_page: 100, status: "active" }),
    ]).then(([referenceResponse, requesterResponse, supplierResponse]) => {
      if (!active) return;
      if (!referenceResponse.success || !referenceResponse.data) throw new Error(responseMessage(referenceResponse, "Currencies could not be loaded"));
      if (!requesterResponse.success || !requesterResponse.data) throw new Error(responseMessage(requesterResponse, "Requesters could not be loaded"));
      if (!supplierResponse.success || !supplierResponse.data) throw new Error(responseMessage(supplierResponse, "Suppliers could not be loaded"));
      setState({ data: referenceResponse.data, requesters: requesterResponse.data.employees, suppliers: supplierResponse.data.suppliers, loading: false, error: null });
    }).catch((referenceError) => {
      if (active) setState((current) => ({ ...current, loading: false, error: referenceError instanceof Error ? referenceError.message : "Procurement references could not be loaded" }));
    });
    return () => { active = false; };
  }, [enabled, retryKey]);
  return {
    ...state,
    retry: () => {
      setState((current) => ({ ...current, error: null }));
      setRetryKey((current) => current + 1);
    },
  };
}

type EditableLine = { key: string; description: string; quantity: string; unitLabel: string; amount: string; supplierId: string | null };
function emptyLine(): EditableLine { return { key: crypto.randomUUID(), description: "", quantity: "1", unitLabel: "", amount: "", supplierId: null }; }
function editableLine(line: RequisitionLine, minorUnits: number): EditableLine { return { key: line.id, description: line.description, quantity: String(line.quantity), unitLabel: line.unit_label ?? "", amount: exactAmount(line.estimated_unit_amount_minor, minorUnits), supplierId: line.preferred_supplier_id }; }

function lineInput(line: EditableLine, index: number, minorUnits: number): RequisitionLineInput {
  const quantity = Number(line.quantity);
  if (!Number.isSafeInteger(quantity) || quantity < 1 || quantity > 1_000_000_000) throw new Error(`Item ${index + 1} needs a valid whole quantity`);
  const amount = parseAmount(line.amount, minorUnits);
  if (amount === null || amount < 0) throw new Error(`Item ${index + 1} needs a valid unit estimate`);
  if (!line.description.trim()) throw new Error(`Item ${index + 1} needs a description`);
  if (!Number.isSafeInteger(quantity * amount)) throw new Error(`Item ${index + 1} estimate is too large`);
  return { description: line.description.trim(), quantity, unit_label: line.unitLabel.trim() || null, estimated_unit_amount_minor: amount, preferred_supplier_id: line.supplierId };
}

function estimatedTotal(lines: EditableLine[], minorUnits: number) {
  return lines.reduce((total, line) => {
    const quantity = Number(line.quantity);
    const amount = parseAmount(line.amount, minorUnits);
    if (!Number.isSafeInteger(quantity) || amount === null) return total;
    const lineTotal = quantity * amount;
    return Number.isSafeInteger(total + lineTotal) ? total + lineTotal : total;
  }, 0);
}

function exactAmount(amountMinor: number, minorUnits: number) {
  const value = String(Math.trunc(amountMinor)).padStart(minorUnits + 1, "0");
  return minorUnits === 0 ? value : `${value.slice(0, -minorUnits)}.${value.slice(-minorUnits)}`;
}

function parseAmount(value: string, minorUnits: number) {
  const normalized = value.trim();
  if (!/^\d+(\.\d*)?$/.test(normalized)) return null;
  const [whole, fraction = ""] = normalized.split(".");
  if (fraction.length > minorUnits) return null;
  const parsed = Number(`${whole}${fraction.padEnd(minorUnits, "0")}`);
  return Number.isSafeInteger(parsed) ? parsed : null;
}

function formatMinor(amountMinor: number, minorUnits: number, code: string) {
  return new Intl.NumberFormat(undefined, { style: "currency", currency: code, minimumFractionDigits: minorUnits, maximumFractionDigits: minorUnits }).format(amountMinor / 10 ** minorUnits);
}

function formatDate(value: string) { return new Intl.DateTimeFormat(undefined, { dateStyle: "medium", timeZone: "UTC" }).format(new Date(`${value}T00:00:00Z`)); }

function RequisitionStatusBadge({ status }: { status: RequisitionStatus }) {
  const tone = status === "approved" ? "success" : status === "submitted" ? "info" : status === "draft" ? "warning" : status === "rejected" ? "danger" : "neutral";
  return <Badge tone={tone}>{status}</Badge>;
}

function Fact({ label, value }: { label: string; value: string }) {
  return <div className="rounded-[var(--radius-lg)] border border-[var(--border)] bg-[var(--surface)] p-4 shadow-[var(--shadow-card)]"><p className="text-[11px] font-semibold uppercase tracking-[0.12em] text-[var(--text-subtle)]">{label}</p><p className="mt-1 text-sm font-medium text-[var(--text-strong)]">{value}</p></div>;
}

function Notice({ danger = false, icon, text }: { danger?: boolean; icon: ReactNode; text: string }) {
  return <div className={`flex gap-3 rounded-[var(--radius-lg)] border p-4 ${danger ? "border-[var(--tone-danger)] bg-[var(--tone-danger-bg)] text-[var(--tone-danger)]" : "border-[var(--border)] bg-[var(--surface-muted)] text-[var(--text-muted)]"}`}><span className="mt-0.5 shrink-0 [&_svg]:size-5">{icon}</span><p className="text-sm leading-6">{text}</p></div>;
}

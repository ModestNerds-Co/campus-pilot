/** Purchase order list, direct-load detail, and version-safe drawer workflows. */

import { useCallback, useEffect, useMemo, useState } from "react";
import type { FormEvent, ReactNode } from "react";
import { Link } from "@tanstack/react-router";
import {
  ArrowLeft, Ban, CheckCircle2, Eye, FileCheck2, Loader2, PackageCheck,
  Plus, Search, Send, ShoppingCart, TriangleAlert,
} from "lucide-react";
import toast from "react-hot-toast";

import { SearchableSelect } from "@/components/searchable-select";
import { Badge } from "@/components/ui/badge";
import { Button, buttonVariants } from "@/components/ui/button";
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
  PurchaseOrder, PurchaseOrderLine, PurchaseOrderLineInput,
  PurchaseOrderStatus, PurchaseOrderSummary, Requisition, RequisitionLine,
  RequisitionSummary, Supplier,
} from "./types";

type OrderDrawerState = { mode: "create"; order: null } | { mode: "edit"; order: PurchaseOrder };
type OrderAction = { kind: "issue" | "cancel"; order: PurchaseOrder };

export function PurchaseOrdersWorkspace() {
  const permissions = useAuthStore((state) => state.user?.permissions);
  const canCreate = hasPermission(permissions, "procurement:create");
  const [orders, setOrders] = useState<PurchaseOrderSummary[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [page, setPage] = useState(1);
  const [totalPages, setTotalPages] = useState(1);
  const [search, setSearch] = useState("");
  const [submittedSearch, setSubmittedSearch] = useState("");
  const [status, setStatus] = useState("all");
  const [drawer, setDrawer] = useState<OrderDrawerState | null>(null);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const response = await procurementService.listPurchaseOrders({
        page,
        per_page: 20,
        search: submittedSearch || undefined,
        status: status === "all" ? undefined : status,
      });
      if (!response.success || !response.data) throw new Error(responseMessage(response, "Purchase orders could not be loaded"));
      setOrders(response.data.purchase_orders);
      setTotalPages(response.pagination?.total_pages ?? 1);
    } catch (loadError) {
      setError(loadError instanceof Error ? loadError.message : "Purchase orders could not be loaded");
    } finally {
      setLoading(false);
    }
  }, [page, status, submittedSearch]);

  useEffect(() => { void load(); }, [load]);
  usePageChrome("Purchase orders", canCreate ? <Button onClick={() => setDrawer({ mode: "create", order: null })}><Plus className="size-4" />New purchase order</Button> : null);
  const filtered = Boolean(submittedSearch) || status !== "all";

  return <div className="space-y-5">
    <p className="text-sm text-[var(--text-muted)]">Issue approved requests to suppliers and track what remains to be received.</p>
    <TableControlsBar>
      <TableControlsSearch onSubmit={(event) => { event.preventDefault(); setPage(1); setSubmittedSearch(search.trim()); }}>
        <Input aria-label="Search purchase orders" leadingIcon={<Search />} onChange={(event) => setSearch(event.target.value)} placeholder="Search order, requisition, or supplier…" value={search} />
        <Button type="submit" variant="secondary">Search</Button>
      </TableControlsSearch>
      <Select aria-label="Purchase order status" className="sm:w-52" onChange={(event) => { setPage(1); setStatus(event.target.value); }} value={status}>
        <option value="all">All statuses</option>
        <option value="draft">Draft</option>
        <option value="issued">Issued</option>
        <option value="partially_received">Partially received</option>
        <option value="received">Received</option>
        <option value="cancelled">Cancelled</option>
      </Select>
      {!loading && orders.length > 0 ? <TableControlsPagination onNext={() => setPage((value) => Math.min(totalPages, value + 1))} onPrevious={() => setPage((value) => Math.max(1, value - 1))} page={page} totalPages={totalPages} /> : null}
    </TableControlsBar>

    <TableWrap>
      {loading ? <TableLoading columns={7} label="Loading purchase orders…" /> : error ? <TableError description={error} onRetry={() => void load()} /> : orders.length === 0 ? <TableEmpty description={filtered ? "Change the current filters." : "Create an order from an approved requisition."} icon={<ShoppingCart />} title={filtered ? "No purchase orders match these filters" : "No purchase orders yet"} /> : <TableScroll><Table>
        <THead><tr><TH>Purchase order</TH><TH>Supplier</TH><TH>Requisition</TH><TH>Delivery</TH><TH>Total</TH><TH>Status</TH><TH className="text-right">Open</TH></tr></THead>
        <TBody>{orders.map((order) => <TR key={order.id}>
          <TD><p className="font-tabular font-semibold text-[var(--text-strong)]">{order.purchase_order_number}</p><p className="mt-1 text-xs text-[var(--text-subtle)]">{order.line_count} {order.line_count === 1 ? "line" : "lines"}</p></TD>
          <TD><p className="font-medium text-[var(--text-strong)]">{order.supplier_name}</p><p className="mt-1 font-tabular text-xs text-[var(--text-subtle)]">{order.supplier_number}</p></TD>
          <TD className="font-tabular text-[var(--text-muted)]">{order.requisition_number}</TD>
          <TD className="whitespace-nowrap text-[var(--text-muted)]">{order.delivery_date ? formatDate(order.delivery_date) : "Not set"}</TD>
          <TD className="whitespace-nowrap font-tabular font-semibold text-[var(--text-strong)]">{formatMinor(order.total_minor, order.currency_minor_units, order.currency_code)}</TD>
          <TD><PurchaseOrderStatusBadge status={order.status} /></TD>
          <TD className="text-right"><Link aria-label={`Open ${order.purchase_order_number}`} className={buttonVariants({ variant: "ghost", size: "icon-sm" })} params={{ purchaseOrderId: order.id }} to="/modules/procurement/purchase-orders/$purchaseOrderId"><Eye className="size-4" /></Link></TD>
        </TR>)}</TBody>
      </Table></TableScroll>}
    </TableWrap>

    <PurchaseOrderDrawer drawer={drawer} onClose={() => setDrawer(null)} onSaved={() => { setDrawer(null); void load(); }} />
  </div>;
}

export function PurchaseOrderDetail({ purchaseOrderId }: { purchaseOrderId: string }) {
  const permissions = useAuthStore((state) => state.user?.permissions);
  const userId = useAuthStore((state) => state.user?.id);
  const canEdit = hasPermission(permissions, "procurement:edit");
  const canApprove = hasPermission(permissions, "procurement:approve");
  const [order, setOrder] = useState<PurchaseOrder | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [editOpen, setEditOpen] = useState(false);
  const [action, setAction] = useState<OrderAction | null>(null);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const response = await procurementService.readPurchaseOrder(purchaseOrderId);
      if (!response.success || !response.data) throw new Error(responseMessage(response, "Purchase order could not be loaded"));
      setOrder(response.data);
    } catch (loadError) {
      setError(loadError instanceof Error ? loadError.message : "Purchase order could not be loaded");
    } finally {
      setLoading(false);
    }
  }, [purchaseOrderId]);

  useEffect(() => { void load(); }, [load]);
  usePageChrome(order?.purchase_order_number ?? "Purchase order");

  if (loading) return <div aria-label="Loading purchase order" className="space-y-4" role="status"><div className="h-36 animate-pulse rounded-[var(--radius-xl)] bg-[var(--surface-sunken)]" /><div className="h-72 animate-pulse rounded-[var(--radius-xl)] bg-[var(--surface-sunken)]" /></div>;
  if (error || !order) return <TableWrap><TableError description={error ?? "Purchase order was not found"} onRetry={() => void load()} title="Purchase order could not be opened" /></TableWrap>;

  const mayCancel = canApprove && (order.status === "draft" || order.status === "issued");
  const currentUserPreparedOrder = order.created_by === userId || order.prepared_by === userId;
  const mayIssue = canApprove && order.status === "draft" && !currentUserPreparedOrder;
  return <div className="space-y-6">
    <section className="rounded-[var(--radius-xl)] border border-[var(--border)] bg-[var(--surface)] p-5 shadow-[var(--shadow-card)] sm:p-6">
      <div className="flex flex-col gap-5 lg:flex-row lg:items-start lg:justify-between">
        <div className="min-w-0">
          <Link className={buttonVariants({ variant: "ghost", size: "sm" })} to="/modules/procurement/purchase-orders"><ArrowLeft className="size-4" />Back to purchase orders</Link>
          <div className="mt-4 flex flex-wrap items-center gap-3"><h1 className="font-tabular text-xl font-semibold text-[var(--text-strong)]">{order.purchase_order_number}</h1><PurchaseOrderStatusBadge status={order.status} /></div>
          <p className="mt-2 text-base font-medium text-[var(--text-body)]">{order.supplier_name}</p>
          <p className="mt-1 font-tabular text-sm text-[var(--text-muted)]">From {order.requisition_number}</p>
        </div>
        <div className="flex flex-wrap gap-2">
          {canEdit && order.status === "draft" ? <Button onClick={() => setEditOpen(true)} variant="secondary">Edit</Button> : null}
          {mayCancel ? <Button onClick={() => setAction({ kind: "cancel", order })} variant="secondary"><Ban className="size-4" />Cancel order</Button> : null}
          {mayIssue ? <Button onClick={() => setAction({ kind: "issue", order })}><Send className="size-4" />Issue order</Button> : null}
        </div>
      </div>
    </section>

    {order.status === "draft" && currentUserPreparedOrder ? <Notice icon={<FileCheck2 />} text="A different Procurement approver must issue this purchase order." /> : null}
    {order.status === "partially_received" ? <Notice icon={<PackageCheck />} text="Some quantities have been posted as received. The remaining quantities are shown per line." /> : null}
    {order.cancellation_note ? <Notice danger icon={<Ban />} text={`Cancelled: ${order.cancellation_note}`} /> : null}

    <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
      <Fact label="Supplier" value={`${order.supplier_name} · ${order.supplier_number}`} />
      <Fact label="Delivery date" value={order.delivery_date ? formatDate(order.delivery_date) : "Not set"} />
      <Fact label="Requester" value={`${order.requester_name} · ${order.requester_employee_number}`} />
      <Fact label="Order total" value={formatMinor(order.total_minor, order.currency_minor_units, order.currency_code)} />
    </div>

    {order.notes ? <Fact label="Notes" value={order.notes} /> : null}

    <TableWrap><TableScroll><Table>
      <THead><tr><TH>#</TH><TH>Item</TH><TH className="text-right">Ordered</TH><TH className="text-right">Received</TH><TH className="text-right">Remaining</TH><TH className="text-right">Unit price</TH><TH className="text-right">Line total</TH></tr></THead>
      <TBody>{order.lines.map((line) => <TR key={line.id}>
        <TD className="font-tabular text-[var(--text-subtle)]">{line.line_number}</TD>
        <TD className="min-w-64 font-medium text-[var(--text-strong)]">{line.description}</TD>
        <QuantityCell line={line} value={line.quantity_minor} />
        <QuantityCell line={line} value={line.received_quantity_minor} />
        <QuantityCell emphasized={line.remaining_quantity_minor > 0} line={line} value={line.remaining_quantity_minor} />
        <TD className="whitespace-nowrap text-right font-tabular">{formatMinor(line.unit_amount_minor, order.currency_minor_units, order.currency_code)}</TD>
        <TD className="whitespace-nowrap text-right font-tabular font-semibold">{formatMinor(line.line_amount_minor, order.currency_minor_units, order.currency_code)}</TD>
      </TR>)}</TBody>
      <tfoot className="border-t border-[var(--border)] bg-[var(--surface-muted)]"><tr><TD className="text-right font-semibold text-[var(--text-strong)]" colSpan={6}>Order total</TD><TD className="whitespace-nowrap text-right font-tabular font-semibold text-[var(--text-strong)]">{formatMinor(order.total_minor, order.currency_minor_units, order.currency_code)}</TD></tr></tfoot>
    </Table></TableScroll></TableWrap>

    <PurchaseOrderDrawer drawer={editOpen ? { mode: "edit", order } : null} onClose={() => setEditOpen(false)} onSaved={() => { setEditOpen(false); void load(); }} />
    <PurchaseOrderActionDrawer action={action} onClose={() => setAction(null)} onDone={() => { setAction(null); void load(); }} />
  </div>;
}

function PurchaseOrderDrawer({ drawer, onClose, onSaved }: { drawer: OrderDrawerState | null; onClose: () => void; onSaved: () => void }) {
  const order = drawer?.order ?? null;
  const references = usePurchaseOrderReferences(drawer !== null);
  const [requisitionId, setRequisitionId] = useState<string | null>(null);
  const [supplierId, setSupplierId] = useState<string | null>(null);
  const [requisition, setRequisition] = useState<Requisition | null>(null);
  const [sourceLoading, setSourceLoading] = useState(false);
  const [sourceError, setSourceError] = useState<string | null>(null);
  const [deliveryDate, setDeliveryDate] = useState("");
  const [notes, setNotes] = useState("");
  const [lines, setLines] = useState<EditableOrderLine[]>([]);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    if (!drawer) return;
    setRequisitionId(order?.requisition_id ?? null);
    setSupplierId(order?.supplier_id ?? null);
    setDeliveryDate(order?.delivery_date ?? "");
    setNotes(order?.notes ?? "");
    setRequisition(null);
    setLines([]);
    setSourceError(null);
  }, [drawer, order]);

  useEffect(() => {
    if (!drawer || !requisitionId) { setRequisition(null); setLines([]); return; }
    if (order) {
      setRequisition(null);
      setSourceLoading(false);
      setSourceError(null);
      setLines(editOrderLines(order.lines, order.currency_minor_units));
      return;
    }
    let active = true;
    setSourceLoading(true);
    setSourceError(null);
    void procurementService.readRequisition(requisitionId).then((response) => {
      if (!active) return;
      if (!response.success || !response.data) throw new Error(responseMessage(response, "Requisition could not be loaded"));
      setRequisition(response.data);
      setLines(newOrderLines(response.data.lines, response.data.currency_minor_units));
    }).catch((loadError) => {
      if (active) setSourceError(loadError instanceof Error ? loadError.message : "Requisition could not be loaded");
    }).finally(() => { if (active) setSourceLoading(false); });
    return () => { active = false; };
  }, [drawer, order, requisitionId]);

  const requisitionOptions = useMemo(() => references.requisitions.map((item) => ({ id: item.id, value: item.requisition_number, label: item.title, description: item.requester_name })), [references.requisitions]);
  const supplierOptions = useMemo(() => {
    const options = references.suppliers.map((item) => ({ id: item.id, value: item.supplier_number, label: item.legal_name }));
    if (order && !options.some((item) => item.id === order.supplier_id)) options.push({ id: order.supplier_id, value: order.supplier_number, label: order.supplier_name });
    return options;
  }, [order, references.suppliers]);

  const updateLine = (key: string, patch: Partial<EditableOrderLine>) => setLines((current) => current.map((line) => line.key === key ? { ...line, ...patch } : line));

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    if ((!order && !requisition) || !supplierId) { toast.error("Choose an approved requisition and supplier"); return; }
    const currencyMinorUnits = order?.currency_minor_units ?? requisition!.currency_minor_units;
    let linePayload: PurchaseOrderLineInput[];
    try {
      linePayload = lines.filter((line) => line.included).map((line, index) => orderLineInput(line, index, currencyMinorUnits));
      if (linePayload.length === 0) throw new Error("Include at least one requisition line");
    } catch (inputError) {
      toast.error(inputError instanceof Error ? inputError.message : "Check the purchase order lines");
      return;
    }
    setSaving(true);
    try {
      const response = order
        ? await procurementService.updatePurchaseOrder(order.id, { delivery_date: deliveryDate || null, notes: optional(notes), lines: linePayload, expected_version: order.version })
        : await procurementService.createPurchaseOrder({ requisition_id: requisition!.id, supplier_id: supplierId, delivery_date: deliveryDate || null, notes: optional(notes), lines: linePayload, idempotency_key: crypto.randomUUID() });
      if (!response.success) throw new Error(responseMessage(response, "Purchase order could not be saved"));
      toast.success("Purchase order saved");
      onSaved();
    } catch (saveError) {
      toast.error(saveError instanceof Error ? saveError.message : "Purchase order could not be saved");
    } finally {
      setSaving(false);
    }
  };

  const minorUnits = requisition?.currency_minor_units ?? order?.currency_minor_units ?? 2;
  const currencyCode = requisition?.currency_code ?? order?.currency_code ?? "";
  return <DialogShell onClose={saving ? () => undefined : onClose} open={drawer !== null} panelClassName="sm:max-w-[780px]">
    <DialogHeader onClose={saving ? undefined : onClose} title={order ? `Edit ${order.purchase_order_number}` : "New purchase order"} />
    <form onSubmit={submit}>
      <DialogBody className="space-y-6">
        {references.error ? <ErrorNotice message={references.error} onRetry={references.retry} /> : null}
        <section className="space-y-5" aria-labelledby="order-source-heading">
          <h3 className="text-sm font-semibold text-[var(--text-strong)]" id="order-source-heading">Order source</h3>
          <div><Label htmlFor="purchase-order-requisition">Approved requisition</Label><SearchableSelect allowClear={!order} className="mt-1.5" disabled={Boolean(order) || references.loading} id="purchase-order-requisition" loading={references.loading} onChange={(value) => setRequisitionId(value)} options={requisitionOptions} placeholder="Choose a requisition" value={requisitionId} /></div>
          <div><Label htmlFor="purchase-order-supplier">Supplier</Label><SearchableSelect allowClear={false} className="mt-1.5" disabled={Boolean(order) || references.loading} id="purchase-order-supplier" loading={references.loading} onChange={setSupplierId} options={supplierOptions} placeholder="Choose a supplier" value={supplierId} /></div>
        </section>

        <section className="space-y-5" aria-labelledby="order-delivery-heading">
          <h3 className="text-sm font-semibold text-[var(--text-strong)]" id="order-delivery-heading">Delivery</h3>
          <div><Label htmlFor="purchase-order-date">Expected date</Label><Input className="mt-1.5 sm:max-w-xs" id="purchase-order-date" onChange={(event) => setDeliveryDate(event.target.value)} type="date" value={deliveryDate} /></div>
          <div><Label htmlFor="purchase-order-notes">Notes</Label><Textarea className="mt-1.5 min-h-24" id="purchase-order-notes" maxLength={2000} onChange={(event) => setNotes(event.target.value)} value={notes} /></div>
        </section>

        <section className="space-y-4" aria-labelledby="order-lines-heading">
          <div className="flex items-center justify-between gap-4"><h3 className="text-sm font-semibold text-[var(--text-strong)]" id="order-lines-heading">Order lines</h3>{currencyCode ? <Badge tone="outline">{currencyCode}</Badge> : null}</div>
          {sourceLoading ? <div aria-label="Loading requisition lines" className="flex items-center gap-2 rounded-[var(--radius-lg)] border border-[var(--border)] p-4 text-sm text-[var(--text-muted)]" role="status"><Loader2 className="size-4 animate-spin" />Loading requisition lines…</div> : sourceError ? <ErrorNotice message={sourceError} /> : !order && !requisition ? <Notice icon={<FileCheck2 />} text="Choose an approved requisition to prepare order lines." /> : <div className="space-y-3">
            {lines.map((line, index) => <div className="rounded-[var(--radius-lg)] border border-[var(--border)] bg-[var(--surface-muted)] p-4" key={line.key}>
              <div className="flex items-start gap-3">{order ? null : <input aria-label={`Include ${line.description}`} checked={line.included} className="mt-1 size-4 accent-[var(--brand-strong)]" onChange={(event) => updateLine(line.key, { included: event.target.checked })} type="checkbox" />}<div className="min-w-0 flex-1"><p className="font-medium text-[var(--text-strong)]">{index + 1}. {line.description}</p><p className="mt-1 text-xs text-[var(--text-muted)]">Requested {formatQuantity(line.availableMinor, line.quantityScale)} {line.unitLabel || "units"}</p></div></div>
              {line.included ? <div className="mt-4 grid gap-4 sm:grid-cols-2"><div><Label htmlFor={`purchase-order-quantity-${line.key}`}>Order quantity</Label><Input className="mt-1.5" id={`purchase-order-quantity-${line.key}`} inputMode="decimal" onChange={(event) => updateLine(line.key, { quantity: event.target.value })} required value={line.quantity} /></div><div><Label htmlFor={`purchase-order-price-${line.key}`}>Unit price</Label><Input className="mt-1.5" id={`purchase-order-price-${line.key}`} inputMode="decimal" onChange={(event) => updateLine(line.key, { unitPrice: event.target.value })} required value={line.unitPrice} /></div></div> : null}
            </div>)}
            <div className="flex items-center justify-between border-t border-[var(--border)] pt-4"><span className="text-sm text-[var(--text-muted)]">Draft total</span><strong className="font-tabular text-sm text-[var(--text-strong)]">{currencyCode ? formatMinor(draftOrderTotal(lines, minorUnits), minorUnits, currencyCode) : "—"}</strong></div>
          </div>}
        </section>
      </DialogBody>
      <DialogFooter><Button disabled={saving} onClick={onClose} type="button" variant="secondary">Cancel</Button><Button disabled={saving || sourceLoading || references.loading} type="submit">{saving ? <Loader2 className="size-4 animate-spin" /> : null}{saving ? "Saving…" : "Save purchase order"}</Button></DialogFooter>
    </form>
  </DialogShell>;
}

function PurchaseOrderActionDrawer({ action, onClose, onDone }: { action: OrderAction | null; onClose: () => void; onDone: () => void }) {
  const [note, setNote] = useState("");
  const [pending, setPending] = useState(false);
  useEffect(() => { if (action) setNote(""); }, [action]);

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    if (!action) return;
    setPending(true);
    try {
      const response = action.kind === "issue"
        ? await procurementService.issuePurchaseOrder(action.order.id, action.order.version)
        : await procurementService.cancelPurchaseOrder(action.order.id, action.order.version, note.trim());
      if (!response.success) throw new Error(responseMessage(response, action.kind === "issue" ? "Purchase order could not be issued" : "Purchase order could not be cancelled"));
      toast.success(action.kind === "issue" ? "Purchase order issued" : "Purchase order cancelled");
      onDone();
    } catch (actionError) {
      toast.error(actionError instanceof Error ? actionError.message : "Purchase order could not be updated");
    } finally {
      setPending(false);
    }
  };

  const issuing = action?.kind === "issue";
  return <DialogShell onClose={pending ? () => undefined : onClose} open={action !== null}>
    <DialogHeader onClose={pending ? undefined : onClose} title={issuing ? "Issue purchase order?" : "Cancel purchase order?"} />
    <form onSubmit={submit}>
      <DialogBody className="space-y-5">
        <Notice danger={!issuing} icon={issuing ? <Send /> : <TriangleAlert />} text={issuing ? `${action?.order.purchase_order_number ?? "This order"} will become available for goods receiving. Its supplier, currency, and line values can no longer be edited.` : `Cancel ${action?.order.purchase_order_number ?? "this order"}. Posted receipts are not reversed by this action.`} />
        {!issuing ? <div><Label htmlFor="purchase-order-cancel-note">Reason</Label><Textarea className="mt-1.5 min-h-28" data-autofocus="true" id="purchase-order-cancel-note" maxLength={1000} onChange={(event) => setNote(event.target.value)} required value={note} /></div> : null}
      </DialogBody>
      <DialogFooter><Button data-autofocus={issuing ? "true" : undefined} disabled={pending} onClick={onClose} type="button" variant="secondary">Keep order</Button><Button disabled={pending} type="submit" variant={issuing ? "default" : "destructive"}>{pending ? <Loader2 className="size-4 animate-spin" /> : null}{pending ? "Saving…" : issuing ? "Issue order" : "Cancel order"}</Button></DialogFooter>
    </form>
  </DialogShell>;
}

type PurchaseOrderReferences = { requisitions: RequisitionSummary[]; suppliers: Supplier[]; loading: boolean; error: string | null; retry: () => void };

function usePurchaseOrderReferences(enabled: boolean): PurchaseOrderReferences {
  const [retryKey, setRetryKey] = useState(0);
  const [state, setState] = useState<Omit<PurchaseOrderReferences, "retry">>({ requisitions: [], suppliers: [], loading: false, error: null });
  useEffect(() => {
    if (!enabled) return;
    let active = true;
    setState((current) => ({ ...current, loading: true, error: null }));
    void Promise.all([
      procurementService.listRequisitions({ page: 1, per_page: 100, status: "approved" }),
      procurementService.listSuppliers({ page: 1, per_page: 100, status: "active" }),
    ]).then(([requisitionResponse, supplierResponse]) => {
      if (!active) return;
      if (!requisitionResponse.success || !requisitionResponse.data) throw new Error(responseMessage(requisitionResponse, "Approved requisitions could not be loaded"));
      if (!supplierResponse.success || !supplierResponse.data) throw new Error(responseMessage(supplierResponse, "Suppliers could not be loaded"));
      setState({ requisitions: requisitionResponse.data.requisitions, suppliers: supplierResponse.data.suppliers, loading: false, error: null });
    }).catch((referenceError) => {
      if (active) setState((current) => ({ ...current, loading: false, error: referenceError instanceof Error ? referenceError.message : "Order references could not be loaded" }));
    });
    return () => { active = false; };
  }, [enabled, retryKey]);
  return { ...state, retry: () => setRetryKey((value) => value + 1) };
}

type EditableOrderLine = {
  key: string;
  requisitionLineId: string;
  description: string;
  unitLabel: string | null;
  included: boolean;
  quantity: string;
  quantityScale: number;
  availableMinor: number;
  unitPrice: string;
};

function newOrderLines(lines: RequisitionLine[], minorUnits: number): EditableOrderLine[] {
  return lines.map((line) => ({ key: line.id, requisitionLineId: line.id, description: line.description, unitLabel: line.unit_label, included: true, quantity: String(line.quantity), quantityScale: 0, availableMinor: line.quantity, unitPrice: exactAmount(line.estimated_unit_amount_minor, minorUnits) }));
}

function editOrderLines(orderLines: PurchaseOrderLine[], minorUnits: number): EditableOrderLine[] {
  return orderLines.map((line) => ({
    key: line.id,
    requisitionLineId: line.requisition_line_id,
    description: line.description,
    unitLabel: line.unit_label,
    included: true,
    quantity: exactAmount(line.quantity_minor, line.quantity_scale),
    quantityScale: line.quantity_scale,
    availableMinor: line.requisition_quantity_minor,
    unitPrice: exactAmount(line.unit_amount_minor, minorUnits),
  }));
}

function orderLineInput(line: EditableOrderLine, index: number, minorUnits: number): PurchaseOrderLineInput {
  const quantity = parseScaled(line.quantity, line.quantityScale);
  if (quantity === null || quantity < 1) throw new Error(`Line ${index + 1} needs a valid quantity`);
  if (quantity > line.availableMinor) throw new Error(`Line ${index + 1} exceeds the approved quantity`);
  const unitPrice = parseScaled(line.unitPrice, minorUnits);
  if (unitPrice === null || unitPrice < 0) throw new Error(`Line ${index + 1} needs a valid unit price`);
  return { requisition_line_id: line.requisitionLineId, quantity_minor: quantity, quantity_scale: line.quantityScale, unit_amount_minor: unitPrice };
}

function draftOrderTotal(lines: EditableOrderLine[], minorUnits: number) {
  return lines.reduce((total, line) => {
    if (!line.included) return total;
    const quantity = parseScaled(line.quantity, line.quantityScale);
    const unitPrice = parseScaled(line.unitPrice, minorUnits);
    if (quantity === null || unitPrice === null) return total;
    const lineTotal = Math.round((quantity * unitPrice) / 10 ** line.quantityScale);
    return Number.isSafeInteger(total + lineTotal) ? total + lineTotal : total;
  }, 0);
}

function QuantityCell({ emphasized = false, line, value }: { emphasized?: boolean; line: PurchaseOrderLine; value: number }) {
  return <TD className={`whitespace-nowrap text-right font-tabular ${emphasized ? "font-semibold text-[var(--brand-strong)]" : "text-[var(--text-body)]"}`}>{formatQuantity(value, line.quantity_scale)} {line.unit_label || "units"}</TD>;
}

function PurchaseOrderStatusBadge({ status }: { status: PurchaseOrderStatus }) {
  const tone = status === "received" ? "success" : status === "partially_received" ? "info" : status === "issued" ? "brand" : status === "draft" ? "warning" : "neutral";
  return <Badge tone={tone}>{status.replace("_", " ")}</Badge>;
}

function ErrorNotice({ message, onRetry }: { message: string; onRetry?: () => void }) {
  return <div className="flex flex-col gap-3 rounded-[var(--radius-lg)] border border-[var(--tone-danger-bd)] bg-[var(--tone-danger-bg)] p-4 sm:flex-row sm:items-center"><div className="flex min-w-0 flex-1 gap-3 text-[var(--tone-danger)]"><TriangleAlert className="mt-0.5 size-5 shrink-0" /><p className="text-sm leading-5">{message}</p></div>{onRetry ? <Button onClick={onRetry} type="button" variant="secondary">Try again</Button> : null}</div>;
}

function Notice({ danger = false, icon, text }: { danger?: boolean; icon: ReactNode; text: string }) {
  return <div className={`flex gap-3 rounded-[var(--radius-lg)] border p-4 ${danger ? "border-[var(--tone-danger-bd)] bg-[var(--tone-danger-bg)] text-[var(--tone-danger)]" : "border-[var(--border)] bg-[var(--surface-muted)] text-[var(--text-muted)]"}`}><span className="mt-0.5 shrink-0 [&_svg]:size-5">{icon}</span><p className="text-sm leading-6">{text}</p></div>;
}

function Fact({ label, value }: { label: string; value: string }) {
  return <div className="rounded-[var(--radius-lg)] border border-[var(--border)] bg-[var(--surface)] p-4 shadow-[var(--shadow-card)]"><p className="text-[11px] font-semibold uppercase tracking-[0.12em] text-[var(--text-subtle)]">{label}</p><p className="mt-1 break-words text-sm font-medium text-[var(--text-strong)]">{value}</p></div>;
}

function optional(value: string) { return value.trim() || null; }
function parseScaled(value: string, scale: number) { const normalized = value.trim(); if (!/^\d+(\.\d*)?$/.test(normalized)) return null; const [whole, fraction = ""] = normalized.split("."); if (fraction.length > scale) return null; const parsed = Number(`${whole}${fraction.padEnd(scale, "0")}`); return Number.isSafeInteger(parsed) ? parsed : null; }
function exactAmount(valueMinor: number, scale: number) { const sign = valueMinor < 0 ? "-" : ""; const value = String(Math.abs(Math.trunc(valueMinor))).padStart(scale + 1, "0"); return scale === 0 ? `${sign}${value}` : `${sign}${value.slice(0, -scale)}.${value.slice(-scale)}`; }
function formatQuantity(valueMinor: number, scale: number) { return exactAmount(valueMinor, scale).replace(/(\.\d*?)0+$/, "$1").replace(/\.$/, ""); }
function formatMinor(valueMinor: number, minorUnits: number, code: string) { return new Intl.NumberFormat(undefined, { style: "currency", currency: code, minimumFractionDigits: minorUnits, maximumFractionDigits: minorUnits }).format(valueMinor / 10 ** minorUnits); }
function formatDate(value: string) { return new Intl.DateTimeFormat(undefined, { dateStyle: "medium", timeZone: "UTC" }).format(new Date(`${value}T00:00:00Z`)); }

/** Goods receipt list and version-safe draft/post drawer workflows. */

import { useCallback, useEffect, useMemo, useState } from "react";
import type { FormEvent, ReactNode } from "react";
import { CheckCircle2, Edit3, Eye, Loader2, PackageCheck, Plus, Search, TriangleAlert } from "lucide-react";
import toast from "react-hot-toast";

import { SearchableSelect } from "@/components/searchable-select";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
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
  GoodsReceipt, GoodsReceiptInput, GoodsReceiptLineInput, GoodsReceiptStatus,
  GoodsReceiptSummary, PurchaseOrder, PurchaseOrderLine, PurchaseOrderSummary,
} from "./types";

type ReceiptDrawerState = { mode: "create"; receipt: null } | { mode: "edit"; receipt: GoodsReceipt };

export function GoodsReceiptsWorkspace() {
  const permissions = useAuthStore((state) => state.user?.permissions);
  const userId = useAuthStore((state) => state.user?.id);
  const canReceive = hasPermission(permissions, "procurement:receive");
  const [receipts, setReceipts] = useState<GoodsReceiptSummary[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [page, setPage] = useState(1);
  const [totalPages, setTotalPages] = useState(1);
  const [search, setSearch] = useState("");
  const [submittedSearch, setSubmittedSearch] = useState("");
  const [status, setStatus] = useState("all");
  const [drawer, setDrawer] = useState<ReceiptDrawerState | null>(null);
  const [detailId, setDetailId] = useState<string | null>(null);
  const [postReceipt, setPostReceipt] = useState<GoodsReceipt | null>(null);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const response = await procurementService.listGoodsReceipts({
        page,
        per_page: 20,
        search: submittedSearch || undefined,
        status: status === "all" ? undefined : status,
      });
      if (!response.success || !response.data) throw new Error(responseMessage(response, "Goods receipts could not be loaded"));
      setReceipts(response.data.goods_receipts);
      setTotalPages(response.pagination?.total_pages ?? 1);
    } catch (loadError) {
      setError(loadError instanceof Error ? loadError.message : "Goods receipts could not be loaded");
    } finally {
      setLoading(false);
    }
  }, [page, status, submittedSearch]);

  useEffect(() => { void load(); }, [load]);
  usePageChrome("Goods receipts", canReceive ? <Button onClick={() => setDrawer({ mode: "create", receipt: null })}><Plus className="size-4" />New goods receipt</Button> : null);
  const filtered = Boolean(submittedSearch) || status !== "all";

  const openReceipt = async (id: string, destination: "detail" | "edit" | "post") => {
    if (destination === "detail") { setDetailId(id); return; }
    try {
      const response = await procurementService.readGoodsReceipt(id);
      if (!response.success || !response.data) throw new Error(responseMessage(response, "Goods receipt could not be loaded"));
      if (destination === "edit") setDrawer({ mode: "edit", receipt: response.data });
      else setPostReceipt(response.data);
    } catch (readError) {
      toast.error(readError instanceof Error ? readError.message : "Goods receipt could not be loaded");
    }
  };

  return <div className="space-y-5">
    <p className="text-sm text-[var(--text-muted)]">Record supplier deliveries against issued purchase orders.</p>
    <TableControlsBar>
      <TableControlsSearch onSubmit={(event) => { event.preventDefault(); setPage(1); setSubmittedSearch(search.trim()); }}>
        <Input aria-label="Search goods receipts" leadingIcon={<Search />} onChange={(event) => setSearch(event.target.value)} placeholder="Search receipt, order, or supplier…" value={search} />
        <Button type="submit" variant="secondary">Search</Button>
      </TableControlsSearch>
      <Select aria-label="Goods receipt status" className="sm:w-44" onChange={(event) => { setPage(1); setStatus(event.target.value); }} value={status}>
        <option value="all">All statuses</option>
        <option value="draft">Draft</option>
        <option value="posted">Posted</option>
      </Select>
      {!loading && receipts.length > 0 ? <TableControlsPagination onNext={() => setPage((value) => Math.min(totalPages, value + 1))} onPrevious={() => setPage((value) => Math.max(1, value - 1))} page={page} totalPages={totalPages} /> : null}
    </TableControlsBar>

    <TableWrap>
      {loading ? <TableLoading columns={7} label="Loading goods receipts…" /> : error ? <TableError description={error} onRetry={() => void load()} /> : receipts.length === 0 ? <TableEmpty description={filtered ? "Change the current filters." : "Create a draft when a supplier delivery arrives."} icon={<PackageCheck />} title={filtered ? "No goods receipts match these filters" : "No goods receipts yet"} /> : <TableScroll><Table>
        <THead><tr><TH>Goods receipt</TH><TH>Purchase order</TH><TH>Supplier</TH><TH>Received</TH><TH>Lines</TH><TH>Status</TH><TH className="text-right">Actions</TH></tr></THead>
        <TBody>{receipts.map((receipt) => <TR key={receipt.id}>
          <TD><p className="font-tabular font-semibold text-[var(--text-strong)]">{receipt.goods_receipt_number}</p>{receipt.delivery_reference ? <p className="mt-1 text-xs text-[var(--text-subtle)]">Delivery reference {receipt.delivery_reference}</p> : null}</TD>
          <TD className="font-tabular text-[var(--text-muted)]">{receipt.purchase_order_number}</TD>
          <TD><p className="font-medium text-[var(--text-strong)]">{receipt.supplier_name}</p><p className="mt-1 font-tabular text-xs text-[var(--text-subtle)]">{receipt.supplier_number}</p></TD>
          <TD className="whitespace-nowrap text-[var(--text-muted)]">{formatDate(receipt.received_on)}</TD>
          <TD className="font-tabular text-[var(--text-muted)]">{receipt.line_count}</TD>
          <TD><GoodsReceiptStatusBadge status={receipt.status} /></TD>
          <TD className="text-right"><div className="inline-flex gap-1">
            <Button aria-label={`View ${receipt.goods_receipt_number}`} onClick={() => void openReceipt(receipt.id, "detail")} size="icon-sm" variant="ghost"><Eye className="size-4" /></Button>
            {canReceive && receipt.status === "draft" ? <Button aria-label={`Edit ${receipt.goods_receipt_number}`} onClick={() => void openReceipt(receipt.id, "edit")} size="icon-sm" variant="ghost"><Edit3 className="size-4" /></Button> : null}
            {canReceive && receipt.status === "draft" && receipt.created_by !== userId && receipt.prepared_by !== userId ? <Button aria-label={`Post ${receipt.goods_receipt_number}`} onClick={() => void openReceipt(receipt.id, "post")} size="icon-sm" variant="ghost"><CheckCircle2 className="size-4" /></Button> : null}
          </div></TD>
        </TR>)}</TBody>
      </Table></TableScroll>}
    </TableWrap>

    <GoodsReceiptDrawer drawer={drawer} onClose={() => setDrawer(null)} onSaved={() => { setDrawer(null); void load(); }} />
    <GoodsReceiptDetailDrawer canReceive={canReceive} currentUserId={userId} receiptId={detailId} onClose={() => setDetailId(null)} onEdit={(receipt) => { setDetailId(null); setDrawer({ mode: "edit", receipt }); }} onPost={(receipt) => { setDetailId(null); setPostReceipt(receipt); }} />
    <PostReceiptDrawer onClose={() => setPostReceipt(null)} onDone={() => { setPostReceipt(null); void load(); }} receipt={postReceipt} />
  </div>;
}

function GoodsReceiptDrawer({ drawer, onClose, onSaved }: { drawer: ReceiptDrawerState | null; onClose: () => void; onSaved: () => void }) {
  const receipt = drawer?.receipt ?? null;
  const orders = useReceivableOrders(drawer !== null);
  const [purchaseOrderId, setPurchaseOrderId] = useState<string | null>(null);
  const [purchaseOrder, setPurchaseOrder] = useState<PurchaseOrder | null>(null);
  const [sourceLoading, setSourceLoading] = useState(false);
  const [sourceError, setSourceError] = useState<string | null>(null);
  const [receivedAt, setReceivedAt] = useState("");
  const [deliveryNoteNumber, setDeliveryNoteNumber] = useState("");
  const [notes, setNotes] = useState("");
  const [lines, setLines] = useState<EditableReceiptLine[]>([]);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    if (!drawer) return;
    setPurchaseOrderId(receipt?.purchase_order_id ?? null);
    setPurchaseOrder(null);
    setReceivedAt(receipt?.received_on ?? localDate());
    setDeliveryNoteNumber(receipt?.delivery_reference ?? "");
    setNotes(receipt?.notes ?? "");
    setLines([]);
    setSourceError(null);
  }, [drawer, receipt]);

  useEffect(() => {
    if (!drawer || !purchaseOrderId) { setPurchaseOrder(null); setLines([]); return; }
    let active = true;
    setSourceLoading(true);
    setSourceError(null);
    void procurementService.readPurchaseOrder(purchaseOrderId).then((response) => {
      if (!active) return;
      if (!response.success || !response.data) throw new Error(responseMessage(response, "Purchase order could not be loaded"));
      setPurchaseOrder(response.data);
      setLines(receipt ? editReceiptLines(response.data.lines, receipt) : newReceiptLines(response.data.lines));
    }).catch((loadError) => {
      if (active) setSourceError(loadError instanceof Error ? loadError.message : "Purchase order could not be loaded");
    }).finally(() => { if (active) setSourceLoading(false); });
    return () => { active = false; };
  }, [drawer, purchaseOrderId, receipt]);

  const orderOptions = useMemo(() => {
    const options = orders.orders.map((order) => ({ id: order.id, value: order.purchase_order_number, label: order.supplier_name, description: order.status.replace("_", " ") }));
    if (receipt && !options.some((option) => option.id === receipt.purchase_order_id)) options.push({ id: receipt.purchase_order_id, value: receipt.purchase_order_number, label: receipt.supplier_name, description: "Current order" });
    return options;
  }, [orders.orders, receipt]);

  const updateLine = (key: string, patch: Partial<EditableReceiptLine>) => setLines((current) => current.map((line) => line.key === key ? { ...line, ...patch } : line));

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    if (!purchaseOrder) { toast.error("Choose a purchase order"); return; }
    let linePayload: GoodsReceiptLineInput[];
    try {
      linePayload = lines.filter((line) => line.included).map((line, index) => receiptLineInput(line, index));
      if (linePayload.length === 0) throw new Error("Include at least one delivered line");
    } catch (inputError) {
      toast.error(inputError instanceof Error ? inputError.message : "Check the receipt lines");
      return;
    }
    const payload: GoodsReceiptInput = {
      purchase_order_id: purchaseOrder.id,
      received_on: receivedAt,
      delivery_reference: optional(deliveryNoteNumber),
      notes: optional(notes),
      lines: linePayload,
    };
    setSaving(true);
    try {
      const response = receipt
        ? await procurementService.updateGoodsReceipt(receipt.id, { received_on: payload.received_on, delivery_reference: payload.delivery_reference, notes: payload.notes, lines: payload.lines, expected_version: receipt.version })
        : await procurementService.createGoodsReceipt({ ...payload, idempotency_key: crypto.randomUUID() });
      if (!response.success) throw new Error(responseMessage(response, "Goods receipt could not be saved"));
      toast.success("Goods receipt saved");
      onSaved();
    } catch (saveError) {
      toast.error(saveError instanceof Error ? saveError.message : "Goods receipt could not be saved");
    } finally {
      setSaving(false);
    }
  };

  return <DialogShell onClose={saving ? () => undefined : onClose} open={drawer !== null} panelClassName="sm:max-w-[760px]">
    <DialogHeader onClose={saving ? undefined : onClose} title={receipt ? `Edit ${receipt.goods_receipt_number}` : "New goods receipt"} />
    <form onSubmit={submit}>
      <DialogBody className="space-y-6">
        {orders.error ? <ErrorNotice message={orders.error} onRetry={orders.retry} /> : null}
        <section className="space-y-5" aria-labelledby="receipt-source-heading">
          <h3 className="text-sm font-semibold text-[var(--text-strong)]" id="receipt-source-heading">Delivery source</h3>
          <div><Label htmlFor="goods-receipt-order">Purchase order</Label><SearchableSelect allowClear={!receipt} className="mt-1.5" disabled={Boolean(receipt) || orders.loading} id="goods-receipt-order" loading={orders.loading} onChange={setPurchaseOrderId} options={orderOptions} placeholder="Choose an issued order" value={purchaseOrderId} /></div>
          <div className="grid gap-5 sm:grid-cols-2"><div><Label htmlFor="goods-receipt-received-on">Received on</Label><Input className="mt-1.5" id="goods-receipt-received-on" onChange={(event) => setReceivedAt(event.target.value)} required type="date" value={receivedAt} /></div><div><Label htmlFor="goods-receipt-delivery-reference">Delivery reference</Label><Input className="mt-1.5" id="goods-receipt-delivery-reference" maxLength={200} onChange={(event) => setDeliveryNoteNumber(event.target.value)} value={deliveryNoteNumber} /></div></div>
          <div><Label htmlFor="goods-receipt-notes">Notes</Label><Textarea className="mt-1.5 min-h-24" id="goods-receipt-notes" maxLength={2000} onChange={(event) => setNotes(event.target.value)} value={notes} /></div>
        </section>

        <section className="space-y-4" aria-labelledby="receipt-lines-heading">
          <h3 className="text-sm font-semibold text-[var(--text-strong)]" id="receipt-lines-heading">Delivered quantities</h3>
          {sourceLoading ? <div aria-label="Loading purchase order lines" className="flex items-center gap-2 rounded-[var(--radius-lg)] border border-[var(--border)] p-4 text-sm text-[var(--text-muted)]" role="status"><Loader2 className="size-4 animate-spin" />Loading order lines…</div> : sourceError ? <ErrorNotice message={sourceError} /> : !purchaseOrder ? <Notice icon={<PackageCheck />} text="Choose an issued purchase order to record delivered quantities." /> : lines.length === 0 ? <Notice icon={<CheckCircle2 />} text="This purchase order has no remaining quantity to receive." /> : <div className="space-y-3">
            {lines.map((line, index) => <div className="rounded-[var(--radius-lg)] border border-[var(--border)] bg-[var(--surface-muted)] p-4" key={line.key}>
              <div className="flex items-start gap-3"><input aria-label={`Include ${line.description}`} checked={line.included} className="mt-1 size-4 accent-[var(--brand-strong)]" onChange={(event) => updateLine(line.key, { included: event.target.checked })} type="checkbox" /><div className="min-w-0 flex-1"><p className="font-medium text-[var(--text-strong)]">{index + 1}. {line.description}</p><p className="mt-1 text-xs text-[var(--text-muted)]">Up to {formatQuantity(line.availableMinor, line.quantityScale)} {line.unitLabel || "units"} may be received</p></div></div>
              {line.included ? <div className="mt-4"><Label htmlFor={`goods-receipt-quantity-${line.key}`}>Received quantity</Label><Input className="mt-1.5" id={`goods-receipt-quantity-${line.key}`} inputMode="decimal" onChange={(event) => updateLine(line.key, { quantity: event.target.value })} required value={line.quantity} /></div> : null}
            </div>)}
          </div>}
        </section>
      </DialogBody>
      <DialogFooter><Button disabled={saving} onClick={onClose} type="button" variant="secondary">Cancel</Button><Button disabled={saving || sourceLoading || orders.loading} type="submit">{saving ? <Loader2 className="size-4 animate-spin" /> : null}{saving ? "Saving…" : "Save draft receipt"}</Button></DialogFooter>
    </form>
  </DialogShell>;
}

function GoodsReceiptDetailDrawer({ canReceive, currentUserId, receiptId, onClose, onEdit, onPost }: { canReceive: boolean; currentUserId?: string; receiptId: string | null; onClose: () => void; onEdit: (receipt: GoodsReceipt) => void; onPost: (receipt: GoodsReceipt) => void }) {
  const [receipt, setReceipt] = useState<GoodsReceipt | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    if (!receiptId) return;
    setLoading(true);
    setError(null);
    try {
      const response = await procurementService.readGoodsReceipt(receiptId);
      if (!response.success || !response.data) throw new Error(responseMessage(response, "Goods receipt could not be loaded"));
      setReceipt(response.data);
    } catch (loadError) {
      setError(loadError instanceof Error ? loadError.message : "Goods receipt could not be loaded");
    } finally {
      setLoading(false);
    }
  }, [receiptId]);

  useEffect(() => { setReceipt(null); if (receiptId) void load(); }, [load, receiptId]);
  return <DialogShell onClose={onClose} open={receiptId !== null} panelClassName="sm:max-w-[700px]">
    <DialogHeader onClose={onClose} title={receipt?.goods_receipt_number ?? "Goods receipt"} />
    <DialogBody className="space-y-6">
      {loading ? <div aria-label="Loading goods receipt" className="space-y-3" role="status"><div className="h-28 animate-pulse rounded-[var(--radius-lg)] bg-[var(--surface-sunken)]" /><div className="h-52 animate-pulse rounded-[var(--radius-lg)] bg-[var(--surface-sunken)]" /></div> : error ? <ErrorNotice message={error} onRetry={() => void load()} /> : receipt ? <>
        <div><div className="flex flex-wrap items-center gap-3"><h3 className="font-tabular text-xl font-semibold text-[var(--text-strong)]">{receipt.goods_receipt_number}</h3><GoodsReceiptStatusBadge status={receipt.status} /></div><p className="mt-2 text-sm text-[var(--text-muted)]">{receipt.purchase_order_number} · {receipt.supplier_name}</p></div>
        <div className="grid gap-4 sm:grid-cols-2"><Fact label="Received on" value={formatDate(receipt.received_on)} /><Fact label="Delivery reference" value={receipt.delivery_reference || "Not set"} /><Fact label="Supplier number" value={receipt.supplier_number} /><Fact label="Lines" value={String(receipt.line_count)} /></div>
        {receipt.status === "draft" && (receipt.created_by === currentUserId || receipt.prepared_by === currentUserId) ? <Notice icon={<PackageCheck />} text="A different Procurement receiver must post this goods receipt." /> : null}
        {receipt.notes ? <Fact label="Notes" value={receipt.notes} /> : null}
        <TableWrap><TableScroll><Table><THead><tr><TH>#</TH><TH>Item</TH><TH className="text-right">Received</TH></tr></THead><TBody>{receipt.lines.map((line) => <TR key={line.id}><TD className="font-tabular text-[var(--text-subtle)]">{line.line_number}</TD><TD className="min-w-56 font-medium text-[var(--text-strong)]">{line.description}</TD><TD className="whitespace-nowrap text-right font-tabular">{formatQuantity(line.quantity_minor, line.quantity_scale)} {line.unit_label || "units"}</TD></TR>)}</TBody></Table></TableScroll></TableWrap>
      </> : null}
    </DialogBody>
    <DialogFooter><Button data-autofocus="true" onClick={onClose} type="button" variant="secondary">Close</Button>{canReceive && receipt?.status === "draft" ? <Button onClick={() => onEdit(receipt)} type="button" variant="secondary">Edit</Button> : null}{canReceive && receipt?.status === "draft" && receipt.created_by !== currentUserId && receipt.prepared_by !== currentUserId ? <Button onClick={() => onPost(receipt)} type="button">Post receipt</Button> : null}</DialogFooter>
  </DialogShell>;
}

function PostReceiptDrawer({ receipt, onClose, onDone }: { receipt: GoodsReceipt | null; onClose: () => void; onDone: () => void }) {
  const [pending, setPending] = useState(false);
  const post = async () => {
    if (!receipt) return;
    setPending(true);
    try {
      const response = await procurementService.postGoodsReceipt(receipt.id, receipt.version);
      if (!response.success) throw new Error(responseMessage(response, "Goods receipt could not be posted"));
      toast.success("Goods receipt posted");
      onDone();
    } catch (postError) {
      toast.error(postError instanceof Error ? postError.message : "Goods receipt could not be posted");
    } finally {
      setPending(false);
    }
  };
  return <DialogShell onClose={pending ? () => undefined : onClose} open={receipt !== null}>
    <DialogHeader onClose={pending ? undefined : onClose} title="Post goods receipt?" />
    <DialogBody><Notice icon={<CheckCircle2 />} text={`Post ${receipt?.goods_receipt_number ?? "this receipt"}. The received quantities will update the purchase order and the receipt can no longer be edited.`} /></DialogBody>
    <DialogFooter><Button data-autofocus="true" disabled={pending} onClick={onClose} type="button" variant="secondary">Keep draft</Button><Button disabled={pending} onClick={() => void post()} type="button">{pending ? <Loader2 className="size-4 animate-spin" /> : null}{pending ? "Posting…" : "Post receipt"}</Button></DialogFooter>
  </DialogShell>;
}

type ReceivableOrdersState = { orders: PurchaseOrderSummary[]; loading: boolean; error: string | null; retry: () => void };

function useReceivableOrders(enabled: boolean): ReceivableOrdersState {
  const [retryKey, setRetryKey] = useState(0);
  const [state, setState] = useState<Omit<ReceivableOrdersState, "retry">>({ orders: [], loading: false, error: null });
  useEffect(() => {
    if (!enabled) return;
    let active = true;
    setState((current) => ({ ...current, loading: true, error: null }));
    void procurementService.listPurchaseOrders({ page: 1, per_page: 100 }).then((response) => {
      if (!active) return;
      if (!response.success || !response.data) throw new Error(responseMessage(response, "Receivable purchase orders could not be loaded"));
      setState({ orders: response.data.purchase_orders.filter((order) => order.status === "issued" || order.status === "partially_received"), loading: false, error: null });
    }).catch((loadError) => {
      if (active) setState((current) => ({ ...current, loading: false, error: loadError instanceof Error ? loadError.message : "Receivable purchase orders could not be loaded" }));
    });
    return () => { active = false; };
  }, [enabled, retryKey]);
  return { ...state, retry: () => setRetryKey((value) => value + 1) };
}

type EditableReceiptLine = { key: string; purchaseOrderLineId: string; description: string; unitLabel: string | null; included: boolean; quantity: string; quantityScale: number; availableMinor: number };

function newReceiptLines(lines: PurchaseOrderLine[]): EditableReceiptLine[] {
  return lines.filter((line) => line.remaining_quantity_minor > 0).map((line) => ({ key: line.id, purchaseOrderLineId: line.id, description: line.description, unitLabel: line.unit_label, included: true, quantity: exactAmount(line.remaining_quantity_minor, line.quantity_scale), quantityScale: line.quantity_scale, availableMinor: line.remaining_quantity_minor }));
}

function editReceiptLines(lines: PurchaseOrderLine[], receipt: GoodsReceipt): EditableReceiptLine[] {
  return lines.map((line) => {
    const existing = receipt.lines.find((receiptLine) => receiptLine.purchase_order_line_id === line.id);
    const available = line.remaining_quantity_minor;
    return { key: line.id, purchaseOrderLineId: line.id, description: line.description, unitLabel: line.unit_label, included: Boolean(existing), quantity: exactAmount(existing?.quantity_minor ?? available, line.quantity_scale), quantityScale: line.quantity_scale, availableMinor: available };
  }).filter((line) => line.availableMinor > 0);
}

function receiptLineInput(line: EditableReceiptLine, index: number): GoodsReceiptLineInput {
  const quantity = parseScaled(line.quantity, line.quantityScale);
  if (quantity === null || quantity < 1) throw new Error(`Line ${index + 1} needs a valid received quantity`);
  if (quantity > line.availableMinor) throw new Error(`Line ${index + 1} exceeds the remaining order quantity`);
  return { purchase_order_line_id: line.purchaseOrderLineId, quantity_minor: quantity, quantity_scale: line.quantityScale };
}

function GoodsReceiptStatusBadge({ status }: { status: GoodsReceiptStatus }) { return <Badge tone={status === "posted" ? "success" : "warning"}>{status}</Badge>; }
function ErrorNotice({ message, onRetry }: { message: string; onRetry?: () => void }) { return <div className="flex flex-col gap-3 rounded-[var(--radius-lg)] border border-[var(--tone-danger-bd)] bg-[var(--tone-danger-bg)] p-4 sm:flex-row sm:items-center"><div className="flex min-w-0 flex-1 gap-3 text-[var(--tone-danger)]"><TriangleAlert className="mt-0.5 size-5 shrink-0" /><p className="text-sm leading-5">{message}</p></div>{onRetry ? <Button onClick={onRetry} type="button" variant="secondary">Try again</Button> : null}</div>; }
function Notice({ icon, text }: { icon: ReactNode; text: string }) { return <div className="flex gap-3 rounded-[var(--radius-lg)] border border-[var(--border)] bg-[var(--surface-muted)] p-4 text-[var(--text-muted)]"><span className="mt-0.5 shrink-0 [&_svg]:size-5">{icon}</span><p className="text-sm leading-6">{text}</p></div>; }
function Fact({ label, value }: { label: string; value: string }) { return <div className="rounded-[var(--radius-lg)] border border-[var(--border)] bg-[var(--surface-muted)] p-4"><p className="text-[11px] font-semibold uppercase tracking-[0.12em] text-[var(--text-subtle)]">{label}</p><p className="mt-1 break-words text-sm text-[var(--text-strong)]">{value}</p></div>; }
function optional(value: string) { return value.trim() || null; }
function parseScaled(value: string, scale: number) { const normalized = value.trim(); if (!/^\d+(\.\d*)?$/.test(normalized)) return null; const [whole, fraction = ""] = normalized.split("."); if (fraction.length > scale) return null; const parsed = Number(`${whole}${fraction.padEnd(scale, "0")}`); return Number.isSafeInteger(parsed) ? parsed : null; }
function exactAmount(valueMinor: number, scale: number) { const value = String(Math.abs(Math.trunc(valueMinor))).padStart(scale + 1, "0"); const sign = valueMinor < 0 ? "-" : ""; return scale === 0 ? `${sign}${value}` : `${sign}${value.slice(0, -scale)}.${value.slice(-scale)}`; }
function formatQuantity(valueMinor: number, scale: number) { return exactAmount(valueMinor, scale).replace(/(\.\d*?)0+$/, "$1").replace(/\.$/, ""); }
function formatDate(value: string) { return new Intl.DateTimeFormat(undefined, { dateStyle: "medium", timeZone: "UTC" }).format(new Date(`${value}T00:00:00Z`)); }
function localDate() { const date = new Date(); const offset = date.getTimezoneOffset() * 60_000; return new Date(date.getTime() - offset).toISOString().slice(0, 10); }

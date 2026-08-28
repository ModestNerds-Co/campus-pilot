/** Posted Procurement receipt allocation queue and exact mapping drawer. */

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { FormEvent } from "react";
import { Loader2, PackageCheck, Search, TriangleAlert } from "lucide-react";
import toast from "react-hot-toast";

import { SearchableSelect } from "@/components/searchable-select";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Table, TableControlsBar, TableControlsPagination, TableControlsSearch, TableEmpty,
  TableError, TableLoading, TableScroll, TableWrap, TBody, TD, TH, THead, TR,
} from "@/components/ui/data-table";
import { DialogBody, DialogFooter, DialogHeader, DialogShell } from "@/components/ui/dialog";
import { Input, Label, Textarea } from "@/components/ui/input";
import { usePageChrome } from "@/modules/admin/layouts/page-chrome";
import { hasPermission } from "@/modules/users/access-control";
import { useAuthStore } from "@/stores/auth-store";

import { createIdempotencyKeyLifecycle } from "./create-idempotency-key";
import { loadAllStockReferences } from "./reference-pages";
import { assetsInventoryService, responseMessage } from "./service";
import { exactStockQuantity, formatStockQuantity, parseStockQuantity } from "./stock-quantity";
import { StockNotice, formatOperationalDate } from "./stock-ui";
import type { GoodsReceiptAllocationLine, GoodsReceiptAllocationSource } from "./stock-types";
import type { InventoryItem, InventoryStore } from "./types";

export function ProcurementReceiptsWorkspace() {
  const procurementEnabled = useAuthStore((state) => state.user?.modules.includes("procurement") ?? false);
  const canAllocate = useAuthStore((state) => hasPermission(state.user?.permissions, "assets_inventory:receive"));
  const [receipts, setReceipts] = useState<GoodsReceiptAllocationSource[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [search, setSearch] = useState("");
  const [submittedSearch, setSubmittedSearch] = useState("");
  const [page, setPage] = useState(1);
  const [totalPages, setTotalPages] = useState(1);
  const [drawer, setDrawer] = useState<GoodsReceiptAllocationSource | null>(null);

  const load = useCallback(async () => {
    if (!procurementEnabled) {
      setReceipts([]);
      setLoading(false);
      setError(null);
      return;
    }
    setLoading(true);
    setError(null);
    try {
      const response = await assetsInventoryService.listGoodsReceiptAllocations({ page, per_page: 5, search: submittedSearch || undefined });
      if (!response.success || !response.data) throw new Error(responseMessage(response, "Procurement receipts could not be loaded"));
      setReceipts(response.data.goods_receipts);
      setTotalPages(response.pagination?.total_pages ?? 1);
    } catch (loadError) {
      setError(loadError instanceof Error ? loadError.message : "Procurement receipts could not be loaded");
    } finally {
      setLoading(false);
    }
  }, [page, procurementEnabled, submittedSearch]);

  useEffect(() => { void load(); }, [load]);
  usePageChrome("Procurement receipts");

  if (!procurementEnabled) return <div className="space-y-5"><p className="text-sm text-[var(--text-muted)]">Allocate posted goods receipts to inventory items and stores.</p><StockNotice><span className="flex items-start gap-3"><PackageCheck className="mt-0.5 size-5 shrink-0 text-[var(--brand-strong)]" /><span><strong className="block text-[var(--text-strong)]">Procurement is not enabled for this campus</strong><span className="mt-1 block">Posted receipt allocation is available when the Procurement module is enabled.</span></span></span></StockNotice></div>;

  return <div className="space-y-5">
    <p className="text-sm text-[var(--text-muted)]">Allocate posted goods receipts to inventory items and stores.</p>
    <TableControlsBar>
      <TableControlsSearch onSubmit={(event) => { event.preventDefault(); setPage(1); setSubmittedSearch(search.trim()); }}>
        <Input aria-label="Search Procurement receipts" leadingIcon={<Search />} onChange={(event) => setSearch(event.target.value)} placeholder="Search receipt or purchase order…" value={search} />
        <Button type="submit" variant="secondary">Search</Button>
      </TableControlsSearch>
      {!loading && receipts.length > 0 ? <TableControlsPagination onNext={() => setPage((value) => Math.min(totalPages, value + 1))} onPrevious={() => setPage((value) => Math.max(1, value - 1))} page={page} totalPages={totalPages} /> : null}
    </TableControlsBar>
    <TableWrap>
      {loading ? <TableLoading columns={7} label="Loading Procurement receipts…" /> : error ? <TableError description={error} onRetry={() => void load()} title="Procurement receipts are unavailable" /> : receipts.length === 0 ? <TableEmpty description={submittedSearch ? "Change the current search." : "No posted receipts are waiting for allocation."} icon={<PackageCheck />} title={submittedSearch ? "No receipts match this search" : "No receipts to allocate"} /> : <TableScroll><Table>
        <THead><tr><TH>Receipt</TH><TH>Purchase order</TH><TH>Supplier</TH><TH>Received</TH><TH>Lines remaining</TH><TH>Status</TH><TH className="text-right">Action</TH></tr></THead>
        <TBody>{receipts.map((receipt) => {
          const totalLines = receipt.lines.length;
          const remainingLines = receipt.lines.filter((line) => line.remaining_quantity_minor > 0).length;
          const status = remainingLines === 0 ? "Allocated" : remainingLines === totalLines ? "Waiting" : "Partially allocated";
          return <TR key={receipt.id}>
            <TD><p className="font-tabular font-medium text-[var(--text-strong)]">{receipt.goods_receipt_number}</p><p className="mt-1 text-xs text-[var(--text-subtle)]">{receipt.delivery_reference || "No delivery reference"}</p></TD>
            <TD className="font-tabular text-[var(--text-muted)]">{receipt.purchase_order_number}</TD>
            <TD><p className="font-medium text-[var(--text-body)]">{receipt.supplier_name}</p><p className="mt-1 font-tabular text-xs text-[var(--text-subtle)]">{receipt.supplier_number}</p></TD>
            <TD className="whitespace-nowrap">{formatOperationalDate(receipt.received_on)}</TD>
            <TD className="font-tabular text-[var(--text-muted)]">{remainingLines}</TD>
            <TD><Badge tone={remainingLines === 0 ? "success" : remainingLines === totalLines ? "warning" : "brand"}>{status}</Badge></TD>
            <TD className="text-right">{remainingLines > 0 && canAllocate ? <Button onClick={() => setDrawer(receipt)} size="sm" variant="secondary">Allocate</Button> : null}</TD>
          </TR>;
        })}</TBody>
      </Table></TableScroll>}
    </TableWrap>
    <AllocationDrawer onClose={() => setDrawer(null)} onSaved={() => { setDrawer(null); void load(); }} receipt={drawer} />
  </div>;
}

type EditableAllocationLine = {
  source: GoodsReceiptAllocationLine;
  included: boolean;
  itemId: string | null;
  storeId: string | null;
  quantity: string;
};

function AllocationDrawer({ onClose, onSaved, receipt }: { onClose: () => void; onSaved: () => void; receipt: GoodsReceiptAllocationSource | null }) {
  const createKey = useRef(createIdempotencyKeyLifecycle());
  const references = useAllocationReferences(receipt !== null);
  const [lines, setLines] = useState<EditableAllocationLine[]>([]);
  const [effectiveOn, setEffectiveOn] = useState(today());
  const [reason, setReason] = useState("");
  const [dirty, setDirty] = useState(false);
  const [discarding, setDiscarding] = useState(false);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    if (!receipt) return;
    setLines(receipt.lines.filter((line) => line.remaining_quantity_minor > 0).map((source) => ({ source, included: false, itemId: source.mapped_item_id, storeId: null, quantity: exactStockQuantity(source.remaining_quantity_minor, source.quantity_scale) })));
    setEffectiveOn(today());
    setReason("");
    setDirty(false);
    setDiscarding(false);
    createKey.current.startFresh();
  }, [receipt]);

  const storeOptions = useMemo(() => references.stores.map((store) => ({ id: store.id, value: store.store_number, label: store.name, description: store.location_label ?? undefined })), [references.stores]);
  const updateLine = (lineId: string, patch: Partial<EditableAllocationLine>) => { setLines((current) => current.map((line) => line.source.id === lineId ? { ...line, ...patch } : line)); setDirty(true); };
  const requestClose = () => {
    if (saving) return;
    if (discarding) { setDiscarding(false); return; }
    if (dirty) { setDiscarding(true); return; }
    onClose();
  };
  const submit = async (event: FormEvent) => {
    event.preventDefault();
    if (!receipt) return;
    const included = lines.filter((line) => line.included);
    if (included.length === 0) { toast.error("Select at least one receipt line"); return; }
    try {
      const payloadLines = included.map((line) => {
        const item = references.items.find((candidate) => candidate.id === line.itemId);
        if (!item || !line.storeId) throw new Error(`Choose an item and store for line ${line.source.line_number}`);
        if (line.source.unit_label === null) throw new Error(`Line ${line.source.line_number} has no unit and cannot be allocated`);
        if (item.quantity_scale !== line.source.quantity_scale || normalizeUnit(item.unit_label) !== normalizeUnit(line.source.unit_label)) throw new Error(`Line ${line.source.line_number} does not match the selected item's unit and precision`);
        const quantityMinor = parseStockQuantity(line.quantity, line.source.quantity_scale);
        if (quantityMinor === null || quantityMinor > line.source.remaining_quantity_minor) throw new Error(`Check the quantity for line ${line.source.line_number}`);
        return { goods_receipt_line_id: line.source.id, item_id: item.id, store_id: line.storeId, quantity_minor: quantityMinor };
      });
      setSaving(true);
      const response = await assetsInventoryService.createGoodsReceiptAllocation({ goods_receipt_id: receipt.id, effective_on: effectiveOn, reason: optional(reason), idempotency_key: createKey.current.current(), lines: payloadLines });
      if (!response.success || !response.data) throw new Error(responseMessage(response, "Receipt allocation could not be posted"));
      createKey.current.startFresh();
      setDirty(false);
      toast.success("Receipt allocation posted");
      onSaved();
    } catch (allocationError) {
      toast.error(allocationError instanceof Error ? allocationError.message : "Receipt allocation could not be posted");
    } finally {
      setSaving(false);
    }
  };

  return <DialogShell onClose={requestClose} open={receipt !== null} panelClassName="sm:max-w-[760px]">
    {discarding ? <><DialogHeader onClose={requestClose} title="Discard allocation?" /><DialogBody><StockNotice danger><span className="flex gap-3"><TriangleAlert className="mt-0.5 size-5 shrink-0" />The receipt mappings entered here will be lost.</span></StockNotice></DialogBody><DialogFooter><Button autoFocus data-autofocus="true" onClick={() => setDiscarding(false)} type="button" variant="secondary">Keep editing</Button><Button onClick={() => { createKey.current.startFresh(); setDirty(false); onClose(); }} type="button" variant="destructive">Discard changes</Button></DialogFooter></> : <>
      <DialogHeader onClose={saving ? undefined : requestClose} title={`Allocate ${receipt?.goods_receipt_number ?? "receipt"}`} />
      <form onSubmit={submit}>
        <DialogBody className="space-y-6">
          <StockNotice>Only posted receipt quantities are shown. Each selected item must use the same unit and quantity precision as its Procurement line.</StockNotice>
          {references.error ? <StockNotice danger>{references.error}</StockNotice> : null}
          <section className="grid gap-4 sm:grid-cols-3" aria-label="Receipt source"><div><p className="text-xs font-medium text-[var(--text-subtle)]">Purchase order</p><p className="mt-1 font-tabular text-sm font-semibold text-[var(--text-strong)]">{receipt?.purchase_order_number}</p></div><div><p className="text-xs font-medium text-[var(--text-subtle)]">Received</p><p className="mt-1 text-sm font-semibold text-[var(--text-strong)]">{receipt ? formatOperationalDate(receipt.received_on) : "—"}</p></div><div><p className="text-xs font-medium text-[var(--text-subtle)]">Delivery reference</p><p className="mt-1 text-sm font-semibold text-[var(--text-strong)]">{receipt?.delivery_reference || "Not set"}</p></div></section>
          <section className="space-y-3" aria-labelledby="allocation-lines-heading">
            <h3 className="text-sm font-semibold text-[var(--text-strong)]" id="allocation-lines-heading">Receipt lines</h3>
            {lines.map((line) => {
              const compatibleItems = references.items.filter((item) => item.quantity_scale === line.source.quantity_scale && line.source.unit_label !== null && normalizeUnit(item.unit_label) === normalizeUnit(line.source.unit_label));
              const itemOptions = compatibleItems.map((item) => ({ id: item.id, value: item.item_number, label: item.name, description: item.unit_label }));
              return <div className="rounded-[var(--radius-lg)] border border-[var(--border)] bg-[var(--surface-muted)] p-4" key={line.source.id}>
                <label className="flex items-start gap-3"><input aria-label={`Allocate line ${line.source.line_number}`} checked={line.included} className="mt-1 size-4 accent-[var(--brand-strong)]" disabled={line.source.unit_label === null} onChange={(event) => updateLine(line.source.id, { included: event.target.checked })} type="checkbox" /><span className="min-w-0"><span className="block font-medium text-[var(--text-strong)]">{line.source.line_number}. {line.source.description}</span><span className="mt-1 block text-xs text-[var(--text-muted)]">Received {formatStockQuantity(line.source.quantity_minor, line.source.quantity_scale)} · Allocated {formatStockQuantity(line.source.allocated_quantity_minor, line.source.quantity_scale)} · Remaining {formatStockQuantity(line.source.remaining_quantity_minor, line.source.quantity_scale)} {line.source.unit_label || "unit not supplied"}</span></span></label>
                {line.source.unit_label === null ? <div className="mt-4"><StockNotice danger>This line has no unit and cannot be allocated.</StockNotice></div> : line.included ? <div className="mt-4 grid gap-4">
                  <div><Label htmlFor={`allocation-item-${line.source.id}`}>Inventory item</Label><SearchableSelect allowClear={false} className="mt-1.5" disabled={saving || references.loading} id={`allocation-item-${line.source.id}`} loading={references.loading} onChange={(value) => updateLine(line.source.id, { itemId: value })} options={itemOptions} placeholder={itemOptions.length ? "Choose a matching item" : "No matching active item"} value={line.itemId} /></div>
                  <div><Label htmlFor={`allocation-store-${line.source.id}`}>Store</Label><SearchableSelect allowClear={false} className="mt-1.5" disabled={saving || references.loading} id={`allocation-store-${line.source.id}`} loading={references.loading} onChange={(value) => updateLine(line.source.id, { storeId: value })} options={storeOptions} placeholder="Choose an active store" value={line.storeId} /></div>
                  <div><Label htmlFor={`allocation-quantity-${line.source.id}`}>Quantity</Label><Input className="mt-1.5 font-tabular" id={`allocation-quantity-${line.source.id}`} inputMode="decimal" onChange={(event) => updateLine(line.source.id, { quantity: event.target.value })} required value={line.quantity} /></div>
                </div> : null}
              </div>;
            })}
          </section>
          <div><Label htmlFor="allocation-effective-on">Effective date</Label><Input className="mt-1.5 sm:max-w-xs" id="allocation-effective-on" onChange={(event) => { setEffectiveOn(event.target.value); setDirty(true); }} required type="date" value={effectiveOn} /></div>
          <div><Label htmlFor="allocation-reason">Reason</Label><Textarea className="mt-1.5 min-h-24" id="allocation-reason" maxLength={2000} onChange={(event) => { setReason(event.target.value); setDirty(true); }} value={reason} /></div>
        </DialogBody>
        <DialogFooter><Button disabled={saving} onClick={requestClose} type="button" variant="secondary">Cancel</Button><Button disabled={saving || references.loading} type="submit">{saving ? <Loader2 className="size-4 animate-spin" /> : null}{saving ? "Posting…" : "Post allocation"}</Button></DialogFooter>
      </form>
    </>}
  </DialogShell>;
}

function useAllocationReferences(enabled: boolean) {
  const [items, setItems] = useState<InventoryItem[]>([]);
  const [stores, setStores] = useState<InventoryStore[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    if (!enabled) return;
    let active = true;
    setLoading(true);
    setError(null);
    void Promise.all([
      loadAllStockReferences("Active inventory items", async (page, perPage) => {
        const response = await assetsInventoryService.listItems({ page, per_page: perPage, status: "active" });
        if (!response.success || !response.data) throw new Error(responseMessage(response, "Items could not be loaded"));
        return {
          records: response.data.items,
          total: response.pagination?.total ?? response.data.items.length,
          totalPages: response.pagination?.total_pages ?? 1,
        };
      }),
      loadAllStockReferences("Active inventory stores", async (page, perPage) => {
        const response = await assetsInventoryService.listStores({ page, per_page: perPage, status: "active" });
        if (!response.success || !response.data) throw new Error(responseMessage(response, "Stores could not be loaded"));
        return {
          records: response.data.stores,
          total: response.pagination?.total ?? response.data.stores.length,
          totalPages: response.pagination?.total_pages ?? 1,
        };
      }),
    ]).then(([loadedItems, loadedStores]) => {
      if (!active) return;
      setItems(loadedItems);
      setStores(loadedStores);
    }).catch((referenceError) => { if (active) setError(referenceError instanceof Error ? referenceError.message : "Allocation references could not be loaded"); }).finally(() => { if (active) setLoading(false); });
    return () => { active = false; };
  }, [enabled]);
  return { items, stores, loading, error };
}

function normalizeUnit(value: string) { return value.trim().toLowerCase(); }
function optional(value: string) { return value.trim() || null; }
function today() { const now = new Date(); const offset = now.getTimezoneOffset() * 60_000; return new Date(now.getTime() - offset).toISOString().slice(0, 10); }

/** Manual stock posting drawer with exact quantities and interruption-safe retries. */

import { useEffect, useMemo, useRef, useState } from "react";
import type { FormEvent } from "react";
import { Loader2, TriangleAlert } from "lucide-react";
import toast from "react-hot-toast";

import { SearchableSelect } from "@/components/searchable-select";
import { Button } from "@/components/ui/button";
import { DialogBody, DialogFooter, DialogHeader, DialogShell } from "@/components/ui/dialog";
import { Input, Label, Select, Textarea } from "@/components/ui/input";

import { createIdempotencyKeyLifecycle } from "./create-idempotency-key";
import { loadAllStockReferences } from "./reference-pages";
import { assetsInventoryService, responseMessage } from "./service";
import { formatStockQuantity, parseStockQuantity, quantityScaleLabel } from "./stock-quantity";
import { StockNotice, movementKindLabel } from "./stock-ui";
import type { StockBalance } from "./stock-types";
import type { InventoryItem, InventoryStore } from "./types";

export type ManualMovementKind = "manual_receipt" | "issue" | "transfer" | "adjustment";

export interface MovementDrawerSeed {
  kind?: ManualMovementKind;
  itemId?: string;
  storeId?: string;
}

export function RecordMovementDrawer({
  allowedKinds,
  onClose,
  onSaved,
  open,
  seed,
}: {
  allowedKinds: ManualMovementKind[];
  onClose: () => void;
  onSaved: (movementId: string) => void;
  open: boolean;
  seed?: MovementDrawerSeed | null;
}) {
  const createKey = useRef(createIdempotencyKeyLifecycle());
  const [kind, setKind] = useState<ManualMovementKind>("manual_receipt");
  const [itemId, setItemId] = useState<string | null>(null);
  const [storeId, setStoreId] = useState<string | null>(null);
  const [destinationStoreId, setDestinationStoreId] = useState<string | null>(null);
  const [quantity, setQuantity] = useState("");
  const [countedQuantity, setCountedQuantity] = useState("");
  const [effectiveOn, setEffectiveOn] = useState(today());
  const [reference, setReference] = useState("");
  const [reason, setReason] = useState("");
  const [dirty, setDirty] = useState(false);
  const [discarding, setDiscarding] = useState(false);
  const [saving, setSaving] = useState(false);
  const references = useStockReferences(open);
  const balance = useSelectedBalance(open && kind !== "manual_receipt", itemId, storeId);
  const allowedKindsKey = allowedKinds.join("|");

  useEffect(() => {
    if (!open) return;
    const nextKind = seed?.kind && allowedKinds.includes(seed.kind) ? seed.kind : allowedKinds[0] ?? "manual_receipt";
    setKind(nextKind);
    setItemId(seed?.itemId ?? null);
    setStoreId(seed?.storeId ?? null);
    setDestinationStoreId(null);
    setQuantity("");
    setCountedQuantity("");
    setEffectiveOn(today());
    setReference("");
    setReason("");
    setDirty(false);
    setDiscarding(false);
    createKey.current.startFresh();
  }, [allowedKindsKey, open, seed?.itemId, seed?.kind, seed?.storeId]);

  const selectedItem = references.items.find((item) => item.id === itemId) ?? null;
  const itemOptions = useMemo(() => references.items.map((item) => ({ id: item.id, value: item.item_number, label: item.name, description: `${item.unit_label} · ${quantityScaleLabel(item.quantity_scale)}` })), [references.items]);
  const storeOptions = useMemo(() => references.stores.map((store) => ({ id: store.id, value: store.store_number, label: store.name, description: store.location_label ?? undefined })), [references.stores]);

  const mark = <T,>(setter: (value: T) => void, value: T) => { setter(value); setDirty(true); };
  const requestClose = () => {
    if (saving) return;
    if (discarding) { setDiscarding(false); return; }
    if (dirty) { setDiscarding(true); return; }
    onClose();
  };
  const discard = () => {
    createKey.current.startFresh();
    setDirty(false);
    setDiscarding(false);
    onClose();
  };

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    if (!selectedItem || !storeId) { toast.error("Choose an item and store"); return; }
    if (kind === "transfer" && (!destinationStoreId || destinationStoreId === storeId)) { toast.error("Choose a different destination store"); return; }
    const parsedQuantity = kind === "adjustment" ? null : parseStockQuantity(quantity, selectedItem.quantity_scale);
    const parsedCounted = kind === "adjustment" ? parseStockQuantity(countedQuantity, selectedItem.quantity_scale, true) : null;
    if (kind !== "adjustment" && parsedQuantity === null) { toast.error(`Quantity must be positive with at most ${selectedItem.quantity_scale} decimal places`); return; }
    if (kind === "adjustment" && parsedCounted === null) { toast.error(`Counted quantity must have at most ${selectedItem.quantity_scale} decimal places`); return; }
    if (kind === "adjustment" && (balance.loading || balance.error)) { toast.error(balance.error ?? "Wait for the current balance to load"); return; }
    if (kind === "adjustment" && !reason.trim()) { toast.error("Enter the reason for this adjustment"); return; }
    const expectedMinor = balance.value?.on_hand_minor ?? 0;
    if (kind === "adjustment" && parsedCounted === expectedMinor) { toast.error("Counted quantity must differ from the current balance"); return; }

    const header = { effective_on: effectiveOn, reference: optional(reference), reason: optional(reason), idempotency_key: createKey.current.current() };
    setSaving(true);
    try {
      const response = kind === "manual_receipt"
        ? await assetsInventoryService.createManualReceipt({ ...header, lines: [{ item_id: selectedItem.id, store_id: storeId, quantity_minor: parsedQuantity! }] })
        : kind === "issue"
          ? await assetsInventoryService.createIssue({ ...header, lines: [{ item_id: selectedItem.id, store_id: storeId, quantity_minor: parsedQuantity! }] })
          : kind === "transfer"
            ? await assetsInventoryService.createTransfer({ ...header, lines: [{ item_id: selectedItem.id, from_store_id: storeId, to_store_id: destinationStoreId!, quantity_minor: parsedQuantity! }] })
            : await assetsInventoryService.createAdjustment({ ...header, reason: reason.trim(), lines: [{ item_id: selectedItem.id, store_id: storeId, expected_on_hand_minor: expectedMinor, counted_on_hand_minor: parsedCounted! }] });
      if (!response.success || !response.data) throw new Error(responseMessage(response, "Stock movement could not be posted"));
      createKey.current.startFresh();
      setDirty(false);
      toast.success(`${movementKindLabel(kind)} posted`);
      onSaved(response.data.id);
    } catch (postError) {
      toast.error(postError instanceof Error ? postError.message : "Stock movement could not be posted");
    } finally {
      setSaving(false);
    }
  };

  return <DialogShell onClose={requestClose} open={open}>
    {discarding ? <>
      <DialogHeader onClose={requestClose} title="Discard movement?" />
      <DialogBody><StockNotice danger><span className="flex gap-3"><TriangleAlert className="mt-0.5 size-5 shrink-0" />The values entered in this movement will be lost.</span></StockNotice></DialogBody>
      <DialogFooter><Button autoFocus data-autofocus="true" onClick={() => setDiscarding(false)} type="button" variant="secondary">Keep editing</Button><Button onClick={discard} type="button" variant="destructive">Discard changes</Button></DialogFooter>
    </> : <>
      <DialogHeader onClose={saving ? undefined : requestClose} title="Record movement" />
      <form onSubmit={submit}>
        <DialogBody className="space-y-6">
          {allowedKinds.length === 0 ? <StockNotice>No stock movement actions are available with your current access.</StockNotice> : null}
          {references.error ? <StockNotice danger>{references.error}</StockNotice> : null}
          <div><Label htmlFor="stock-movement-kind">Movement type</Label><Select className="mt-1.5" data-autofocus="true" disabled={saving || allowedKinds.length === 0} id="stock-movement-kind" onChange={(event) => { mark(setKind, event.target.value as ManualMovementKind); setDestinationStoreId(null); setQuantity(""); setCountedQuantity(""); }} value={kind}>{allowedKinds.map((value) => <option key={value} value={value}>{movementKindLabel(value)}</option>)}</Select></div>
          <div><Label htmlFor="stock-movement-item">Item</Label><SearchableSelect allowClear={false} className="mt-1.5" disabled={saving || references.loading} id="stock-movement-item" loading={references.loading} onChange={(value) => { mark(setItemId, value); setQuantity(""); setCountedQuantity(""); }} options={itemOptions} placeholder="Choose an active item" value={itemId} /></div>
          <div><Label htmlFor="stock-movement-store">{kind === "transfer" ? "Source store" : kind === "manual_receipt" ? "Destination store" : "Store"}</Label><SearchableSelect allowClear={false} className="mt-1.5" disabled={saving || references.loading} id="stock-movement-store" loading={references.loading} onChange={(value) => mark(setStoreId, value)} options={storeOptions} placeholder="Choose an active store" value={storeId} /></div>
          {kind === "transfer" ? <div><Label htmlFor="stock-movement-destination">Destination store</Label><SearchableSelect allowClear={false} className="mt-1.5" disabled={saving || references.loading} id="stock-movement-destination" loading={references.loading} onChange={(value) => mark(setDestinationStoreId, value)} options={storeOptions.filter((option) => option.id !== storeId)} placeholder="Choose a destination" value={destinationStoreId} /></div> : null}
          {kind !== "manual_receipt" && itemId && storeId ? <BalanceNotice balance={balance.value} error={balance.error} item={selectedItem} loading={balance.loading} /> : null}
          {kind === "adjustment" ? <div><Label htmlFor="stock-movement-counted">Counted quantity</Label><Input className="mt-1.5 font-tabular" id="stock-movement-counted" inputMode="decimal" onChange={(event) => mark(setCountedQuantity, event.target.value)} required value={countedQuantity} /></div> : <div><Label htmlFor="stock-movement-quantity">Quantity</Label><Input className="mt-1.5 font-tabular" id="stock-movement-quantity" inputMode="decimal" onChange={(event) => mark(setQuantity, event.target.value)} required value={quantity} /></div>}
          <div><Label htmlFor="stock-movement-effective-on">Effective date</Label><Input className="mt-1.5 sm:max-w-xs" id="stock-movement-effective-on" onChange={(event) => mark(setEffectiveOn, event.target.value)} required type="date" value={effectiveOn} /></div>
          <div><Label htmlFor="stock-movement-reference">Reference</Label><Input className="mt-1.5" id="stock-movement-reference" maxLength={200} onChange={(event) => mark(setReference, event.target.value)} value={reference} /></div>
          <div><Label htmlFor="stock-movement-reason">Reason{kind === "adjustment" ? " (required)" : ""}</Label><Textarea className="mt-1.5 min-h-24" id="stock-movement-reason" maxLength={2000} onChange={(event) => mark(setReason, event.target.value)} required={kind === "adjustment"} value={reason} /></div>
        </DialogBody>
        <DialogFooter><Button disabled={saving} onClick={requestClose} type="button" variant="secondary">Cancel</Button><Button disabled={saving || references.loading || allowedKinds.length === 0} type="submit">{saving ? <Loader2 className="size-4 animate-spin" /> : null}{saving ? "Posting…" : `Post ${movementKindLabel(kind).toLowerCase()}`}</Button></DialogFooter>
      </form>
    </>}
  </DialogShell>;
}

function BalanceNotice({ balance, error, item, loading }: { balance: StockBalance | null; error: string | null; item: InventoryItem | null; loading: boolean }) {
  if (loading) return <StockNotice><span className="flex items-center gap-2"><Loader2 className="size-4 animate-spin" />Loading current balance…</span></StockNotice>;
  if (error) return <StockNotice danger>{error}</StockNotice>;
  if (!item) return null;
  return <StockNotice>{balance ? <>On hand: <strong className="font-tabular text-[var(--text-strong)]">{formatStockQuantity(balance.on_hand_minor, item.quantity_scale)} {item.unit_label}</strong></> : "No stock balance has been posted for this item and store."}</StockNotice>;
}

function useStockReferences(enabled: boolean) {
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
    }).catch((referenceError) => { if (active) setError(referenceError instanceof Error ? referenceError.message : "Movement references could not be loaded"); }).finally(() => { if (active) setLoading(false); });
    return () => { active = false; };
  }, [enabled]);
  return { items, stores, loading, error };
}

function useSelectedBalance(enabled: boolean, itemId: string | null, storeId: string | null) {
  const [value, setValue] = useState<StockBalance | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    if (!enabled || !itemId || !storeId) { setValue(null); setLoading(false); setError(null); return; }
    let active = true;
    setLoading(true);
    setError(null);
    void assetsInventoryService.listStockBalances({ page: 1, per_page: 1, item_id: itemId, store_id: storeId }).then((response) => {
      if (!active) return;
      if (!response.success || !response.data) throw new Error(responseMessage(response, "Current balance could not be loaded"));
      setValue(response.data.balances[0] ?? null);
    }).catch((loadError) => { if (active) { setValue(null); setError(loadError instanceof Error ? loadError.message : "Current balance could not be loaded"); } }).finally(() => { if (active) setLoading(false); });
    return () => { active = false; };
  }, [enabled, itemId, storeId]);
  return { value, loading, error };
}

function optional(value: string) { return value.trim() || null; }
function today() { const now = new Date(); const offset = now.getTimezoneOffset() * 60_000; return new Date(now.getTime() - offset).toISOString().slice(0, 10); }

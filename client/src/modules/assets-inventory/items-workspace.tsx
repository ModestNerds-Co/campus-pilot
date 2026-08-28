/** Inventory item master data with exact scaled reorder levels. */

import { useCallback, useEffect, useRef, useState } from "react";
import type { FormEvent } from "react";
import { Boxes, Edit3, Loader2, Plus, Search, Trash2 } from "lucide-react";
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
import { hasPermission } from "@/modules/users/access-control";
import { useAuthStore } from "@/stores/auth-store";

import { createIdempotencyKeyLifecycle } from "./create-idempotency-key";
import { assetsInventoryService, responseMessage } from "./service";
import type { CreateInventoryItemInput, InventoryItem, InventoryItemStatus, UpdateInventoryItemInput } from "./types";

export function InventoryItemsWorkspace() {
  const permissions = useAuthStore((state) => state.user?.permissions);
  const canCreate = hasPermission(permissions, "assets_inventory:create");
  const canEdit = hasPermission(permissions, "assets_inventory:edit");
  const canDelete = hasPermission(permissions, "assets_inventory:delete");
  const [items, setItems] = useState<InventoryItem[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [page, setPage] = useState(1);
  const [totalPages, setTotalPages] = useState(1);
  const [search, setSearch] = useState("");
  const [submittedSearch, setSubmittedSearch] = useState("");
  const [status, setStatus] = useState("all");
  const [drawer, setDrawer] = useState<InventoryItem | null | undefined>(undefined);
  const [deleteItem, setDeleteItem] = useState<InventoryItem | null>(null);
  const [deleting, setDeleting] = useState(false);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const response = await assetsInventoryService.listItems({ page, per_page: 20, search: submittedSearch || undefined, status: status === "all" ? undefined : status });
      if (!response.success || !response.data) throw new Error(responseMessage(response, "Items could not be loaded"));
      setItems(response.data.items);
      setTotalPages(response.pagination?.total_pages ?? 1);
    } catch (loadError) {
      setError(loadError instanceof Error ? loadError.message : "Items could not be loaded");
    } finally {
      setLoading(false);
    }
  }, [page, status, submittedSearch]);

  useEffect(() => { void load(); }, [load]);
  usePageChrome("Items", canCreate ? <Button onClick={() => setDrawer(null)}><Plus className="size-4" />New item</Button> : null);

  const remove = async () => {
    if (!deleteItem || deleting) return;
    setDeleting(true);
    try {
      const response = await assetsInventoryService.deleteItem(deleteItem.id, deleteItem.version);
      if (!response.success) throw new Error(responseMessage(response, "Item could not be removed"));
      toast.success("Item removed");
      setDeleteItem(null);
      await load();
    } catch (deleteError) {
      toast.error(deleteError instanceof Error ? deleteError.message : "Item could not be removed");
    } finally {
      setDeleting(false);
    }
  };

  const filtered = Boolean(submittedSearch) || status !== "all";
  return <div className="space-y-5">
    <p className="text-sm text-[var(--text-muted)]">Maintain the item definitions used by inventory operations.</p>
    <TableControlsBar>
      <TableControlsSearch onSubmit={(event) => { event.preventDefault(); setPage(1); setSubmittedSearch(search.trim()); }}>
        <Input aria-label="Search items" leadingIcon={<Search />} onChange={(event) => setSearch(event.target.value)} placeholder="Search item number, name, or barcode…" value={search} />
        <Button type="submit" variant="secondary">Search</Button>
      </TableControlsSearch>
      <Select aria-label="Item status" className="sm:w-44" onChange={(event) => { setPage(1); setStatus(event.target.value); }} value={status}>
        <option value="all">All statuses</option><option value="active">Active</option><option value="inactive">Inactive</option>
      </Select>
      {!loading && items.length > 0 ? <TableControlsPagination onNext={() => setPage((value) => Math.min(totalPages, value + 1))} onPrevious={() => setPage((value) => Math.max(1, value - 1))} page={page} totalPages={totalPages} /> : null}
    </TableControlsBar>

    <TableWrap>
      {loading ? <TableLoading columns={7} label="Loading items…" /> : error ? <TableError description={error} onRetry={() => void load()} /> : items.length === 0 ? <TableEmpty description={filtered ? "Change the current filters." : "Create the first inventory item definition."} icon={<Boxes />} title={filtered ? "No items match these filters" : "No items yet"} /> : <TableScroll><Table>
        <THead><tr><TH>Item</TH><TH>Barcode</TH><TH>Unit</TH><TH>Quantity precision</TH><TH>Reorder level</TH><TH>Status</TH><TH className="text-right">Actions</TH></tr></THead>
        <TBody>{items.map((item) => <TR key={item.id}>
          <TD><p className="font-medium text-[var(--text-strong)]">{item.name}</p><p className="mt-1 font-tabular text-xs text-[var(--text-subtle)]">{item.item_number}</p></TD>
          <TD className="font-tabular text-[var(--text-muted)]">{item.barcode || "—"}</TD>
          <TD className="text-[var(--text-body)]">{item.unit_label}</TD>
          <TD className="font-tabular text-[var(--text-muted)]">{scaleLabel(item.quantity_scale)}</TD>
          <TD className="whitespace-nowrap font-tabular text-[var(--text-body)]">{item.reorder_level_minor === null ? "Not set" : `${formatScaled(item.reorder_level_minor, item.quantity_scale)} ${item.unit_label}`}</TD>
          <TD><Badge tone={item.status === "active" ? "success" : "neutral"}>{item.status}</Badge></TD>
          <TD className="text-right"><div className="inline-flex gap-1">{canEdit ? <Button aria-label={`Edit ${item.name}`} onClick={() => setDrawer(item)} size="icon-sm" variant="ghost"><Edit3 className="size-4" /></Button> : null}{canDelete && item.status === "inactive" ? <Button aria-label={`Remove ${item.name}`} className="text-[var(--tone-danger)]" onClick={() => setDeleteItem(item)} size="icon-sm" variant="ghost"><Trash2 className="size-4" /></Button> : null}</div></TD>
        </TR>)}</TBody>
      </Table></TableScroll>}
    </TableWrap>

    <ItemDrawer item={drawer ?? null} onClose={() => setDrawer(undefined)} onSaved={() => { setDrawer(undefined); void load(); }} open={drawer !== undefined} />
    <ConfirmDrawer confirmLabel="Remove item" description={`Remove ${deleteItem?.item_number ?? "this item"}? Only an inactive, unused item can be removed.`} isPending={deleting} onClose={() => setDeleteItem(null)} onConfirm={() => void remove()} open={deleteItem !== null} title="Remove item?" />
  </div>;
}

function ItemDrawer({ item, onClose, onSaved, open }: { item: InventoryItem | null; onClose: () => void; onSaved: () => void; open: boolean }) {
  const createKey = useRef(createIdempotencyKeyLifecycle());
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [barcode, setBarcode] = useState("");
  const [unitLabel, setUnitLabel] = useState("");
  const [quantityScale, setQuantityScale] = useState(0);
  const [reorderLevel, setReorderLevel] = useState("");
  const [status, setStatus] = useState<InventoryItemStatus>("active");
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    if (!open) return;
    if (!item) createKey.current.startFresh();
    setName(item?.name ?? "");
    setDescription(item?.description ?? "");
    setBarcode(item?.barcode ?? "");
    setUnitLabel(item?.unit_label ?? "");
    setQuantityScale(item?.quantity_scale ?? 0);
    setReorderLevel(item?.reorder_level_minor === null || item?.reorder_level_minor === undefined ? "" : exactScaled(item.reorder_level_minor, item.quantity_scale));
    setStatus(item?.status ?? "active");
  }, [item, open]);

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    const parsedReorderLevel = reorderLevel.trim() ? parseScaled(reorderLevel, quantityScale) : null;
    if (reorderLevel.trim() && parsedReorderLevel === null) { toast.error(`Reorder level must have at most ${quantityScale} decimal places`); return; }
    const createPayload: CreateInventoryItemInput = { name: name.trim(), description: optional(description), barcode: optional(barcode), unit_label: unitLabel.trim(), quantity_scale: quantityScale, reorder_level_minor: parsedReorderLevel };
    const updatePayload: UpdateInventoryItemInput = { name: name.trim(), description: optional(description), barcode: optional(barcode), reorder_level_minor: parsedReorderLevel, status };
    setSaving(true);
    try {
      const response = item
        ? await assetsInventoryService.updateItem(item.id, { ...updatePayload, expected_version: item.version })
        : await assetsInventoryService.createItem({ ...createPayload, idempotency_key: createKey.current.current() });
      if (!response.success) throw new Error(responseMessage(response, "Item could not be saved"));
      if (!item) createKey.current.startFresh();
      toast.success("Item saved");
      onSaved();
    } catch (saveError) {
      toast.error(saveError instanceof Error ? saveError.message : "Item could not be saved");
    } finally {
      setSaving(false);
    }
  };

  return <DialogShell onClose={saving ? () => undefined : onClose} open={open}>
    <DialogHeader onClose={saving ? undefined : onClose} title={item ? `Edit ${item.item_number}` : "New item"} />
    <form onSubmit={submit}>
      <DialogBody className="space-y-5">
        {item ? <div><Label htmlFor="inventory-item-number">Item number</Label><Input className="mt-1.5 font-tabular" disabled id="inventory-item-number" value={item.item_number} /></div> : null}
        <div><Label htmlFor="inventory-item-name">Name</Label><Input className="mt-1.5" data-autofocus="true" id="inventory-item-name" maxLength={180} onChange={(event) => setName(event.target.value)} required value={name} /></div>
        <div><Label htmlFor="inventory-item-description">Description</Label><Textarea className="mt-1.5 min-h-24" id="inventory-item-description" maxLength={2000} onChange={(event) => setDescription(event.target.value)} value={description} /></div>
        <div className="grid gap-5 sm:grid-cols-2"><div><Label htmlFor="inventory-item-barcode">Barcode</Label><Input className="mt-1.5 font-tabular" id="inventory-item-barcode" maxLength={200} onChange={(event) => setBarcode(event.target.value)} value={barcode} /></div><div><Label htmlFor="inventory-item-unit">Unit label</Label><Input className="mt-1.5" disabled={Boolean(item)} id="inventory-item-unit" maxLength={40} onChange={(event) => setUnitLabel(event.target.value)} placeholder="e.g. units, kg, litres" required value={unitLabel} /></div></div>
        <div className="grid gap-5 sm:grid-cols-2"><div><Label htmlFor="inventory-item-scale">Quantity precision</Label><Select className="mt-1.5" disabled={Boolean(item)} id="inventory-item-scale" onChange={(event) => { setQuantityScale(Number(event.target.value)); setReorderLevel(""); }} value={quantityScale}>{Array.from({ length: 7 }, (_, scale) => <option key={scale} value={scale}>{scaleLabel(scale)}</option>)}</Select></div><div><Label htmlFor="inventory-item-reorder">Reorder level</Label><Input className="mt-1.5 font-tabular" id="inventory-item-reorder" inputMode="decimal" onChange={(event) => setReorderLevel(event.target.value)} placeholder="Not set" value={reorderLevel} /></div></div>
        {item ? <div><Label htmlFor="inventory-item-status">Status</Label><Select className="mt-1.5" id="inventory-item-status" onChange={(event) => setStatus(event.target.value as InventoryItemStatus)} value={status}><option value="active">Active</option><option value="inactive">Inactive</option></Select></div> : null}
      </DialogBody>
      <DialogFooter><Button disabled={saving} onClick={onClose} type="button" variant="secondary">Cancel</Button><Button disabled={saving} type="submit">{saving ? <Loader2 className="size-4 animate-spin" /> : null}{saving ? "Saving…" : "Save item"}</Button></DialogFooter>
    </form>
  </DialogShell>;
}

function optional(value: string) { return value.trim() || null; }
function scaleLabel(scale: number) { return scale === 0 ? "Whole units" : `${scale} decimal ${scale === 1 ? "place" : "places"}`; }
function exactScaled(valueMinor: number, scale: number) { const value = String(Math.abs(Math.trunc(valueMinor))).padStart(scale + 1, "0"); const sign = valueMinor < 0 ? "-" : ""; return scale === 0 ? `${sign}${value}` : `${sign}${value.slice(0, -scale)}.${value.slice(-scale)}`; }
function formatScaled(valueMinor: number, scale: number) { return exactScaled(valueMinor, scale).replace(/(\.\d*?)0+$/, "$1").replace(/\.$/, ""); }
function parseScaled(value: string, scale: number) { const normalized = value.trim(); if (!/^\d+(\.\d*)?$/.test(normalized)) return null; const [whole, fraction = ""] = normalized.split("."); if (fraction.length > scale) return null; const parsed = Number(`${whole}${fraction.padEnd(scale, "0")}`); return Number.isSafeInteger(parsed) ? parsed : null; }

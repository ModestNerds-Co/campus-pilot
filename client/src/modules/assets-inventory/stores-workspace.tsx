/** Inventory store master data with version-safe drawer workflows. */

import { useCallback, useEffect, useRef, useState } from "react";
import type { FormEvent } from "react";
import { Edit3, Loader2, Plus, Search, Trash2, Warehouse } from "lucide-react";
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
import type { CreateInventoryStoreInput, InventoryStore, InventoryStoreStatus, UpdateInventoryStoreInput } from "./types";

export function InventoryStoresWorkspace() {
  const permissions = useAuthStore((state) => state.user?.permissions);
  const canCreate = hasPermission(permissions, "assets_inventory:create");
  const canEdit = hasPermission(permissions, "assets_inventory:edit");
  const canDelete = hasPermission(permissions, "assets_inventory:delete");
  const [stores, setStores] = useState<InventoryStore[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [page, setPage] = useState(1);
  const [totalPages, setTotalPages] = useState(1);
  const [search, setSearch] = useState("");
  const [submittedSearch, setSubmittedSearch] = useState("");
  const [status, setStatus] = useState("all");
  const [drawer, setDrawer] = useState<InventoryStore | null | undefined>(undefined);
  const [deleteStore, setDeleteStore] = useState<InventoryStore | null>(null);
  const [deleting, setDeleting] = useState(false);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const response = await assetsInventoryService.listStores({ page, per_page: 20, search: submittedSearch || undefined, status: status === "all" ? undefined : status });
      if (!response.success || !response.data) throw new Error(responseMessage(response, "Stores could not be loaded"));
      setStores(response.data.stores);
      setTotalPages(response.pagination?.total_pages ?? 1);
    } catch (loadError) {
      setError(loadError instanceof Error ? loadError.message : "Stores could not be loaded");
    } finally {
      setLoading(false);
    }
  }, [page, status, submittedSearch]);

  useEffect(() => { void load(); }, [load]);
  usePageChrome("Stores", canCreate ? <Button onClick={() => setDrawer(null)}><Plus className="size-4" />New store</Button> : null);

  const remove = async () => {
    if (!deleteStore || deleting) return;
    setDeleting(true);
    try {
      const response = await assetsInventoryService.deleteStore(deleteStore.id, deleteStore.version);
      if (!response.success) throw new Error(responseMessage(response, "Store could not be removed"));
      toast.success("Store removed");
      setDeleteStore(null);
      await load();
    } catch (deleteError) {
      toast.error(deleteError instanceof Error ? deleteError.message : "Store could not be removed");
    } finally {
      setDeleting(false);
    }
  };

  const filtered = Boolean(submittedSearch) || status !== "all";
  return <div className="space-y-5">
    <p className="text-sm text-[var(--text-muted)]">Maintain the stores used to hold inventory.</p>
    <TableControlsBar>
      <TableControlsSearch onSubmit={(event) => { event.preventDefault(); setPage(1); setSubmittedSearch(search.trim()); }}>
        <Input aria-label="Search stores" leadingIcon={<Search />} onChange={(event) => setSearch(event.target.value)} placeholder="Search store number, name, or location…" value={search} />
        <Button type="submit" variant="secondary">Search</Button>
      </TableControlsSearch>
      <Select aria-label="Store status" className="sm:w-44" onChange={(event) => { setPage(1); setStatus(event.target.value); }} value={status}>
        <option value="all">All statuses</option><option value="active">Active</option><option value="inactive">Inactive</option>
      </Select>
      {!loading && stores.length > 0 ? <TableControlsPagination onNext={() => setPage((value) => Math.min(totalPages, value + 1))} onPrevious={() => setPage((value) => Math.max(1, value - 1))} page={page} totalPages={totalPages} /> : null}
    </TableControlsBar>

    <TableWrap>
      {loading ? <TableLoading columns={5} label="Loading stores…" /> : error ? <TableError description={error} onRetry={() => void load()} /> : stores.length === 0 ? <TableEmpty description={filtered ? "Change the current filters." : "Create the first inventory store."} icon={<Warehouse />} title={filtered ? "No stores match these filters" : "No stores yet"} /> : <TableScroll><Table>
        <THead><tr><TH>Store</TH><TH>Location</TH><TH>Notes</TH><TH>Status</TH><TH className="text-right">Actions</TH></tr></THead>
        <TBody>{stores.map((store) => <TR key={store.id}>
          <TD><p className="font-medium text-[var(--text-strong)]">{store.name}</p><p className="mt-1 font-tabular text-xs text-[var(--text-subtle)]">{store.store_number}</p></TD>
          <TD className="text-[var(--text-body)]">{store.location_label || "—"}</TD>
          <TD><p className="max-w-80 truncate text-[var(--text-muted)]">{store.notes || "—"}</p></TD>
          <TD><Badge tone={store.status === "active" ? "success" : "neutral"}>{store.status}</Badge></TD>
          <TD className="text-right"><div className="inline-flex gap-1">{canEdit ? <Button aria-label={`Edit ${store.name}`} onClick={() => setDrawer(store)} size="icon-sm" variant="ghost"><Edit3 className="size-4" /></Button> : null}{canDelete && store.status === "inactive" ? <Button aria-label={`Remove ${store.name}`} className="text-[var(--tone-danger)]" onClick={() => setDeleteStore(store)} size="icon-sm" variant="ghost"><Trash2 className="size-4" /></Button> : null}</div></TD>
        </TR>)}</TBody>
      </Table></TableScroll>}
    </TableWrap>

    <StoreDrawer onClose={() => setDrawer(undefined)} onSaved={() => { setDrawer(undefined); void load(); }} open={drawer !== undefined} store={drawer ?? null} />
    <ConfirmDrawer confirmLabel="Remove store" description={`Remove ${deleteStore?.store_number ?? "this store"}? Only an inactive, unused store can be removed.`} isPending={deleting} onClose={() => setDeleteStore(null)} onConfirm={() => void remove()} open={deleteStore !== null} title="Remove store?" />
  </div>;
}

function StoreDrawer({ onClose, onSaved, open, store }: { onClose: () => void; onSaved: () => void; open: boolean; store: InventoryStore | null }) {
  const createKey = useRef(createIdempotencyKeyLifecycle());
  const [name, setName] = useState("");
  const [locationLabel, setLocationLabel] = useState("");
  const [notes, setNotes] = useState("");
  const [status, setStatus] = useState<InventoryStoreStatus>("active");
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    if (!open) return;
    if (!store) createKey.current.startFresh();
    setName(store?.name ?? "");
    setLocationLabel(store?.location_label ?? "");
    setNotes(store?.notes ?? "");
    setStatus(store?.status ?? "active");
  }, [open, store]);

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    const createPayload: CreateInventoryStoreInput = { name: name.trim(), location_label: optional(locationLabel), notes: optional(notes) };
    const updatePayload: UpdateInventoryStoreInput = { ...createPayload, status };
    setSaving(true);
    try {
      const response = store
        ? await assetsInventoryService.updateStore(store.id, { ...updatePayload, expected_version: store.version })
        : await assetsInventoryService.createStore({ ...createPayload, idempotency_key: createKey.current.current() });
      if (!response.success) throw new Error(responseMessage(response, "Store could not be saved"));
      if (!store) createKey.current.startFresh();
      toast.success("Store saved");
      onSaved();
    } catch (saveError) {
      toast.error(saveError instanceof Error ? saveError.message : "Store could not be saved");
    } finally {
      setSaving(false);
    }
  };

  return <DialogShell onClose={saving ? () => undefined : onClose} open={open}>
    <DialogHeader onClose={saving ? undefined : onClose} title={store ? `Edit ${store.store_number}` : "New store"} />
    <form onSubmit={submit}>
      <DialogBody className="space-y-5">
        {store ? <div><Label htmlFor="inventory-store-number">Store number</Label><Input className="mt-1.5 font-tabular" disabled id="inventory-store-number" value={store.store_number} /></div> : null}
        <div><Label htmlFor="inventory-store-name">Name</Label><Input className="mt-1.5" data-autofocus="true" id="inventory-store-name" maxLength={180} onChange={(event) => setName(event.target.value)} required value={name} /></div>
        <div><Label htmlFor="inventory-store-location">Location</Label><Input className="mt-1.5" id="inventory-store-location" maxLength={200} onChange={(event) => setLocationLabel(event.target.value)} value={locationLabel} /></div>
        <div><Label htmlFor="inventory-store-notes">Notes</Label><Textarea className="mt-1.5 min-h-28" id="inventory-store-notes" maxLength={2000} onChange={(event) => setNotes(event.target.value)} value={notes} /></div>
        {store ? <div><Label htmlFor="inventory-store-status">Status</Label><Select className="mt-1.5" id="inventory-store-status" onChange={(event) => setStatus(event.target.value as InventoryStoreStatus)} value={status}><option value="active">Active</option><option value="inactive">Inactive</option></Select></div> : null}
      </DialogBody>
      <DialogFooter><Button disabled={saving} onClick={onClose} type="button" variant="secondary">Cancel</Button><Button disabled={saving} type="submit">{saving ? <Loader2 className="size-4 animate-spin" /> : null}{saving ? "Saving…" : "Save store"}</Button></DialogFooter>
    </form>
  </DialogShell>;
}

function optional(value: string) { return value.trim() || null; }

/** Authoritative on-hand stock balances with permission-aware movement entry. */

import { useCallback, useEffect, useMemo, useState } from "react";
import { Boxes, Plus, Search } from "lucide-react";

import { Button } from "@/components/ui/button";
import {
  Table, TableControlsBar, TableControlsPagination, TableControlsSearch, TableEmpty,
  TableError, TableLoading, TableScroll, TableWrap, TBody, TD, TH, THead, TR,
} from "@/components/ui/data-table";
import { Input } from "@/components/ui/input";
import { usePageChrome } from "@/modules/admin/layouts/page-chrome";
import { hasPermission } from "@/modules/users/access-control";
import { useAuthStore } from "@/stores/auth-store";

import { RecordMovementDrawer } from "./record-movement-drawer";
import type { ManualMovementKind, MovementDrawerSeed } from "./record-movement-drawer";
import { assetsInventoryService, responseMessage } from "./service";
import { formatStockQuantity } from "./stock-quantity";
import { formatOperationalDateTime } from "./stock-ui";
import type { StockBalance } from "./stock-types";

export function StockWorkspace() {
  const permissions = useAuthStore((state) => state.user?.permissions);
  const allowedKinds = useMemo(() => movementKindsFor(permissions), [permissions]);
  const [balances, setBalances] = useState<StockBalance[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [search, setSearch] = useState("");
  const [submittedSearch, setSubmittedSearch] = useState("");
  const [page, setPage] = useState(1);
  const [totalPages, setTotalPages] = useState(1);
  const [drawer, setDrawer] = useState<MovementDrawerSeed | null | undefined>(undefined);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const response = await assetsInventoryService.listStockBalances({ page, per_page: 20, search: submittedSearch || undefined });
      if (!response.success || !response.data) throw new Error(responseMessage(response, "Stock balances could not be loaded"));
      setBalances(response.data.balances);
      setTotalPages(response.pagination?.total_pages ?? 1);
    } catch (loadError) {
      setError(loadError instanceof Error ? loadError.message : "Stock balances could not be loaded");
    } finally {
      setLoading(false);
    }
  }, [page, submittedSearch]);

  useEffect(() => { void load(); }, [load]);
  usePageChrome("Stock", allowedKinds.length > 0 ? <Button onClick={() => setDrawer(null)}><Plus className="size-4" />Record movement</Button> : null);

  return <div className="space-y-5">
    <p className="text-sm text-[var(--text-muted)]">Review on-hand quantities by item and store.</p>
    <TableControlsBar>
      <TableControlsSearch onSubmit={(event) => { event.preventDefault(); setPage(1); setSubmittedSearch(search.trim()); }}>
        <Input aria-label="Search stock" leadingIcon={<Search />} onChange={(event) => setSearch(event.target.value)} placeholder="Search item or store…" value={search} />
        <Button type="submit" variant="secondary">Search</Button>
      </TableControlsSearch>
      {!loading && balances.length > 0 ? <TableControlsPagination onNext={() => setPage((value) => Math.min(totalPages, value + 1))} onPrevious={() => setPage((value) => Math.max(1, value - 1))} page={page} totalPages={totalPages} /> : null}
    </TableControlsBar>

    <TableWrap>
      {loading ? <TableLoading columns={6} label="Loading stock balances…" /> : error ? <TableError description={error} onRetry={() => void load()} title="Stock balances could not be loaded" /> : balances.length === 0 ? <TableEmpty description={submittedSearch ? "Change the current search." : "No stock has been posted yet."} icon={<Boxes />} title={submittedSearch ? "No stock matches this search" : "No stock balances"} /> : <TableScroll><Table>
        <THead><tr><TH>Item</TH><TH>Store</TH><TH className="text-right">On hand</TH><TH>Unit</TH><TH>Updated</TH><TH className="text-right">Actions</TH></tr></THead>
        <TBody>{balances.map((balance) => <TR key={`${balance.item_id}:${balance.store_id}`}>
          <TD><p className="font-medium text-[var(--text-strong)]">{balance.item_name}</p><p className="mt-1 font-tabular text-xs text-[var(--text-subtle)]">{balance.item_number}</p></TD>
          <TD><p className="text-[var(--text-body)]">{balance.store_name}</p><p className="mt-1 font-tabular text-xs text-[var(--text-subtle)]">{balance.store_number}</p></TD>
          <TD className="whitespace-nowrap text-right font-tabular font-semibold text-[var(--text-strong)]">{formatStockQuantity(balance.on_hand_minor, balance.quantity_scale)}</TD>
          <TD className="text-[var(--text-muted)]">{balance.unit_label}</TD>
          <TD className="whitespace-nowrap text-[var(--text-muted)]">{formatOperationalDateTime(balance.updated_at)}</TD>
          <TD className="text-right">{allowedKinds.length > 0 ? <Button onClick={() => setDrawer({ itemId: balance.item_id, storeId: balance.store_id })} size="sm" variant="ghost">Record</Button> : null}</TD>
        </TR>)}</TBody>
      </Table></TableScroll>}
    </TableWrap>

    <RecordMovementDrawer allowedKinds={allowedKinds} onClose={() => setDrawer(undefined)} onSaved={() => { setDrawer(undefined); void load(); }} open={drawer !== undefined} seed={drawer ?? null} />
  </div>;
}

export function movementKindsFor(permissions: string[] | undefined): ManualMovementKind[] {
  const candidates: Array<[ManualMovementKind, string]> = [
    ["manual_receipt", "assets_inventory:receive"],
    ["issue", "assets_inventory:issue"],
    ["transfer", "assets_inventory:transfer"],
    ["adjustment", "assets_inventory:adjust"],
  ];
  return candidates.filter(([, permission]) => hasPermission(permissions, permission)).map(([kind]) => kind);
}

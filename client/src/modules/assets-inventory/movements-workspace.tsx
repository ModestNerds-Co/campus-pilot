/** Immutable stock movement register and full-page evidence view. */

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { FormEvent } from "react";
import { Link, useNavigate } from "@tanstack/react-router";
import { ArrowLeft, Eye, Loader2, Plus, RotateCcw, Search, TriangleAlert } from "lucide-react";
import toast from "react-hot-toast";

import { Button, buttonVariants } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
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
import { RecordMovementDrawer } from "./record-movement-drawer";
import { assetsInventoryService, responseMessage } from "./service";
import { formatStockQuantity } from "./stock-quantity";
import { MovementKindBadge, StockFact, StockNotice, formatOperationalDate, formatOperationalDateTime, movementKindLabel } from "./stock-ui";
import type { StockMovement, StockMovementKind, StockMovementSummary } from "./stock-types";
import { movementKindsFor } from "./stock-workspace";

export function MovementsWorkspace() {
  const permissions = useAuthStore((state) => state.user?.permissions);
  const allowedKinds = useMemo(() => movementKindsFor(permissions), [permissions]);
  const [movements, setMovements] = useState<StockMovementSummary[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [search, setSearch] = useState("");
  const [submittedSearch, setSubmittedSearch] = useState("");
  const [kind, setKind] = useState("all");
  const [page, setPage] = useState(1);
  const [totalPages, setTotalPages] = useState(1);
  const [recordOpen, setRecordOpen] = useState(false);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const response = await assetsInventoryService.listStockMovements({ page, per_page: 20, search: submittedSearch || undefined, kind: kind === "all" ? undefined : kind as StockMovementKind });
      if (!response.success || !response.data) throw new Error(responseMessage(response, "Stock movements could not be loaded"));
      setMovements(response.data.movements);
      setTotalPages(response.pagination?.total_pages ?? 1);
    } catch (loadError) {
      setError(loadError instanceof Error ? loadError.message : "Stock movements could not be loaded");
    } finally {
      setLoading(false);
    }
  }, [kind, page, submittedSearch]);

  useEffect(() => { void load(); }, [load]);
  usePageChrome("Movements", allowedKinds.length > 0 ? <Button onClick={() => setRecordOpen(true)}><Plus className="size-4" />Record movement</Button> : null);
  const filtered = Boolean(submittedSearch) || kind !== "all";

  return <div className="space-y-5">
    <p className="text-sm text-[var(--text-muted)]">Review posted stock changes and reversals.</p>
    <TableControlsBar>
      <TableControlsSearch onSubmit={(event) => { event.preventDefault(); setPage(1); setSubmittedSearch(search.trim()); }}>
        <Input aria-label="Search movements" leadingIcon={<Search />} onChange={(event) => setSearch(event.target.value)} placeholder="Search movement or reference…" value={search} />
        <Button type="submit" variant="secondary">Search</Button>
      </TableControlsSearch>
      <Select aria-label="Movement type" className="sm:w-52" onChange={(event) => { setPage(1); setKind(event.target.value); }} value={kind}>
        <option value="all">All movement types</option>
        {(["manual_receipt", "issue", "transfer", "adjustment", "goods_receipt_allocation", "reversal"] as StockMovementKind[]).map((value) => <option key={value} value={value}>{movementKindLabel(value)}</option>)}
      </Select>
      {!loading && movements.length > 0 ? <TableControlsPagination onNext={() => setPage((value) => Math.min(totalPages, value + 1))} onPrevious={() => setPage((value) => Math.max(1, value - 1))} page={page} totalPages={totalPages} /> : null}
    </TableControlsBar>
    <TableWrap>
      {loading ? <TableLoading columns={7} label="Loading stock movements…" /> : error ? <TableError description={error} onRetry={() => void load()} title="Stock movements could not be loaded" /> : movements.length === 0 ? <TableEmpty description={filtered ? "Change the current filters." : "Post the first stock movement."} icon={<RotateCcw />} title={filtered ? "No movements match these filters" : "No movements yet"} /> : <TableScroll><Table>
        <THead><tr><TH>Movement</TH><TH>Effective date</TH><TH>Type</TH><TH>Reference</TH><TH>Lines</TH><TH>Posted</TH><TH className="text-right">Open</TH></tr></THead>
        <TBody>{movements.map((movement) => <TR key={movement.id}>
          <TD className="font-tabular font-medium text-[var(--text-strong)]">{movement.movement_number}</TD>
          <TD className="whitespace-nowrap">{formatOperationalDate(movement.effective_on)}</TD>
          <TD><MovementKindBadge kind={movement.kind} /></TD>
          <TD className="text-[var(--text-muted)]">{movement.reference || movement.source_goods_receipt_number || movement.reverses_movement_number || "—"}</TD>
          <TD className="font-tabular text-[var(--text-muted)]">{movement.line_count}</TD>
          <TD className="whitespace-nowrap text-[var(--text-muted)]">{formatOperationalDateTime(movement.posted_at)}</TD>
          <TD className="text-right"><Link aria-label={`Open ${movement.movement_number}`} className={buttonVariants({ variant: "ghost", size: "icon-sm" })} params={{ movementId: movement.id }} to="/modules/assets-inventory/movements/$movementId"><Eye className="size-4" /></Link></TD>
        </TR>)}</TBody>
      </Table></TableScroll>}
    </TableWrap>
    <RecordMovementDrawer allowedKinds={allowedKinds} onClose={() => setRecordOpen(false)} onSaved={() => { setRecordOpen(false); void load(); }} open={recordOpen} />
  </div>;
}

export function MovementDetail({ movementId }: { movementId: string }) {
  const navigate = useNavigate();
  const permissions = useAuthStore((state) => state.user?.permissions);
  const canReverse = hasPermission(permissions, "assets_inventory:reverse");
  const [movement, setMovement] = useState<StockMovement | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [reverseOpen, setReverseOpen] = useState(false);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const response = await assetsInventoryService.readStockMovement(movementId);
      if (!response.success || !response.data) throw new Error(responseMessage(response, "Stock movement could not be loaded"));
      setMovement(response.data);
    } catch (loadError) {
      setError(loadError instanceof Error ? loadError.message : "Stock movement could not be loaded");
    } finally {
      setLoading(false);
    }
  }, [movementId]);

  useEffect(() => { void load(); }, [load]);
  const mayReverse = Boolean(movement && canReverse && movement.kind !== "reversal" && !movement.reversed_by_movement_id);
  usePageChrome(movement?.movement_number ?? "Stock movement", mayReverse ? <Button onClick={() => setReverseOpen(true)} variant="secondary"><RotateCcw className="size-4" />Reverse</Button> : null);

  if (loading) return <div aria-label="Loading stock movement" className="space-y-4" role="status"><div className="h-36 animate-pulse rounded-[var(--radius-xl)] bg-[var(--surface-sunken)]" /><div className="h-72 animate-pulse rounded-[var(--radius-xl)] bg-[var(--surface-sunken)]" /></div>;
  if (error || !movement) return <TableWrap><TableError description={error ?? "Stock movement was not found"} onRetry={() => void load()} title="Stock movement could not be opened" /></TableWrap>;

  return <div className="space-y-6">
    <section className="rounded-[var(--radius-xl)] border border-[var(--border)] bg-[var(--surface)] p-5 shadow-[var(--shadow-card)] sm:p-6">
      <Link className={buttonVariants({ variant: "ghost", size: "sm" })} to="/modules/assets-inventory/movements"><ArrowLeft className="size-4" />Back to movements</Link>
      <div className="mt-4 flex flex-wrap items-center gap-3"><h1 className="font-tabular text-xl font-semibold text-[var(--text-strong)]">{movement.movement_number}</h1><MovementKindBadge kind={movement.kind} /><Badge dot tone="success">Posted</Badge></div>
      <p className="mt-2 text-sm text-[var(--text-muted)]">Effective {formatOperationalDate(movement.effective_on)}</p>
    </section>
    {movement.reversed_by_movement_number ? <StockNotice>This movement was reversed by <strong>{movement.reversed_by_movement_number}</strong>.</StockNotice> : null}
    <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
      <StockFact label="Reference" value={movement.reference || "Not set"} />
      <StockFact label="Posted" value={formatOperationalDateTime(movement.posted_at)} />
      <StockFact label="Posted by" value={<span className="break-all font-tabular">{movement.posted_by}</span>} />
      <StockFact label="Created" value={formatOperationalDateTime(movement.created_at)} />
      <StockFact label="Procurement receipt" value={movement.source_goods_receipt_number || "Not linked"} />
      <StockFact label="Reverses" value={movement.reverses_movement_number || "Not a reversal"} />
      <StockFact label="Movement ID" value={<span className="break-all font-tabular">{movement.id}</span>} />
    </div>
    {movement.reason ? <StockFact label="Reason" value={movement.reason} /> : null}
    <TableWrap><TableScroll><Table>
      <THead><tr><TH>#</TH><TH>Item</TH><TH>Store</TH><TH className="text-right">Change</TH><TH className="text-right">Before</TH><TH className="text-right">After</TH><TH>Source line</TH></tr></THead>
      <TBody>{movement.lines.map((line) => <TR key={line.id}>
        <TD className="font-tabular text-[var(--text-subtle)]">{line.line_number}</TD>
        <TD><p className="font-medium text-[var(--text-strong)]">{line.item_name}</p><p className="mt-1 font-tabular text-xs text-[var(--text-subtle)]">{line.item_number}</p></TD>
        <TD><p>{line.store_name}</p><p className="mt-1 font-tabular text-xs text-[var(--text-subtle)]">{line.store_number}</p></TD>
        <TD className={`whitespace-nowrap text-right font-tabular font-semibold ${line.quantity_delta_minor < 0 ? "text-[var(--tone-danger)]" : "text-[var(--tone-success)]"}`}>{line.quantity_delta_minor > 0 ? "+" : ""}{formatStockQuantity(line.quantity_delta_minor, line.quantity_scale)} {line.unit_label}</TD>
        <TD className="whitespace-nowrap text-right font-tabular">{formatStockQuantity(line.on_hand_before_minor, line.quantity_scale)}</TD>
        <TD className="whitespace-nowrap text-right font-tabular">{formatStockQuantity(line.on_hand_after_minor, line.quantity_scale)}</TD>
        <TD className="max-w-72 text-[var(--text-muted)]">{line.source_goods_receipt_line_number ? <><span className="font-tabular text-[var(--text-subtle)]">Line {line.source_goods_receipt_line_number}</span>{line.source_goods_receipt_description ? <span className="mt-1 block">{line.source_goods_receipt_description}</span> : null}</> : "—"}</TD>
      </TR>)}</TBody>
    </Table></TableScroll></TableWrap>
    <ReverseMovementDrawer movement={reverseOpen ? movement : null} onClose={() => setReverseOpen(false)} onDone={(created) => { setReverseOpen(false); void navigate({ to: "/modules/assets-inventory/movements/$movementId", params: { movementId: created.id }, replace: true }); }} />
  </div>;
}

function ReverseMovementDrawer({ movement, onClose, onDone }: { movement: StockMovement | null; onClose: () => void; onDone: (movement: StockMovement) => void }) {
  const createKey = useRef(createIdempotencyKeyLifecycle());
  const [effectiveOn, setEffectiveOn] = useState(today());
  const [reason, setReason] = useState("");
  const [dirty, setDirty] = useState(false);
  const [discarding, setDiscarding] = useState(false);
  const [pending, setPending] = useState(false);
  useEffect(() => { if (movement) { setEffectiveOn(today()); setReason(""); setDirty(false); setDiscarding(false); createKey.current.startFresh(); } }, [movement]);

  const requestClose = () => {
    if (pending) return;
    if (discarding) { setDiscarding(false); return; }
    if (dirty) { setDiscarding(true); return; }
    onClose();
  };
  const submit = async (event: FormEvent) => {
    event.preventDefault();
    if (!movement) return;
    if (!reason.trim()) { toast.error("Enter the reason for this reversal"); return; }
    setPending(true);
    try {
      const response = await assetsInventoryService.reverseStockMovement(movement.id, { effective_on: effectiveOn, reason: reason.trim(), idempotency_key: createKey.current.current() });
      if (!response.success || !response.data) throw new Error(responseMessage(response, "Stock movement could not be reversed"));
      createKey.current.startFresh();
      setDirty(false);
      toast.success("Stock movement reversed");
      onDone(response.data);
    } catch (reverseError) {
      toast.error(reverseError instanceof Error ? reverseError.message : "Stock movement could not be reversed");
    } finally {
      setPending(false);
    }
  };

  return <DialogShell onClose={requestClose} open={movement !== null}>
    {discarding ? <><DialogHeader onClose={requestClose} title="Discard reversal?" /><DialogBody><StockNotice danger><span className="flex gap-3"><TriangleAlert className="mt-0.5 size-5 shrink-0" />The reversal reason will be lost.</span></StockNotice></DialogBody><DialogFooter><Button autoFocus data-autofocus="true" onClick={() => setDiscarding(false)} type="button" variant="secondary">Keep editing</Button><Button onClick={() => { createKey.current.startFresh(); setDirty(false); onClose(); }} type="button" variant="destructive">Discard changes</Button></DialogFooter></> : <>
      <DialogHeader onClose={pending ? undefined : requestClose} title="Reverse stock movement" />
      <form onSubmit={submit}>
        <DialogBody className="space-y-5">
          <StockNotice danger>Reversing {movement?.movement_number ?? "this movement"} creates an exact compensating movement. The original remains in history.</StockNotice>
          <div><Label htmlFor="stock-reversal-date">Effective date</Label><Input className="mt-1.5 sm:max-w-xs" id="stock-reversal-date" onChange={(event) => { setEffectiveOn(event.target.value); setDirty(true); }} required type="date" value={effectiveOn} /></div>
          <div><Label htmlFor="stock-reversal-reason">Reason</Label><Textarea className="mt-1.5 min-h-28" data-autofocus="true" id="stock-reversal-reason" maxLength={2000} onChange={(event) => { setReason(event.target.value); setDirty(true); }} required value={reason} /></div>
        </DialogBody>
        <DialogFooter><Button disabled={pending} onClick={requestClose} type="button" variant="secondary">Keep movement</Button><Button disabled={pending} type="submit" variant="destructive">{pending ? <Loader2 className="size-4 animate-spin" /> : null}{pending ? "Reversing…" : "Reverse movement"}</Button></DialogFooter>
      </form>
    </>}
  </DialogShell>;
}

function today() { const now = new Date(); const offset = now.getTimezoneOffset() * 60_000; return new Date(now.getTime() - offset).toISOString().slice(0, 10); }

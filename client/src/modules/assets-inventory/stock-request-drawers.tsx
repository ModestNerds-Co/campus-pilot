/** Right-side stock-request workflows with stable idempotency and explicit conflict recovery. */

import { useEffect, useMemo, useRef, useState } from "react";
import type { FormEvent } from "react";
import { Loader2, Plus, Trash2, TriangleAlert } from "lucide-react";
import toast from "react-hot-toast";

import { SearchableSelect } from "@/components/searchable-select";
import { Button } from "@/components/ui/button";
import { DialogBody, DialogFooter, DialogHeader, DialogShell } from "@/components/ui/dialog";
import { Input, Label, Textarea } from "@/components/ui/input";

import { createIdempotencyKeyLifecycle } from "./create-idempotency-key";
import { loadAllStockReferences } from "./reference-pages";
import { assetsInventoryService, responseMessage } from "./service";
import { exactStockQuantity, formatStockQuantity, parseStockQuantity, quantityScaleLabel } from "./stock-quantity";
import { StockNotice } from "./stock-ui";
import type { InventoryItem } from "./types";
import type {
  StockRequest, StockRequestBalancePreview, StockRequestDepartment,
  StockRequesterCandidate,
} from "./stock-request-types";

interface DrawerCallbacks {
  onClose: () => void;
  onConflict: () => void;
  onSaved: (request?: StockRequest) => void;
}

interface EditorLine { key: string; itemId: string | null; quantity: string }

export function StockRequestEditorDrawer({ open, request, ...callbacks }: DrawerCallbacks & { open: boolean; request: StockRequest | null }) {
  const keyLifecycle = useRef(createIdempotencyKeyLifecycle());
  const [departments, setDepartments] = useState<StockRequestDepartment[]>([]);
  const [requesters, setRequesters] = useState<StockRequesterCandidate[]>([]);
  const [items, setItems] = useState<InventoryItem[]>([]);
  const [departmentId, setDepartmentId] = useState<string | null>(null);
  const [requesterId, setRequesterId] = useState<string | null>(null);
  const [purpose, setPurpose] = useState("");
  const [neededBy, setNeededBy] = useState("");
  const [lines, setLines] = useState<EditorLine[]>([blankLine()]);
  const [referencesLoading, setReferencesLoading] = useState(false);
  const [referencesError, setReferencesError] = useState<string | null>(null);
  const [requestersLoading, setRequestersLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [conflict, setConflict] = useState(false);
  const [dirty, setDirty] = useState(false);
  const [discarding, setDiscarding] = useState(false);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    if (!open) return;
    keyLifecycle.current.startFresh();
    setDepartmentId(request?.department_id ?? null);
    setRequesterId(request?.requester_employee_id ?? null);
    setPurpose(request?.purpose ?? "");
    setNeededBy(request?.needed_by ?? "");
    setLines(request?.lines.map((line) => ({ key: line.id, itemId: line.item_id, quantity: exactStockQuantity(line.requested_quantity_minor, line.quantity_scale) })) ?? [blankLine()]);
    setError(null);
    setConflict(false);
    setDirty(false);
    setDiscarding(false);
  }, [open, request]);

  useEffect(() => {
    if (!open) return;
    let active = true;
    setReferencesLoading(true);
    setReferencesError(null);
    void Promise.all([
      assetsInventoryService.listStockRequestDepartments(),
      loadAllStockReferences("Inventory items", async (page, perPage) => {
        const response = await assetsInventoryService.listItems({ page, per_page: perPage, status: "active" });
        if (!response.success || !response.data) throw new Error(responseMessage(response, "Inventory items could not be loaded"));
        return { records: response.data.items, total: response.pagination?.total ?? response.data.items.length, totalPages: response.pagination?.total_pages ?? 1 };
      }),
    ]).then(([departmentResponse, itemRecords]) => {
      if (!departmentResponse.success || !departmentResponse.data) throw new Error(responseMessage(departmentResponse, "Departments could not be loaded"));
      if (!active) return;
      setDepartments(departmentResponse.data.departments);
      setItems(itemRecords);
    }).catch((loadError) => {
      if (active) setReferencesError(loadError instanceof Error ? loadError.message : "Request options could not be loaded");
    }).finally(() => { if (active) setReferencesLoading(false); });
    return () => { active = false; };
  }, [open]);

  useEffect(() => {
    if (!open || !departmentId) { setRequesters([]); return; }
    let active = true;
    setRequestersLoading(true);
    void assetsInventoryService.listStockRequesters({ department_id: departmentId }).then((response) => {
      if (!response.success || !response.data) throw new Error(responseMessage(response, "Requesters could not be loaded"));
      if (active) setRequesters(response.data.employees);
    }).catch((loadError) => {
      if (active) setError(loadError instanceof Error ? loadError.message : "Requesters could not be loaded");
    }).finally(() => { if (active) setRequestersLoading(false); });
    return () => { active = false; };
  }, [departmentId, open]);

  const itemById = useMemo(() => new Map(items.map((item) => [item.id, item])), [items]);
  const requestClose = () => {
    if (saving) return;
    if (discarding) { setDiscarding(false); return; }
    if (dirty) { setDiscarding(true); return; }
    callbacks.onClose();
  };
  const changeDepartment = (value: string | null) => {
    setDepartmentId(value);
    if (requesterId && !requesters.some((employee) => employee.id === requesterId && employee.department_id === value)) setRequesterId(null);
    setDirty(true);
  };
  const updateLine = (key: string, patch: Partial<EditorLine>) => {
    setLines((current) => current.map((line) => line.key === key ? { ...line, ...patch } : line));
    setDirty(true);
  };

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    if (saving || conflict) return;
    setError(null);
    if (!departmentId || !requesterId || !purpose.trim()) { setError("Select a department and requester, then enter the purpose."); return; }
    if (lines.length === 0) { setError("Add at least one item."); return; }
    const seen = new Set<string>();
    const payloadLines = [];
    for (const line of lines) {
      const item = line.itemId ? itemById.get(line.itemId) : undefined;
      if (!item) { setError("Select an item on every line."); return; }
      if (seen.has(item.id)) { setError(`${item.name} is listed more than once.`); return; }
      seen.add(item.id);
      const quantity = parseStockQuantity(line.quantity, item.quantity_scale);
      if (quantity === null) { setError(`${item.name} needs a positive quantity with ${quantityScaleLabel(item.quantity_scale).toLowerCase()}.`); return; }
      payloadLines.push({ item_id: item.id, requested_quantity_minor: quantity });
    }
    setSaving(true);
    const common = {
      requester_employee_id: requesterId,
      department_id: departmentId,
      purpose: purpose.trim(),
      needed_by: neededBy || null,
      lines: payloadLines,
    };
    const idempotencyKey = keyLifecycle.current.currentForFingerprint(JSON.stringify(request ? { ...common, expected_version: request.version } : common));
    try {
      const response = request
        ? await assetsInventoryService.updateStockRequest(request.id, { ...common, expected_version: request.version, idempotency_key: idempotencyKey })
        : await assetsInventoryService.createStockRequest({ ...common, idempotency_key: idempotencyKey });
      if (!response.success || !response.data) {
        if (response.http_status === 409) { setConflict(true); return; }
        throw new Error(responseMessage(response, "The request could not be saved"));
      }
      keyLifecycle.current.startFresh();
      setDirty(false);
      toast.success(request ? "Request updated" : "Request created");
      callbacks.onSaved(response.data);
    } catch (saveError) {
      setError(saveError instanceof Error ? saveError.message : "The request could not be saved");
    } finally { setSaving(false); }
  };

  return <DialogShell onClose={requestClose} open={open} panelClassName="sm:max-w-[720px]">
    {discarding ? <DiscardView onDiscard={() => { keyLifecycle.current.startFresh(); setDirty(false); callbacks.onClose(); }} onKeep={() => setDiscarding(false)} /> : <>
      <DialogHeader onClose={saving ? undefined : requestClose} title={request ? `Edit ${request.request_number}` : "New stock request"} />
      <form onSubmit={submit}>
        <DialogBody className="space-y-6">
          {conflict ? <ConflictNotice onReload={callbacks.onConflict} /> : error ? <StockNotice danger>{error}</StockNotice> : null}
          {referencesError ? <StockNotice danger>{referencesError}</StockNotice> : null}
          <div className="grid gap-5 sm:grid-cols-2">
            <div><Label htmlFor="stock-request-department">Department</Label><SearchableSelect allowClear={false} className="mt-1.5" disabled={saving || referencesLoading} id="stock-request-department" loading={referencesLoading} onChange={changeDepartment} options={departments.map((department) => ({ id: department.id, value: department.name, label: department.name, description: department.code }))} placeholder="Select department" value={departmentId} /></div>
            <div><Label htmlFor="stock-request-requester">Requester</Label><SearchableSelect allowClear={false} className="mt-1.5" disabled={saving || !departmentId} id="stock-request-requester" loading={requestersLoading} onChange={(value) => { setRequesterId(value); setDirty(true); }} options={requesters.map((employee) => ({ id: employee.id, value: employee.display_name, label: employee.display_name, description: employee.employee_number }))} placeholder={departmentId ? "Select requester" : "Select a department first"} value={requesterId} /></div>
          </div>
          <div><Label htmlFor="stock-request-purpose">Purpose</Label><Textarea className="mt-1.5 min-h-28" data-autofocus="true" disabled={saving} id="stock-request-purpose" maxLength={2000} onChange={(event) => { setPurpose(event.target.value); setDirty(true); }} required value={purpose} /></div>
          <div><Label htmlFor="stock-request-needed-by">Needed by</Label><Input className="mt-1.5 sm:max-w-xs" disabled={saving} id="stock-request-needed-by" onChange={(event) => { setNeededBy(event.target.value); setDirty(true); }} type="date" value={neededBy} /></div>
          <section aria-labelledby="stock-request-lines-heading" className="space-y-4">
            <div className="flex items-center justify-between gap-4"><div><h3 className="text-sm font-semibold text-[var(--text-strong)]" id="stock-request-lines-heading">Items</h3><p className="mt-1 text-xs text-[var(--text-muted)]">Enter quantities using each item’s configured precision.</p></div><Button disabled={saving || referencesLoading} onClick={() => { setLines((current) => [...current, blankLine()]); setDirty(true); }} size="sm" type="button" variant="secondary"><Plus className="size-4" />Add item</Button></div>
            {lines.map((line, index) => {
              const item = line.itemId ? itemById.get(line.itemId) : undefined;
              return <div className="grid gap-3 rounded-[var(--radius-lg)] border border-[var(--border)] bg-[var(--surface-muted)] p-4 sm:grid-cols-[minmax(0,1fr)_160px_auto] sm:items-end" key={line.key}>
                <div><Label htmlFor={`stock-request-item-${line.key}`}>Item {index + 1}</Label><SearchableSelect allowClear={false} className="mt-1.5" disabled={saving} id={`stock-request-item-${line.key}`} onChange={(value) => updateLine(line.key, { itemId: value, quantity: "" })} options={items.map((candidate) => ({ id: candidate.id, value: candidate.name, label: candidate.name, description: `${candidate.item_number} · ${candidate.unit_label}` }))} placeholder="Select item" value={line.itemId} /></div>
                <div><Label htmlFor={`stock-request-quantity-${line.key}`}>Quantity{item ? ` (${item.unit_label})` : ""}</Label><Input className="mt-1.5 font-tabular" disabled={saving || !item} id={`stock-request-quantity-${line.key}`} inputMode="decimal" onChange={(event) => updateLine(line.key, { quantity: event.target.value })} placeholder={item ? exactStockQuantity(1, item.quantity_scale) : "Select item"} value={line.quantity} /></div>
                <Button aria-label={`Remove item ${index + 1}`} disabled={saving || lines.length === 1} onClick={() => { setLines((current) => current.filter((candidate) => candidate.key !== line.key)); setDirty(true); }} size="icon" type="button" variant="ghost"><Trash2 className="size-4" /></Button>
              </div>;
            })}
          </section>
        </DialogBody>
        <DialogFooter><Button disabled={saving} onClick={requestClose} type="button" variant="secondary">Cancel</Button><Button disabled={saving || referencesLoading || conflict || Boolean(referencesError)} type="submit">{saving ? <Loader2 className="size-4 animate-spin" /> : null}{saving ? "Saving…" : request ? "Save changes" : "Create request"}</Button></DialogFooter>
      </form>
    </>}
  </DialogShell>;
}

export function StockRequestApprovalDrawer({ open, request, ...callbacks }: DrawerCallbacks & { open: boolean; request: StockRequest }) {
  const keyLifecycle = useRef(createIdempotencyKeyLifecycle());
  const [quantities, setQuantities] = useState<Record<string, string>>({});
  const [note, setNote] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [conflict, setConflict] = useState(false);
  const [saving, setSaving] = useState(false);
  useEffect(() => {
    if (!open) return;
    keyLifecycle.current.startFresh();
    setQuantities(Object.fromEntries(request.lines.map((line) => [line.id, exactStockQuantity(line.requested_quantity_minor, line.quantity_scale)])));
    setNote(""); setError(null); setConflict(false);
  }, [open, request]);

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    if (saving || conflict) return;
    const lines = [];
    let hasPositive = false;
    for (const line of request.lines) {
      const quantity = parseStockQuantity(quantities[line.id] ?? "", line.quantity_scale, true);
      if (quantity === null || quantity > line.requested_quantity_minor) { setError(`${line.item_name} must be between 0 and ${formatStockQuantity(line.requested_quantity_minor, line.quantity_scale)} ${line.unit_label}.`); return; }
      if (quantity > 0) hasPositive = true;
      lines.push({ request_line_id: line.id, approved_quantity_minor: quantity });
    }
    if (!hasPositive) { setError("Approve a positive quantity for at least one item."); return; }
    setSaving(true); setError(null);
    try {
      const payload = { expected_version: request.version, note: optional(note), lines };
      const response = await assetsInventoryService.approveStockRequest(request.id, { ...payload, idempotency_key: keyLifecycle.current.currentForFingerprint(JSON.stringify(payload)) });
      if (!response.success || !response.data) {
        if (response.http_status === 409) { setConflict(true); return; }
        throw new Error(responseMessage(response, "The request could not be approved"));
      }
      keyLifecycle.current.startFresh(); toast.success("Request approved"); callbacks.onSaved(response.data);
    } catch (approvalError) { setError(approvalError instanceof Error ? approvalError.message : "The request could not be approved"); }
    finally { setSaving(false); }
  };

  return <DialogShell onClose={saving ? () => undefined : callbacks.onClose} open={open} panelClassName="sm:max-w-[700px]">
    <DialogHeader onClose={saving ? undefined : callbacks.onClose} title={`Approve ${request.request_number}`} />
    <form onSubmit={submit}><DialogBody className="space-y-5">
      {conflict ? <ConflictNotice onReload={callbacks.onConflict} /> : error ? <StockNotice danger>{error}</StockNotice> : null}
      <StockNotice>Set the approved quantity for every line. Enter 0 for a line that is not approved.</StockNotice>
      {request.lines.map((line, index) => <div className="grid gap-3 rounded-[var(--radius-lg)] border border-[var(--border)] p-4 sm:grid-cols-[minmax(0,1fr)_190px] sm:items-end" key={line.id}>
        <div><p className="font-medium text-[var(--text-strong)]">{line.item_name}</p><p className="mt-1 text-xs text-[var(--text-muted)]">{line.item_number} · requested {formatStockQuantity(line.requested_quantity_minor, line.quantity_scale)} {line.unit_label}</p></div>
        <div><Label htmlFor={`approve-request-line-${line.id}`}>Approved quantity</Label><Input className="mt-1.5 font-tabular" data-autofocus={index === 0 ? "true" : undefined} disabled={saving} id={`approve-request-line-${line.id}`} inputMode="decimal" onChange={(event) => setQuantities((current) => ({ ...current, [line.id]: event.target.value }))} value={quantities[line.id] ?? ""} /></div>
      </div>)}
      <div><Label htmlFor="stock-request-approval-note">Decision note</Label><Textarea className="mt-1.5 min-h-24" disabled={saving} id="stock-request-approval-note" maxLength={1000} onChange={(event) => setNote(event.target.value)} value={note} /></div>
    </DialogBody><DialogFooter><Button disabled={saving} onClick={callbacks.onClose} type="button" variant="secondary">Cancel</Button><Button disabled={saving || conflict} type="submit">{saving ? <Loader2 className="size-4 animate-spin" /> : null}{saving ? "Approving…" : "Approve request"}</Button></DialogFooter></form>
  </DialogShell>;
}

export function StockRequestReasonDrawer({ action, open, request, ...callbacks }: DrawerCallbacks & { action: "reject" | "cancel"; open: boolean; request: StockRequest }) {
  const keyLifecycle = useRef(createIdempotencyKeyLifecycle());
  const [reason, setReason] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [conflict, setConflict] = useState(false);
  const [saving, setSaving] = useState(false);
  useEffect(() => { if (open) { keyLifecycle.current.startFresh(); setReason(""); setError(null); setConflict(false); } }, [open, action, request.id]);
  const submit = async (event: FormEvent) => {
    event.preventDefault();
    if (saving || conflict) return;
    if (!reason.trim()) { setError(`Enter the reason for ${action === "reject" ? "rejecting" : "cancelling"} this request.`); return; }
    setSaving(true); setError(null);
    try {
      const commandPayload = { expected_version: request.version, reason: reason.trim() };
      const payload = { ...commandPayload, idempotency_key: keyLifecycle.current.currentForFingerprint(JSON.stringify(commandPayload)) };
      const response = action === "reject" ? await assetsInventoryService.rejectStockRequest(request.id, payload) : await assetsInventoryService.cancelStockRequest(request.id, payload);
      if (!response.success || !response.data) {
        if (response.http_status === 409) { setConflict(true); return; }
        throw new Error(responseMessage(response, `The request could not be ${action === "reject" ? "rejected" : "cancelled"}`));
      }
      keyLifecycle.current.startFresh(); toast.success(action === "reject" ? "Request rejected" : "Request cancelled"); callbacks.onSaved(response.data);
    } catch (saveError) { setError(saveError instanceof Error ? saveError.message : "The decision could not be saved"); }
    finally { setSaving(false); }
  };
  const verb = action === "reject" ? "Reject" : "Cancel";
  return <DialogShell onClose={saving ? () => undefined : callbacks.onClose} open={open}>
    <DialogHeader onClose={saving ? undefined : callbacks.onClose} title={`${verb} ${request.request_number}`} />
    <form onSubmit={submit}><DialogBody className="space-y-5">{conflict ? <ConflictNotice onReload={callbacks.onConflict} /> : error ? <StockNotice danger>{error}</StockNotice> : null}<StockNotice danger>This decision is recorded in the request history.</StockNotice><div><Label htmlFor={`stock-request-${action}-reason`}>Reason</Label><Textarea className="mt-1.5 min-h-32" data-autofocus="true" disabled={saving} id={`stock-request-${action}-reason`} maxLength={1000} onChange={(event) => setReason(event.target.value)} required value={reason} /></div></DialogBody><DialogFooter><Button data-autofocus="false" disabled={saving} onClick={callbacks.onClose} type="button" variant="secondary">Keep request</Button><Button disabled={saving || conflict} type="submit" variant="destructive">{saving ? <Loader2 className="size-4 animate-spin" /> : null}{saving ? (action === "reject" ? "Rejecting…" : "Cancelling…") : `${verb} request`}</Button></DialogFooter></form>
  </DialogShell>;
}

export function StockRequestCloseDrawer({ open, request, ...callbacks }: DrawerCallbacks & { open: boolean; request: StockRequest }) {
  const keyLifecycle = useRef(createIdempotencyKeyLifecycle());
  const [note, setNote] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [conflict, setConflict] = useState(false);
  const [saving, setSaving] = useState(false);
  useEffect(() => { if (open) { keyLifecycle.current.startFresh(); setNote(""); setError(null); setConflict(false); } }, [open, request.id]);
  const submit = async (event: FormEvent) => {
    event.preventDefault(); if (saving || conflict) return; setSaving(true); setError(null);
    try {
      const payload = { expected_version: request.version, note: optional(note) };
      const response = await assetsInventoryService.closeStockRequest(request.id, { ...payload, idempotency_key: keyLifecycle.current.currentForFingerprint(JSON.stringify(payload)) });
      if (!response.success || !response.data) { if (response.http_status === 409) { setConflict(true); return; } throw new Error(responseMessage(response, "The request could not be closed")); }
      keyLifecycle.current.startFresh(); toast.success("Request closed"); callbacks.onSaved(response.data);
    } catch (closeError) { setError(closeError instanceof Error ? closeError.message : "The request could not be closed"); } finally { setSaving(false); }
  };
  return <DialogShell onClose={saving ? () => undefined : callbacks.onClose} open={open}><DialogHeader onClose={saving ? undefined : callbacks.onClose} title={`Close ${request.request_number}`} /><form onSubmit={submit}><DialogBody className="space-y-5">{conflict ? <ConflictNotice onReload={callbacks.onConflict} /> : error ? <StockNotice danger>{error}</StockNotice> : null}<StockNotice>Closing ends this partially fulfilled request. Its issued stock and history remain unchanged.</StockNotice><div><Label htmlFor="stock-request-close-note">Closure note</Label><Textarea className="mt-1.5 min-h-28" data-autofocus="true" disabled={saving} id="stock-request-close-note" maxLength={1000} onChange={(event) => setNote(event.target.value)} value={note} /></div></DialogBody><DialogFooter><Button disabled={saving} onClick={callbacks.onClose} type="button" variant="secondary">Keep open</Button><Button disabled={saving || conflict} type="submit">{saving ? <Loader2 className="size-4 animate-spin" /> : null}{saving ? "Closing…" : "Close request"}</Button></DialogFooter></form></DialogShell>;
}

interface FulfilLineState { storeId: string | null; quantity: string }

export function StockRequestFulfilDrawer({ open, request, ...callbacks }: DrawerCallbacks & { open: boolean; request: StockRequest }) {
  const keyLifecycle = useRef(createIdempotencyKeyLifecycle());
  const [previewRequest, setPreviewRequest] = useState<StockRequest | null>(null);
  const [balances, setBalances] = useState<StockRequestBalancePreview[]>([]);
  const [lines, setLines] = useState<Record<string, FulfilLineState>>({});
  const [effectiveOn, setEffectiveOn] = useState(today());
  const [reason, setReason] = useState("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [conflict, setConflict] = useState(false);
  const [saving, setSaving] = useState(false);

  const load = () => {
    setLoading(true); setError(null); setConflict(false);
    void assetsInventoryService.readStockRequestFulfilmentPreview(request.id).then((response) => {
      if (!response.success || !response.data) throw new Error(responseMessage(response, "Fulfilment details could not be loaded"));
      setPreviewRequest(response.data.request); setBalances(response.data.balances);
      setLines(Object.fromEntries(response.data.request.lines.filter((line) => line.remaining_quantity_minor > 0).map((line) => [line.id, { storeId: null, quantity: "" }])));
    }).catch((loadError) => setError(loadError instanceof Error ? loadError.message : "Fulfilment details could not be loaded"))
      .finally(() => setLoading(false));
  };
  useEffect(() => { if (open) { keyLifecycle.current.startFresh(); setPreviewRequest(null); setBalances([]); setEffectiveOn(today()); setReason(""); setError(null); setConflict(false); load(); } }, [open, request.id]); // eslint-disable-line react-hooks/exhaustive-deps

  const submit = async (event: FormEvent) => {
    event.preventDefault(); if (saving || conflict || !previewRequest) return;
    const payloadLines = [];
    for (const requestLine of previewRequest.lines.filter((line) => line.remaining_quantity_minor > 0)) {
      const state = lines[requestLine.id];
      if (!state?.storeId && !state?.quantity.trim()) continue;
      if (!state?.storeId) { setError(`Select a store for ${requestLine.item_name}.`); return; }
      const balance = balances.find((candidate) => candidate.item_id === requestLine.item_id && candidate.store_id === state.storeId);
      if (!balance) { setError(`The selected balance for ${requestLine.item_name} is no longer available.`); return; }
      const quantity = parseStockQuantity(state.quantity, requestLine.quantity_scale);
      if (quantity === null || quantity > requestLine.remaining_quantity_minor) { setError(`${requestLine.item_name} must be a positive quantity up to ${formatStockQuantity(requestLine.remaining_quantity_minor, requestLine.quantity_scale)} ${requestLine.unit_label}.`); return; }
      if (quantity > balance.on_hand_minor) { setError(`${balance.store_name} only has ${formatStockQuantity(balance.on_hand_minor, balance.quantity_scale)} ${balance.unit_label} of ${requestLine.item_name}.`); return; }
      payloadLines.push({ request_line_id: requestLine.id, store_id: balance.store_id, quantity_minor: quantity, expected_balance_version: balance.version });
    }
    if (payloadLines.length === 0) { setError("Enter at least one issue quantity and store."); return; }
    setSaving(true); setError(null);
    try {
      const payload = { expected_request_version: previewRequest.version, effective_on: effectiveOn, reason: optional(reason), lines: payloadLines };
      const response = await assetsInventoryService.fulfilStockRequest(request.id, { ...payload, idempotency_key: keyLifecycle.current.currentForFingerprint(JSON.stringify(payload)) });
      if (!response.success || !response.data) { if (response.http_status === 409) { setConflict(true); return; } throw new Error(responseMessage(response, "Stock could not be issued")); }
      keyLifecycle.current.startFresh(); toast.success(`Issue ${response.data.movement_number} posted`); callbacks.onSaved(response.data.request);
    } catch (fulfilError) { setError(fulfilError instanceof Error ? fulfilError.message : "Stock could not be issued"); } finally { setSaving(false); }
  };

  return <DialogShell onClose={saving ? () => undefined : callbacks.onClose} open={open} panelClassName="sm:max-w-[760px]">
    <DialogHeader onClose={saving ? undefined : callbacks.onClose} title={`Fulfil ${request.request_number}`} />
    <form onSubmit={submit}><DialogBody className="space-y-5">
      {conflict ? <ConflictNotice onReload={callbacks.onConflict} /> : error ? <StockNotice danger>{error}</StockNotice> : null}
      {loading ? <div aria-label="Loading fulfilment details" className="flex min-h-52 items-center justify-center gap-3 text-sm text-[var(--text-muted)]" role="status"><Loader2 className="size-5 animate-spin" />Loading stock balances…</div> : previewRequest ? <>
        <div className="grid gap-5 sm:grid-cols-2"><div><Label htmlFor="stock-request-fulfil-date">Effective date</Label><Input className="mt-1.5" data-autofocus="true" disabled={saving} id="stock-request-fulfil-date" onChange={(event) => setEffectiveOn(event.target.value)} required type="date" value={effectiveOn} /></div><div><Label htmlFor="stock-request-fulfil-reason">Issue note</Label><Input className="mt-1.5" disabled={saving} id="stock-request-fulfil-reason" maxLength={2000} onChange={(event) => setReason(event.target.value)} value={reason} /></div></div>
        <section aria-labelledby="fulfil-lines-heading" className="space-y-4"><div><h3 className="text-sm font-semibold text-[var(--text-strong)]" id="fulfil-lines-heading">Issue quantities</h3><p className="mt-1 text-xs text-[var(--text-muted)]">Leave a line blank if it is not being issued now.</p></div>
          {previewRequest.lines.filter((line) => line.remaining_quantity_minor > 0).map((line) => {
            const itemBalances = balances.filter((balance) => balance.item_id === line.item_id);
            const state = lines[line.id] ?? { storeId: null, quantity: "" };
            const selected = itemBalances.find((balance) => balance.store_id === state.storeId);
            return <div className="space-y-4 rounded-[var(--radius-lg)] border border-[var(--border)] p-4" key={line.id}>
              <div><p className="font-medium text-[var(--text-strong)]">{line.item_name}</p><p className="mt-1 text-xs text-[var(--text-muted)]">{line.item_number} · remaining {formatStockQuantity(line.remaining_quantity_minor, line.quantity_scale)} {line.unit_label}</p></div>
              {itemBalances.length === 0 ? <StockNotice danger>No store balance is available for this item.</StockNotice> : <div className="grid gap-4 sm:grid-cols-2"><div><Label htmlFor={`fulfil-store-${line.id}`}>Store</Label><SearchableSelect className="mt-1.5" disabled={saving} id={`fulfil-store-${line.id}`} onChange={(value) => setLines((current) => ({ ...current, [line.id]: { ...state, storeId: value } }))} options={itemBalances.map((balance) => ({ id: balance.store_id, value: balance.store_name, label: balance.store_name, description: `${balance.store_number} · ${formatStockQuantity(balance.on_hand_minor, balance.quantity_scale)} ${balance.unit_label} on hand` }))} placeholder="Select store" value={state.storeId} /></div><div><Label htmlFor={`fulfil-quantity-${line.id}`}>Quantity ({line.unit_label})</Label><Input className="mt-1.5 font-tabular" disabled={saving || !selected} id={`fulfil-quantity-${line.id}`} inputMode="decimal" onChange={(event) => setLines((current) => ({ ...current, [line.id]: { ...state, quantity: event.target.value } }))} placeholder={selected ? `Up to ${formatStockQuantity(Math.min(selected.on_hand_minor, line.remaining_quantity_minor), line.quantity_scale)}` : "Select store"} value={state.quantity} /></div></div>}
            </div>;
          })}
        </section>
      </> : !error ? <StockNotice>No fulfilment details are available.</StockNotice> : null}
    </DialogBody><DialogFooter>{error && !previewRequest && !conflict ? <Button disabled={loading} onClick={load} type="button" variant="secondary">Retry</Button> : null}<Button disabled={saving} onClick={callbacks.onClose} type="button" variant="secondary">Cancel</Button><Button disabled={saving || loading || conflict || !previewRequest} type="submit">{saving ? <Loader2 className="size-4 animate-spin" /> : null}{saving ? "Posting issue…" : "Post issue"}</Button></DialogFooter></form>
  </DialogShell>;
}

export function StockRequestCommandDrawer({ command, open, request, ...callbacks }: DrawerCallbacks & { command: "submit" | "delete"; open: boolean; request: StockRequest }) {
  const keyLifecycle = useRef(createIdempotencyKeyLifecycle());
  const [error, setError] = useState<string | null>(null);
  const [conflict, setConflict] = useState(false);
  const [saving, setSaving] = useState(false);
  useEffect(() => { if (open) { keyLifecycle.current.startFresh(); setError(null); setConflict(false); } }, [open, command, request.id]);
  const run = async () => {
    if (saving || conflict) return; setSaving(true); setError(null);
    try {
      const commandPayload = { expected_version: request.version };
      const payload = { ...commandPayload, idempotency_key: keyLifecycle.current.currentForFingerprint(JSON.stringify(commandPayload)) };
      const response = command === "submit" ? await assetsInventoryService.submitStockRequest(request.id, payload) : await assetsInventoryService.deleteStockRequest(request.id, payload);
      if (!response.success || !response.data) { if (response.http_status === 409) { setConflict(true); return; } throw new Error(responseMessage(response, command === "submit" ? "The request could not be submitted" : "The request could not be removed")); }
      keyLifecycle.current.startFresh(); toast.success(command === "submit" ? "Request submitted" : "Draft removed"); callbacks.onSaved(command === "submit" ? response.data as StockRequest : undefined);
    } catch (commandError) { setError(commandError instanceof Error ? commandError.message : "The action could not be completed"); } finally { setSaving(false); }
  };
  const removing = command === "delete";
  return <DialogShell onClose={saving ? () => undefined : callbacks.onClose} open={open}><DialogHeader onClose={saving ? undefined : callbacks.onClose} title={removing ? `Remove ${request.request_number}` : `Submit ${request.request_number}`} /><DialogBody className="space-y-5">{conflict ? <ConflictNotice onReload={callbacks.onConflict} /> : error ? <StockNotice danger>{error}</StockNotice> : null}<div className="flex gap-4"><span className={`flex size-10 shrink-0 items-center justify-center rounded-[9px] ${removing ? "bg-[var(--tone-danger-bg)] text-[var(--tone-danger)]" : "bg-[var(--tone-info-bg)] text-[var(--tone-info)]"}`}><TriangleAlert className="size-5" /></span><p className="text-sm leading-6 text-[var(--text-muted)]">{removing ? "This removes the draft request. Submitted requests remain in the operational record." : "Submitting locks the request lines and sends the request for an approval decision."}</p></div></DialogBody><DialogFooter><Button data-autofocus="true" disabled={saving} onClick={callbacks.onClose} type="button" variant="secondary">{removing ? "Keep draft" : "Keep editing"}</Button><Button disabled={saving || conflict} onClick={() => void run()} type="button" variant={removing ? "destructive" : "default"}>{saving ? <Loader2 className="size-4 animate-spin" /> : null}{saving ? (removing ? "Removing…" : "Submitting…") : (removing ? "Remove draft" : "Submit request")}</Button></DialogFooter></DialogShell>;
}

function ConflictNotice({ onReload }: { onReload: () => void }) {
  return <StockNotice danger><div className="space-y-3"><p>This request changed after the drawer was opened. Your entries have been kept. Reload the request before applying this action.</p><Button onClick={onReload} size="sm" type="button" variant="secondary">Reload request</Button></div></StockNotice>;
}

function DiscardView({ onDiscard, onKeep }: { onDiscard: () => void; onKeep: () => void }) {
  return <><DialogHeader onClose={onKeep} title="Discard changes?" /><DialogBody><StockNotice danger><span className="flex gap-3"><TriangleAlert className="mt-0.5 size-5 shrink-0" />The unsaved request changes will be lost.</span></StockNotice></DialogBody><DialogFooter><Button data-autofocus="true" onClick={onKeep} type="button" variant="secondary">Keep editing</Button><Button onClick={onDiscard} type="button" variant="destructive">Discard changes</Button></DialogFooter></>;
}

function blankLine(): EditorLine { return { key: crypto.randomUUID(), itemId: null, quantity: "" }; }
function optional(value: string): string | null { return value.trim() || null; }
function today(): string { const now = new Date(); const offset = now.getTimezoneOffset() * 60_000; return new Date(now.getTime() - offset).toISOString().slice(0, 10); }

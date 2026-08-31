/** Department stock-request register and direct-link-safe operational detail. */

import { useCallback, useEffect, useMemo, useState } from "react";
import { Link, useNavigate } from "@tanstack/react-router";
import {
  ArrowLeft, Ban, Check, ClipboardCheck, ClipboardList, Edit3, Eye, PackageCheck,
  Plus, Search, Send, Trash2, X,
} from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Button, buttonVariants } from "@/components/ui/button";
import {
  Table, TableControlsBar, TableControlsPagination, TableControlsSearch, TableEmpty,
  TableError, TableLoading, TableScroll, TableWrap, TBody, TD, TH, THead, TR,
} from "@/components/ui/data-table";
import { Input, Select } from "@/components/ui/input";
import { usePageChrome } from "@/modules/admin/layouts/page-chrome";
import { hasPermission } from "@/modules/users/access-control";
import { useAuthStore } from "@/stores/auth-store";

import {
  StockRequestApprovalDrawer, StockRequestCloseDrawer, StockRequestCommandDrawer,
  StockRequestEditorDrawer, StockRequestFulfilDrawer, StockRequestReasonDrawer,
} from "./stock-request-drawers";
import { assetsInventoryService, responseMessage } from "./service";
import { formatStockQuantity } from "./stock-quantity";
import { StockFact, StockNotice, formatOperationalDate, formatOperationalDateTime } from "./stock-ui";
import type { StockRequest, StockRequestStatus, StockRequestSummary } from "./stock-request-types";

const requestStatuses: StockRequestStatus[] = ["draft", "submitted", "approved", "rejected", "cancelled", "partially_fulfilled", "fulfilled", "closed"];

export function StockRequestsWorkspace() {
  const navigate = useNavigate();
  const permissions = useAuthStore((state) => state.user?.permissions);
  const canRequest = hasPermission(permissions, "assets_inventory:request");
  const [requests, setRequests] = useState<StockRequestSummary[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [search, setSearch] = useState("");
  const [submittedSearch, setSubmittedSearch] = useState("");
  const [status, setStatus] = useState<StockRequestStatus | "all">("all");
  const [page, setPage] = useState(1);
  const [totalPages, setTotalPages] = useState(1);
  const [createOpen, setCreateOpen] = useState(false);

  const load = useCallback(async () => {
    setLoading(true); setError(null);
    try {
      const response = await assetsInventoryService.listStockRequests({ page, per_page: 20, search: submittedSearch || undefined, status: status === "all" ? undefined : status });
      if (!response.success || !response.data) throw new Error(responseMessage(response, "Stock requests could not be loaded"));
      setRequests(response.data.requests);
      setTotalPages(response.pagination?.total_pages ?? 1);
    } catch (loadError) { setError(loadError instanceof Error ? loadError.message : "Stock requests could not be loaded"); }
    finally { setLoading(false); }
  }, [page, status, submittedSearch]);

  useEffect(() => { void load(); }, [load]);
  const pageAction = useMemo(() => canRequest ? <Button onClick={() => setCreateOpen(true)}><Plus className="size-4" />New request</Button> : null, [canRequest]);
  usePageChrome("Requests", pageAction);
  const filtered = Boolean(submittedSearch) || status !== "all";

  return <div className="space-y-5">
    <p className="text-sm text-[var(--text-muted)]">Request stock for a department and track its decision and issue status.</p>
    <TableControlsBar>
      <TableControlsSearch onSubmit={(event) => { event.preventDefault(); setPage(1); setSubmittedSearch(search.trim()); }}>
        <Input aria-label="Search stock requests" leadingIcon={<Search />} onChange={(event) => setSearch(event.target.value)} placeholder="Search request, requester, or department…" value={search} />
        <Button type="submit" variant="secondary">Search</Button>
      </TableControlsSearch>
      <Select aria-label="Request status" className="sm:w-56" onChange={(event) => { setPage(1); setStatus(event.target.value as StockRequestStatus | "all"); }} value={status}>
        <option value="all">All statuses</option>{requestStatuses.map((value) => <option key={value} value={value}>{requestStatusLabel(value)}</option>)}
      </Select>
      {!loading && requests.length > 0 ? <TableControlsPagination onNext={() => setPage((value) => Math.min(totalPages, value + 1))} onPrevious={() => setPage((value) => Math.max(1, value - 1))} page={page} totalPages={totalPages} /> : null}
    </TableControlsBar>
    <TableWrap>
      {loading ? <TableLoading columns={7} label="Loading stock requests…" /> : error ? <TableError description={error} onRetry={() => void load()} title="Stock requests could not be loaded" /> : requests.length === 0 ? <TableEmpty description={filtered ? "Change the current filters." : canRequest ? "Create the first department stock request." : "No department stock requests have been recorded."} icon={<ClipboardList />} title={filtered ? "No requests match these filters" : "No stock requests yet"} /> : <TableScroll><Table>
        <THead><tr><TH>Request</TH><TH>Requester</TH><TH>Department</TH><TH>Needed by</TH><TH>Lines</TH><TH>Status</TH><TH className="text-right">Open</TH></tr></THead>
        <TBody>{requests.map((request) => <TR key={request.id}>
          <TD><p className="font-tabular font-medium text-[var(--text-strong)]">{request.request_number}</p><p className="mt-1 text-xs text-[var(--text-subtle)]">Updated {formatOperationalDateTime(request.updated_at)}</p></TD>
          <TD><p className="text-[var(--text-body)]">{request.requester_name || "Requester unavailable"}</p><p className="mt-1 font-tabular text-xs text-[var(--text-subtle)]">{request.requester_employee_number || "—"}</p></TD>
          <TD><p>{request.department_name || "Department unavailable"}</p><p className="mt-1 font-tabular text-xs text-[var(--text-subtle)]">{request.department_code || "—"}</p></TD>
          <TD className="whitespace-nowrap text-[var(--text-muted)]">{request.needed_by ? formatOperationalDate(request.needed_by) : "Not set"}</TD>
          <TD className="font-tabular text-[var(--text-muted)]">{request.line_count}</TD>
          <TD><StockRequestStatusBadge status={request.status} /></TD>
          <TD className="text-right"><Link aria-label={`Open ${request.request_number}`} className={buttonVariants({ variant: "ghost", size: "icon-sm" })} params={{ requestId: request.id }} to="/modules/assets-inventory/requests/$requestId"><Eye className="size-4" /></Link></TD>
        </TR>)}</TBody>
      </Table></TableScroll>}
    </TableWrap>
    <StockRequestEditorDrawer onClose={() => setCreateOpen(false)} onConflict={() => { setCreateOpen(false); void load(); }} onSaved={(created) => { setCreateOpen(false); if (created) void navigate({ to: "/modules/assets-inventory/requests/$requestId", params: { requestId: created.id } }); else void load(); }} open={createOpen} request={null} />
  </div>;
}

type DetailDrawer = "edit" | "submit" | "delete" | "approve" | "reject" | "cancel" | "fulfil" | "close" | null;

export function StockRequestDetail({ requestId }: { requestId: string }) {
  const navigate = useNavigate();
  const permissions = useAuthStore((state) => state.user?.permissions);
  const canRequest = hasPermission(permissions, "assets_inventory:request");
  const canApprove = hasPermission(permissions, "assets_inventory:approve");
  const canIssue = hasPermission(permissions, "assets_inventory:issue");
  const [request, setRequest] = useState<StockRequest | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [notFound, setNotFound] = useState(false);
  const [drawer, setDrawer] = useState<DetailDrawer>(null);

  const load = useCallback(async () => {
    setLoading(true); setError(null); setNotFound(false);
    try {
      const response = await assetsInventoryService.readStockRequest(requestId);
      if (!response.success || !response.data) {
        if (response.http_status === 404) { setRequest(null); setNotFound(true); return; }
        throw new Error(responseMessage(response, "Stock request could not be loaded"));
      }
      setRequest(response.data);
    } catch (loadError) { setError(loadError instanceof Error ? loadError.message : "Stock request could not be loaded"); }
    finally { setLoading(false); }
  }, [requestId]);

  useEffect(() => { void load(); }, [load]);
  const updateFromDrawer = (updated?: StockRequest) => { setDrawer(null); if (updated) setRequest(updated); else void load(); };
  const reloadAfterConflict = () => { setDrawer(null); void load(); };
  const pageActions = useMemo(() => request ? <div className="flex flex-wrap items-center gap-2">
    {canRequest && request.status === "draft" ? <><Button onClick={() => setDrawer("edit")} variant="secondary"><Edit3 className="size-4" />Edit</Button><Button onClick={() => setDrawer("submit")}><Send className="size-4" />Submit</Button></> : null}
    {canApprove && request.status === "submitted" ? <><Button onClick={() => setDrawer("reject")} variant="secondary"><X className="size-4" />Reject</Button><Button onClick={() => setDrawer("approve")}><Check className="size-4" />Approve</Button></> : null}
    {canIssue && (request.status === "approved" || request.status === "partially_fulfilled") ? <Button onClick={() => setDrawer("fulfil")}><PackageCheck className="size-4" />Fulfil</Button> : null}
  </div> : null, [canApprove, canIssue, canRequest, request]);
  usePageChrome(request?.request_number ?? "Stock request", pageActions);

  if (loading) return <div aria-label="Loading stock request" className="space-y-4" role="status"><div className="h-36 animate-pulse rounded-[var(--radius-xl)] bg-[var(--surface-sunken)]" /><div className="h-72 animate-pulse rounded-[var(--radius-xl)] bg-[var(--surface-sunken)]" /></div>;
  if (notFound) return <TableWrap><TableError description="This request does not exist or is not available in this campus." onRetry={() => void load()} title="Stock request not found" /></TableWrap>;
  if (error || !request) return <TableWrap><TableError description={error ?? "Stock request could not be opened"} onRetry={() => void load()} title="Stock request could not be opened" /></TableWrap>;

  const mayCancel = canRequest && (request.status === "submitted" || request.status === "approved");
  const mayClose = canApprove && request.status === "partially_fulfilled";
  return <div className="space-y-6">
    <section className="rounded-[var(--radius-xl)] border border-[var(--border)] bg-[var(--surface)] p-5 shadow-[var(--shadow-card)] sm:p-6">
      <Link className={buttonVariants({ variant: "ghost", size: "sm" })} to="/modules/assets-inventory/requests"><ArrowLeft className="size-4" />Back to requests</Link>
      <div className="mt-4 flex flex-wrap items-center gap-3"><h1 className="font-tabular text-xl font-semibold text-[var(--text-strong)]">{request.request_number}</h1><StockRequestStatusBadge status={request.status} /></div>
      <p className="mt-3 max-w-3xl whitespace-pre-wrap text-sm leading-6 text-[var(--text-body)]">{request.purpose}</p>
      {mayCancel || mayClose || (canRequest && request.status === "draft") ? <div className="mt-5 flex flex-wrap gap-2 border-t border-[var(--border-subtle)] pt-4">
        {mayCancel ? <Button onClick={() => setDrawer("cancel")} size="sm" variant="secondary"><Ban className="size-4" />Cancel request</Button> : null}
        {mayClose ? <Button onClick={() => setDrawer("close")} size="sm" variant="secondary"><ClipboardCheck className="size-4" />Close remainder</Button> : null}
        {canRequest && request.status === "draft" ? <Button className="text-[var(--tone-danger)]" onClick={() => setDrawer("delete")} size="sm" variant="ghost"><Trash2 className="size-4" />Remove draft</Button> : null}
      </div> : null}
    </section>

    <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
      <StockFact label="Requester" value={<><span>{request.requester_name || "Unavailable"}</span>{request.requester_employee_number ? <span className="mt-1 block font-tabular text-xs text-[var(--text-subtle)]">{request.requester_employee_number}</span> : null}</>} />
      <StockFact label="Department" value={<><span>{request.department_name || "Unavailable"}</span>{request.department_code ? <span className="mt-1 block font-tabular text-xs text-[var(--text-subtle)]">{request.department_code}</span> : null}</>} />
      <StockFact label="Needed by" value={request.needed_by ? formatOperationalDate(request.needed_by) : "Not set"} />
      <StockFact label="Last updated" value={formatOperationalDateTime(request.updated_at)} />
    </div>

    {request.decision_note ? <StockFact label="Decision note" value={request.decision_note} /> : null}
    {request.cancellation_note ? <StockNotice danger><strong>Cancellation:</strong> {request.cancellation_note}</StockNotice> : null}
    {request.closure_note ? <StockNotice><strong>Closure:</strong> {request.closure_note}</StockNotice> : null}

    <section aria-labelledby="request-lines-heading" className="space-y-3">
      <h2 className="text-base font-semibold text-[var(--text-strong)]" id="request-lines-heading">Items</h2>
      <TableWrap><TableScroll><Table>
        <THead><tr><TH>#</TH><TH>Item</TH><TH className="text-right">Requested</TH><TH className="text-right">Approved</TH><TH className="text-right">Issued</TH><TH className="text-right">Remaining</TH></tr></THead>
        <TBody>{request.lines.map((line) => <TR key={line.id}><TD className="font-tabular text-[var(--text-subtle)]">{line.line_number}</TD><TD><p className="font-medium text-[var(--text-strong)]">{line.item_name}</p><p className="mt-1 font-tabular text-xs text-[var(--text-subtle)]">{line.item_number}</p></TD><QuantityCell amount={line.requested_quantity_minor} scale={line.quantity_scale} unit={line.unit_label} /><QuantityCell amount={line.approved_quantity_minor} scale={line.quantity_scale} unit={line.unit_label} /><QuantityCell amount={line.issued_quantity_minor} scale={line.quantity_scale} unit={line.unit_label} /><QuantityCell amount={line.remaining_quantity_minor} scale={line.quantity_scale} unit={line.unit_label} /></TR>)}</TBody>
      </Table></TableScroll></TableWrap>
    </section>

    <section aria-labelledby="request-fulfilments-heading" className="space-y-3">
      <h2 className="text-base font-semibold text-[var(--text-strong)]" id="request-fulfilments-heading">Issues</h2>
      {request.fulfilments.length === 0 ? <StockNotice>No stock issues have been posted for this request.</StockNotice> : <div className="space-y-4">{request.fulfilments.map((fulfilment) => <article className="rounded-[var(--radius-xl)] border border-[var(--border)] bg-[var(--surface)] p-5" key={fulfilment.id}>
        <div className="flex flex-wrap items-center justify-between gap-3"><div><Link className="font-tabular text-sm font-semibold text-[var(--brand-strong)] hover:underline" params={{ movementId: fulfilment.movement_id }} to="/modules/assets-inventory/movements/$movementId">{fulfilment.movement_number}</Link><p className="mt-1 text-xs text-[var(--text-muted)]">Effective {formatOperationalDate(fulfilment.effective_on)} · posted {formatOperationalDateTime(fulfilment.created_at)}</p></div><Badge tone="success">Posted</Badge></div>
        <div className="mt-4 divide-y divide-[var(--border-subtle)]">{fulfilment.lines.map((line) => <div className="flex flex-wrap justify-between gap-3 py-3 text-sm" key={`${fulfilment.id}-${line.request_line_id}-${line.store_id}`}><span><strong className="font-medium text-[var(--text-strong)]">{line.item_name}</strong><span className="ml-2 text-[var(--text-muted)]">from {line.store_name}</span></span><span className="font-tabular font-medium">{formatStockQuantity(line.quantity_minor, line.quantity_scale)} {line.unit_label}</span></div>)}</div>
      </article>)}</div>}
    </section>

    <section aria-labelledby="request-history-heading" className="space-y-3">
      <h2 className="text-base font-semibold text-[var(--text-strong)]" id="request-history-heading">History</h2>
      <div className="rounded-[var(--radius-xl)] border border-[var(--border)] bg-[var(--surface)] px-5">{request.events.map((event, index) => <div className="flex gap-4 border-b border-[var(--border-subtle)] py-4 last:border-b-0" key={`${event.request_version}-${event.event_type}-${index}`}><span className="mt-1.5 size-2 shrink-0 rounded-full bg-[var(--brand)]" /><div className="min-w-0"><div className="flex flex-wrap items-center gap-2"><p className="text-sm font-medium text-[var(--text-strong)]">{requestEventLabel(event.event_type)}</p><span className="font-tabular text-xs text-[var(--text-subtle)]">v{event.request_version}</span></div>{event.note ? <p className="mt-1 whitespace-pre-wrap text-sm text-[var(--text-muted)]">{event.note}</p> : null}<p className="mt-1 text-xs text-[var(--text-subtle)]">{formatOperationalDateTime(event.created_at)}</p></div></div>)}</div>
    </section>

    <StockRequestEditorDrawer onClose={() => setDrawer(null)} onConflict={reloadAfterConflict} onSaved={updateFromDrawer} open={drawer === "edit"} request={request} />
    <StockRequestCommandDrawer command="submit" onClose={() => setDrawer(null)} onConflict={reloadAfterConflict} onSaved={updateFromDrawer} open={drawer === "submit"} request={request} />
    <StockRequestCommandDrawer command="delete" onClose={() => setDrawer(null)} onConflict={reloadAfterConflict} onSaved={() => { setDrawer(null); void navigate({ to: "/modules/assets-inventory/requests", replace: true }); }} open={drawer === "delete"} request={request} />
    <StockRequestApprovalDrawer onClose={() => setDrawer(null)} onConflict={reloadAfterConflict} onSaved={updateFromDrawer} open={drawer === "approve"} request={request} />
    <StockRequestReasonDrawer action="reject" onClose={() => setDrawer(null)} onConflict={reloadAfterConflict} onSaved={updateFromDrawer} open={drawer === "reject"} request={request} />
    <StockRequestReasonDrawer action="cancel" onClose={() => setDrawer(null)} onConflict={reloadAfterConflict} onSaved={updateFromDrawer} open={drawer === "cancel"} request={request} />
    <StockRequestFulfilDrawer onClose={() => setDrawer(null)} onConflict={reloadAfterConflict} onSaved={updateFromDrawer} open={drawer === "fulfil"} request={request} />
    <StockRequestCloseDrawer onClose={() => setDrawer(null)} onConflict={reloadAfterConflict} onSaved={updateFromDrawer} open={drawer === "close"} request={request} />
  </div>;
}

function QuantityCell({ amount, scale, unit }: { amount: number | null; scale: number; unit: string }) {
  return <TD className="whitespace-nowrap text-right font-tabular">{amount === null ? "—" : `${formatStockQuantity(amount, scale)} ${unit}`}</TD>;
}

export function StockRequestStatusBadge({ status }: { status: StockRequestStatus }) {
  const tone = status === "fulfilled" ? "success" : status === "approved" || status === "partially_fulfilled" ? "info" : status === "rejected" || status === "cancelled" ? "danger" : status === "submitted" ? "warning" : "neutral";
  return <Badge dot tone={tone}>{requestStatusLabel(status)}</Badge>;
}

export function requestStatusLabel(status: StockRequestStatus): string {
  return ({ draft: "Draft", submitted: "Submitted", approved: "Approved", rejected: "Rejected", cancelled: "Cancelled", partially_fulfilled: "Partially fulfilled", fulfilled: "Fulfilled", closed: "Closed" })[status];
}

function requestEventLabel(eventType: string): string {
  return ({ created: "Request created", updated: "Draft updated", submitted: "Request submitted", approved: "Request approved", rejected: "Request rejected", cancelled: "Request cancelled", fulfilled: "Stock issued", partially_fulfilled: "Stock partially issued", closed: "Request closed" } as Record<string, string>)[eventType] ?? eventType.split("_").join(" ");
}

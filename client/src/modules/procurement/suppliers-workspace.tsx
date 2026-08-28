/** Supplier directory with version-safe drawer workflows. */

import { useCallback, useEffect, useState } from "react";
import type { FormEvent } from "react";
import { Edit3, Eye, Loader2, PackageSearch, Plus, Search, Trash2 } from "lucide-react";
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

import { procurementService, responseMessage } from "./service";
import type { Supplier, SupplierInput, SupplierStatus } from "./types";

export function SuppliersWorkspace() {
  const permissions = useAuthStore((state) => state.user?.permissions);
  const canCreate = hasPermission(permissions, "procurement:create");
  const canEdit = hasPermission(permissions, "procurement:edit");
  const canDelete = hasPermission(permissions, "procurement:delete");
  const [suppliers, setSuppliers] = useState<Supplier[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [page, setPage] = useState(1);
  const [totalPages, setTotalPages] = useState(1);
  const [search, setSearch] = useState("");
  const [submittedSearch, setSubmittedSearch] = useState("");
  const [status, setStatus] = useState("all");
  const [drawer, setDrawer] = useState<Supplier | null | undefined>(undefined);
  const [viewSupplier, setViewSupplier] = useState<Supplier | null>(null);
  const [deleteSupplier, setDeleteSupplier] = useState<Supplier | null>(null);
  const [deleting, setDeleting] = useState(false);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const response = await procurementService.listSuppliers({
        page,
        per_page: 20,
        search: submittedSearch || undefined,
        status: status === "all" ? undefined : status,
      });
      if (!response.success || !response.data) throw new Error(responseMessage(response, "Suppliers could not be loaded"));
      setSuppliers(response.data.suppliers);
      setTotalPages(response.pagination?.total_pages ?? 1);
    } catch (loadError) {
      setError(loadError instanceof Error ? loadError.message : "Suppliers could not be loaded");
    } finally {
      setLoading(false);
    }
  }, [page, status, submittedSearch]);

  useEffect(() => { void load(); }, [load]);

  const remove = async () => {
    if (!deleteSupplier || deleting) return;
    setDeleting(true);
    try {
      const response = await procurementService.deleteSupplier(deleteSupplier.id, deleteSupplier.version);
      if (!response.success) throw new Error(responseMessage(response, "Supplier could not be removed"));
      toast.success("Supplier removed");
      setDeleteSupplier(null);
      await load();
    } catch (deleteError) {
      toast.error(deleteError instanceof Error ? deleteError.message : "Supplier could not be removed");
    } finally {
      setDeleting(false);
    }
  };

  usePageChrome("Suppliers", canCreate ? <Button onClick={() => setDrawer(null)}><Plus className="size-4" />Add supplier</Button> : null);
  const filtered = Boolean(submittedSearch) || status !== "all";

  return <div className="space-y-5">
    <p className="text-sm text-[var(--text-muted)]">Maintain suppliers available to Procurement requests.</p>
    <TableControlsBar>
      <TableControlsSearch onSubmit={(event) => { event.preventDefault(); setPage(1); setSubmittedSearch(search.trim()); }}>
        <Input aria-label="Search suppliers" leadingIcon={<Search />} onChange={(event) => setSearch(event.target.value)} placeholder="Search name or supplier number…" value={search} />
        <Button type="submit" variant="secondary">Search</Button>
      </TableControlsSearch>
      <Select aria-label="Supplier status" className="sm:w-44" onChange={(event) => { setPage(1); setStatus(event.target.value); }} value={status}>
        <option value="all">All statuses</option>
        <option value="active">Active</option>
        <option value="inactive">Inactive</option>
      </Select>
      {!loading && suppliers.length > 0 ? <TableControlsPagination onNext={() => setPage((value) => Math.min(totalPages, value + 1))} onPrevious={() => setPage((value) => Math.max(1, value - 1))} page={page} totalPages={totalPages} /> : null}
    </TableControlsBar>

    <TableWrap>
      {loading ? <TableLoading columns={6} label="Loading suppliers…" /> : error ? <TableError description={error} onRetry={() => void load()} /> : suppliers.length === 0 ? <TableEmpty description={filtered ? "Change the current filters." : "Add the first supplier used for purchasing."} icon={<PackageSearch />} title={filtered ? "No suppliers match these filters" : "No suppliers yet"} /> : <TableScroll><Table>
        <THead><tr><TH>Supplier</TH><TH>Registration</TH><TH>Contact</TH><TH>Status</TH><TH>Updated</TH><TH className="text-right">Actions</TH></tr></THead>
        <TBody>{suppliers.map((supplier) => <TR key={supplier.id}>
          <TD><p className="font-medium text-[var(--text-strong)]">{supplier.legal_name}</p><p className="mt-1 font-tabular text-xs text-[var(--text-subtle)]">{supplier.supplier_number}{supplier.trading_name ? ` · ${supplier.trading_name}` : ""}</p></TD>
          <TD><p className="text-[var(--text-body)]">{supplier.registration_number || "—"}</p>{supplier.tax_number ? <p className="mt-1 text-xs text-[var(--text-subtle)]">Tax {supplier.tax_number}</p> : null}</TD>
          <TD><p className="text-[var(--text-body)]">{supplier.email || supplier.phone || "—"}</p>{supplier.email && supplier.phone ? <p className="mt-1 text-xs text-[var(--text-subtle)]">{supplier.phone}</p> : null}</TD>
          <TD><Badge tone={supplier.status === "active" ? "success" : "neutral"}>{supplier.status}</Badge></TD>
          <TD className="whitespace-nowrap text-[var(--text-muted)]">{formatDate(supplier.updated_at)}</TD>
          <TD className="text-right"><div className="inline-flex gap-1">
            <Button aria-label={`View ${supplier.legal_name}`} onClick={() => setViewSupplier(supplier)} size="icon-sm" variant="ghost"><Eye className="size-4" /></Button>
            {canEdit ? <Button aria-label={`Edit ${supplier.legal_name}`} onClick={() => setDrawer(supplier)} size="icon-sm" variant="ghost"><Edit3 className="size-4" /></Button> : null}
            {canDelete && supplier.status === "inactive" ? <Button aria-label={`Remove ${supplier.legal_name}`} className="text-[var(--tone-danger)]" onClick={() => setDeleteSupplier(supplier)} size="icon-sm" variant="ghost"><Trash2 className="size-4" /></Button> : null}
          </div></TD>
        </TR>)}</TBody>
      </Table></TableScroll>}
    </TableWrap>

    <SupplierDrawer onClose={() => setDrawer(undefined)} onSaved={() => { setDrawer(undefined); void load(); }} open={drawer !== undefined} supplier={drawer ?? null} />
    <SupplierDetailDrawer onClose={() => setViewSupplier(null)} open={viewSupplier !== null} supplier={viewSupplier} />
    <ConfirmDrawer confirmLabel="Remove supplier" description={`Remove ${deleteSupplier?.supplier_number ?? "this supplier"}? The supplier must be inactive and unused by requisitions.`} isPending={deleting} onClose={() => setDeleteSupplier(null)} onConfirm={() => void remove()} open={deleteSupplier !== null} title="Remove supplier?" />
  </div>;
}

function SupplierDrawer({ onClose, onSaved, open, supplier }: { onClose: () => void; onSaved: () => void; open: boolean; supplier: Supplier | null }) {
  const [legalName, setLegalName] = useState("");
  const [tradingName, setTradingName] = useState("");
  const [registrationNumber, setRegistrationNumber] = useState("");
  const [taxNumber, setTaxNumber] = useState("");
  const [email, setEmail] = useState("");
  const [phone, setPhone] = useState("");
  const [address, setAddress] = useState("");
  const [status, setStatus] = useState<SupplierStatus>("active");
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    if (!open) return;
    setLegalName(supplier?.legal_name ?? "");
    setTradingName(supplier?.trading_name ?? "");
    setRegistrationNumber(supplier?.registration_number ?? "");
    setTaxNumber(supplier?.tax_number ?? "");
    setEmail(supplier?.email ?? "");
    setPhone(supplier?.phone ?? "");
    setAddress(supplier?.address ?? "");
    setStatus(supplier?.status ?? "active");
  }, [open, supplier]);

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    const payload: SupplierInput = {
      legal_name: legalName.trim(),
      trading_name: optional(tradingName),
      registration_number: optional(registrationNumber),
      tax_number: optional(taxNumber),
      email: optional(email),
      phone: optional(phone),
      address: optional(address),
    };
    setSaving(true);
    try {
      const response = supplier
        ? await procurementService.updateSupplier(supplier.id, { ...payload, status, expected_version: supplier.version })
        : await procurementService.createSupplier({ ...payload, idempotency_key: crypto.randomUUID() });
      if (!response.success) throw new Error(responseMessage(response, "Supplier could not be saved"));
      toast.success("Supplier saved");
      onSaved();
    } catch (saveError) {
      toast.error(saveError instanceof Error ? saveError.message : "Supplier could not be saved");
    } finally {
      setSaving(false);
    }
  };

  return <DialogShell onClose={saving ? () => undefined : onClose} open={open}>
    <DialogHeader onClose={saving ? undefined : onClose} title={supplier ? `Edit ${supplier.supplier_number}` : "Add supplier"} />
    <form onSubmit={submit}>
      <DialogBody className="space-y-5">
        <div><Label htmlFor="supplier-legal-name">Legal name</Label><Input className="mt-1.5" data-autofocus="true" id="supplier-legal-name" maxLength={180} onChange={(event) => setLegalName(event.target.value)} required value={legalName} /></div>
        <div><Label htmlFor="supplier-trading-name">Trading name</Label><Input className="mt-1.5" id="supplier-trading-name" maxLength={180} onChange={(event) => setTradingName(event.target.value)} value={tradingName} /></div>
        <div className="grid gap-5 sm:grid-cols-2"><div><Label htmlFor="supplier-registration">Registration number</Label><Input className="mt-1.5" id="supplier-registration" maxLength={100} onChange={(event) => setRegistrationNumber(event.target.value)} value={registrationNumber} /></div><div><Label htmlFor="supplier-tax">Tax number</Label><Input className="mt-1.5" id="supplier-tax" maxLength={100} onChange={(event) => setTaxNumber(event.target.value)} value={taxNumber} /></div></div>
        <div className="grid gap-5 sm:grid-cols-2"><div><Label htmlFor="supplier-email">Email</Label><Input className="mt-1.5" id="supplier-email" maxLength={254} onChange={(event) => setEmail(event.target.value)} type="email" value={email} /></div><div><Label htmlFor="supplier-phone">Phone</Label><Input className="mt-1.5" id="supplier-phone" maxLength={50} onChange={(event) => setPhone(event.target.value)} type="tel" value={phone} /></div></div>
        <div><Label htmlFor="supplier-address">Address</Label><Textarea className="mt-1.5 min-h-28" id="supplier-address" maxLength={1000} onChange={(event) => setAddress(event.target.value)} value={address} /></div>
        {supplier ? <div><Label htmlFor="supplier-status">Status</Label><Select className="mt-1.5" id="supplier-status" onChange={(event) => setStatus(event.target.value as SupplierStatus)} value={status}><option value="active">Active</option><option value="inactive">Inactive</option></Select></div> : null}
      </DialogBody>
      <DialogFooter><Button disabled={saving} onClick={onClose} type="button" variant="secondary">Cancel</Button><Button disabled={saving} type="submit">{saving ? <Loader2 className="size-4 animate-spin" /> : null}{saving ? "Saving…" : "Save supplier"}</Button></DialogFooter>
    </form>
  </DialogShell>;
}

function SupplierDetailDrawer({ onClose, open, supplier }: { onClose: () => void; open: boolean; supplier: Supplier | null }) {
  return <DialogShell onClose={onClose} open={open}>
    <DialogHeader onClose={onClose} title={supplier?.supplier_number ?? "Supplier"} />
    <DialogBody className="space-y-6">
      {supplier ? <>
        <div><div className="flex flex-wrap items-center gap-3"><h3 className="text-xl font-semibold text-[var(--text-strong)]">{supplier.legal_name}</h3><Badge tone={supplier.status === "active" ? "success" : "neutral"}>{supplier.status}</Badge></div>{supplier.trading_name ? <p className="mt-1 text-sm text-[var(--text-muted)]">{supplier.trading_name}</p> : null}</div>
        <div className="grid gap-4 sm:grid-cols-2"><Fact label="Registration number" value={supplier.registration_number || "—"} /><Fact label="Tax number" value={supplier.tax_number || "—"} /><Fact label="Email" value={supplier.email || "—"} /><Fact label="Phone" value={supplier.phone || "—"} /></div>
        <Fact label="Address" value={supplier.address || "—"} />
        <Fact label="Last updated" value={formatDateTime(supplier.updated_at)} />
      </> : null}
    </DialogBody>
    <DialogFooter><Button data-autofocus="true" onClick={onClose} type="button" variant="secondary">Close</Button></DialogFooter>
  </DialogShell>;
}

function Fact({ label, value }: { label: string; value: string }) {
  return <div className="rounded-[var(--radius-lg)] border border-[var(--border)] bg-[var(--surface-muted)] p-4"><p className="text-[11px] font-semibold uppercase tracking-[0.12em] text-[var(--text-subtle)]">{label}</p><p className="mt-1 break-words text-sm text-[var(--text-strong)]">{value}</p></div>;
}

function optional(value: string) { return value.trim() || null; }
function formatDate(value: string) { return new Intl.DateTimeFormat(undefined, { dateStyle: "medium" }).format(new Date(value)); }
function formatDateTime(value: string) { return new Intl.DateTimeFormat(undefined, { dateStyle: "medium", timeStyle: "short" }).format(new Date(value)); }

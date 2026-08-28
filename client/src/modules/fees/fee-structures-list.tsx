import { useCallback, useEffect, useMemo, useState } from "react";
import { Archive, Edit, Loader2, Plus, Power, ReceiptText, Search, Trash2 } from "lucide-react";
import toast from "react-hot-toast";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { ConfirmDrawer } from "@/components/ui/confirm-drawer";
import { Table, TableControlsBar, TableControlsPagination, TableControlsSearch, TableEmpty, TableError, TableLoading, TableScroll, TableWrap, TBody, TD, TH, THead, TR } from "@/components/ui/data-table";
import { DialogBody, DialogFooter, DialogHeader, DialogShell } from "@/components/ui/dialog";
import { Input, Label, Select, Textarea } from "@/components/ui/input";
import { usePageChrome } from "@/modules/admin/layouts/page-chrome";
import { hasPermission } from "@/modules/users/access-control";
import { useAuthStore } from "@/stores/auth-store";

import { feesService, responseMessage } from "./service";
import type { FeeStructure, FeeStructureInput, FeeStructureStatus, FeesReferenceData } from "./types";

type LifecycleAction = { kind: "activate" | "retire"; record: FeeStructure };

export function FeeStructuresList() {
  const permissions = useAuthStore((state) => state.user?.permissions);
  const canCreate = hasPermission(permissions, "fees:create");
  const canEdit = hasPermission(permissions, "fees:edit");
  const canDelete = hasPermission(permissions, "fees:delete");
  const [records, setRecords] = useState<FeeStructure[]>([]);
  const [references, setReferences] = useState<FeesReferenceData | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [page, setPage] = useState(1);
  const [totalPages, setTotalPages] = useState(1);
  const [search, setSearch] = useState("");
  const [submittedSearch, setSubmittedSearch] = useState("");
  const [status, setStatus] = useState("all");
  const [drawerRecord, setDrawerRecord] = useState<FeeStructure | null | undefined>(undefined);
  const [lifecycleAction, setLifecycleAction] = useState<LifecycleAction | null>(null);
  const [deleteRecord, setDeleteRecord] = useState<FeeStructure | null>(null);
  const [deleting, setDeleting] = useState(false);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const [structureResponse, referenceResponse] = await Promise.all([
        feesService.listFeeStructures({ page, per_page: 25, search: submittedSearch || undefined, status: status === "all" ? undefined : status }),
        feesService.referenceData(),
      ]);
      if (!structureResponse.success || !structureResponse.data) throw new Error(responseMessage(structureResponse, "Fee structures could not be loaded"));
      if (!referenceResponse.success || !referenceResponse.data) throw new Error(responseMessage(referenceResponse, "Fees reference data could not be loaded"));
      setRecords(structureResponse.data.fee_structures);
      setTotalPages(structureResponse.pagination?.total_pages ?? 1);
      setReferences(referenceResponse.data);
    } catch (loadError) {
      setError(loadError instanceof Error ? loadError.message : "Fee structures could not be loaded");
    } finally {
      setLoading(false);
    }
  }, [page, status, submittedSearch]);

  useEffect(() => { void load(); }, [load]);

  const remove = async () => {
    if (!deleteRecord || deleting) return;
    setDeleting(true);
    const response = await feesService.deleteFeeStructure(deleteRecord.id, deleteRecord.version);
    setDeleting(false);
    if (response.success) { toast.success("Fee structure removed"); setDeleteRecord(null); void load(); }
    else toast.error(responseMessage(response, "Fee structure could not be removed"));
  };

  usePageChrome("Fee structures", canCreate ? <Button disabled={!references || !canConfigure(references)} onClick={() => setDrawerRecord(null)}><Plus className="size-4" />Add fee structure</Button> : undefined);
  const filtered = Boolean(submittedSearch || status !== "all");

  return <div className="space-y-6">
    <p className="text-sm text-[var(--text-muted)]">Versioned fee definitions linked to Academics and Finance.</p>
    {!loading && references && !canConfigure(references) ? <section className="border border-[var(--tone-warn-bd)] bg-[var(--tone-warn-bg)] p-4 text-sm leading-6 text-[var(--text-body)]">An academic year, active currency, receivable account, and revenue account are required before a fee structure can be added.</section> : null}
    <TableControlsBar>
      <TableControlsSearch onSubmit={(event) => { event.preventDefault(); setPage(1); setSubmittedSearch(search.trim()); }}>
        <Input aria-label="Search fee structures" leadingIcon={<Search />} onChange={(event) => setSearch(event.target.value)} placeholder="Search code or name…" value={search} />
        <Button type="submit" variant="secondary">Search</Button>
      </TableControlsSearch>
      <Select aria-label="Fee structure status filter" className="sm:w-40" onChange={(event) => { setPage(1); setStatus(event.target.value); }} value={status}><option value="all">All statuses</option><option value="draft">Draft</option><option value="active">Active</option><option value="retired">Retired</option></Select>
      {!loading && records.length > 0 ? <TableControlsPagination onNext={() => setPage((value) => Math.min(totalPages, value + 1))} onPrevious={() => setPage((value) => Math.max(1, value - 1))} page={page} totalPages={totalPages} /> : null}
    </TableControlsBar>
    <TableWrap>
      {loading ? <TableLoading columns={7} label="Loading fee structures…" /> : error ? <TableError description={error} onRetry={() => void load()} /> : records.length === 0 ? <TableEmpty description={filtered ? "Change the current filters." : references && !canConfigure(references) ? "Complete the required setup above." : canCreate ? "Add the first fee structure." : "No fee structures are available."} icon={<ReceiptText />} title={filtered ? "No fee structures match these filters" : "No fee structures"} /> : <TableScroll><Table>
        <THead><tr><TH>Code</TH><TH>Fee</TH><TH>Academic scope</TH><TH>Amount</TH><TH>Posting accounts</TH><TH>Status</TH>{canEdit || canDelete ? <TH className="text-right">Actions</TH> : null}</tr></THead>
        <TBody>{records.map((record) => <TR key={record.id}>
          <TD className="font-tabular font-semibold text-[var(--text-strong)]">{record.code}</TD>
          <TD><span className="font-medium text-[var(--text-strong)]">{record.name}</span>{record.description ? <span className="mt-1 block max-w-64 truncate text-xs text-[var(--text-subtle)]">{record.description}</span> : null}</TD>
          <TD><AcademicScope record={record} references={references} /></TD>
          <TD className="font-tabular font-semibold">{formatAmount(record, references)}</TD>
          <TD><PostingAccounts record={record} references={references} /></TD>
          <TD><Badge tone={record.status === "active" ? "success" : record.status === "draft" ? "warning" : "neutral"}>{record.status}</Badge></TD>
          {canEdit || canDelete ? <TD className="text-right"><div className="inline-flex gap-1">
            {canEdit && record.status === "draft" ? <button aria-label={`Edit ${record.code}`} className={actionClass} onClick={() => setDrawerRecord(record)} type="button"><Edit className="size-4" /></button> : null}
            {canEdit && record.status === "draft" ? <button aria-label={`Activate ${record.code}`} className={actionClass} onClick={() => setLifecycleAction({ kind: "activate", record })} type="button"><Power className="size-4" /></button> : null}
            {canEdit && record.status === "active" ? <button aria-label={`Retire ${record.code}`} className={actionClass} onClick={() => setLifecycleAction({ kind: "retire", record })} type="button"><Archive className="size-4" /></button> : null}
            {canDelete && record.status === "draft" ? <button aria-label={`Remove ${record.code}`} className={`${actionClass} text-[var(--tone-danger)] hover:bg-[var(--tone-danger-bg)]`} onClick={() => setDeleteRecord(record)} type="button"><Trash2 className="size-4" /></button> : null}
          </div></TD> : null}
        </TR>)}</TBody>
      </Table></TableScroll>}
    </TableWrap>
    <FeeStructureDrawer onClose={() => setDrawerRecord(undefined)} onSaved={() => { setDrawerRecord(undefined); void load(); }} open={drawerRecord !== undefined} record={drawerRecord ?? null} references={references} />
    <LifecycleDrawer action={lifecycleAction} onClose={() => setLifecycleAction(null)} onDone={() => { setLifecycleAction(null); void load(); }} />
    <ConfirmDrawer confirmLabel="Remove fee structure" description={`Remove ${deleteRecord?.code ?? "this draft"}?`} isPending={deleting} onClose={() => setDeleteRecord(null)} onConfirm={() => void remove()} open={deleteRecord !== null} title="Remove draft fee structure?" />
  </div>;
}

const actionClass = "inline-flex size-9 items-center justify-center rounded-[var(--radius-md)] hover:bg-[var(--surface-muted)]";

function FeeStructureDrawer({ onClose, onSaved, open, record, references }: { onClose: () => void; onSaved: () => void; open: boolean; record: FeeStructure | null; references: FeesReferenceData | null }) {
  const [academicYearId, setAcademicYearId] = useState("");
  const [academicTermId, setAcademicTermId] = useState("");
  const [gradeLevelId, setGradeLevelId] = useState("");
  const [currencyId, setCurrencyId] = useState("");
  const [receivableAccountId, setReceivableAccountId] = useState("");
  const [revenueAccountId, setRevenueAccountId] = useState("");
  const [code, setCode] = useState("");
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [amount, setAmount] = useState("");
  const [saving, setSaving] = useState(false);

  const selectedCurrency = references?.currencies.find((item) => item.id === currencyId);
  const terms = useMemo(() => references?.academic_terms.filter((term) => term.academic_year_id === academicYearId) ?? [], [academicYearId, references]);

  useEffect(() => {
    if (!open) return;
    const currency = references?.currencies.find((item) => item.id === record?.currency_id) ?? references?.currencies[0];
    setAcademicYearId(record?.academic_year_id ?? references?.academic_years[0]?.id ?? "");
    setAcademicTermId(record?.academic_term_id ?? "");
    setGradeLevelId(record?.grade_level_id ?? "");
    setCurrencyId(record?.currency_id ?? currency?.id ?? "");
    setReceivableAccountId(record?.receivable_account_id ?? references?.receivable_accounts[0]?.id ?? "");
    setRevenueAccountId(record?.revenue_account_id ?? references?.revenue_accounts[0]?.id ?? "");
    setCode(record?.code ?? "");
    setName(record?.name ?? "");
    setDescription(record?.description ?? "");
    setAmount(record && currency ? exactAmount(record.amount_minor, currency.minor_units) : "");
  }, [open, record, references]);

  const submit = async (event: React.FormEvent) => {
    event.preventDefault();
    if (!selectedCurrency) return;
    const amountMinor = parseAmount(amount, selectedCurrency.minor_units);
    if (amountMinor === null || amountMinor <= 0) { toast.error(`Enter a valid amount in ${selectedCurrency.code}`); return; }
    const payload: FeeStructureInput = {
      academic_year_id: academicYearId,
      academic_term_id: academicTermId || null,
      grade_level_id: gradeLevelId || null,
      currency_id: currencyId,
      receivable_account_id: receivableAccountId,
      revenue_account_id: revenueAccountId,
      code: code.trim(),
      name: name.trim(),
      description: description.trim() || null,
      amount_minor: amountMinor,
    };
    setSaving(true);
    try {
      const response = record
        ? await feesService.updateFeeStructure(record.id, { ...payload, expected_version: record.version })
        : await feesService.createFeeStructure({ ...payload, idempotency_key: crypto.randomUUID() });
      if (!response.success) throw new Error(responseMessage(response, "Fee structure could not be saved"));
      toast.success("Fee structure saved");
      onSaved();
    } catch (saveError) {
      toast.error(saveError instanceof Error ? saveError.message : "Fee structure could not be saved");
    } finally {
      setSaving(false);
    }
  };

  return <DialogShell onClose={onClose} open={open}>
    <DialogHeader onClose={onClose} title={record ? `Edit ${record.code}` : "Add fee structure"} />
    <form onSubmit={submit}>
      <DialogBody className="space-y-5">
        <div className="grid gap-5 sm:grid-cols-[0.7fr_1.3fr]"><div><Label htmlFor="fee-code">Code</Label><Input className="mt-1.5" data-autofocus="true" id="fee-code" maxLength={40} onChange={(event) => setCode(event.target.value)} placeholder="TUITION" required value={code} /></div><div><Label htmlFor="fee-name">Name</Label><Input className="mt-1.5" id="fee-name" maxLength={160} onChange={(event) => setName(event.target.value)} placeholder="Tuition" required value={name} /></div></div>
        <div><Label htmlFor="fee-description">Description</Label><Textarea className="mt-1.5" id="fee-description" maxLength={1000} onChange={(event) => setDescription(event.target.value)} value={description} /></div>
        <div><Label htmlFor="fee-year">Academic year</Label><Select className="mt-1.5" id="fee-year" onChange={(event) => { setAcademicYearId(event.target.value); setAcademicTermId(""); }} required value={academicYearId}><option value="">Choose an academic year</option>{references?.academic_years.map((year) => <option key={year.id} value={year.id}>{year.name}</option>)}</Select></div>
        <div className="grid gap-5 sm:grid-cols-2"><div><Label htmlFor="fee-term">Academic term</Label><Select className="mt-1.5" id="fee-term" onChange={(event) => setAcademicTermId(event.target.value)} value={academicTermId}><option value="">All terms</option>{terms.map((term) => <option key={term.id} value={term.id}>{term.code} · {term.name}</option>)}</Select></div><div><Label htmlFor="fee-grade">Grade level</Label><Select className="mt-1.5" id="fee-grade" onChange={(event) => setGradeLevelId(event.target.value)} value={gradeLevelId}><option value="">All grades</option>{references?.grade_levels.map((grade) => <option key={grade.id} value={grade.id}>{grade.code} · {grade.name}</option>)}</Select></div></div>
        <div className="grid gap-5 sm:grid-cols-[0.7fr_1.3fr]"><div><Label htmlFor="fee-currency">Currency</Label><Select className="mt-1.5" id="fee-currency" onChange={(event) => setCurrencyId(event.target.value)} required value={currencyId}><option value="">Choose</option>{references?.currencies.map((currency) => <option key={currency.id} value={currency.id}>{currency.code}{currency.is_reporting ? " · Reporting" : ""}</option>)}</Select></div><div><Label htmlFor="fee-amount">Amount</Label><Input className="mt-1.5 font-tabular" id="fee-amount" inputMode="decimal" onChange={(event) => setAmount(event.target.value)} placeholder={selectedCurrency ? exactAmount(0, selectedCurrency.minor_units) : "0.00"} required value={amount} /></div></div>
        <div><Label htmlFor="fee-receivable">Receivable account</Label><Select className="mt-1.5" id="fee-receivable" onChange={(event) => setReceivableAccountId(event.target.value)} required value={receivableAccountId}><option value="">Choose an asset account</option>{references?.receivable_accounts.map((account) => <option key={account.id} value={account.id}>{account.code} · {account.name}</option>)}</Select></div>
        <div><Label htmlFor="fee-revenue">Revenue account</Label><Select className="mt-1.5" id="fee-revenue" onChange={(event) => setRevenueAccountId(event.target.value)} required value={revenueAccountId}><option value="">Choose an income account</option>{references?.revenue_accounts.map((account) => <option key={account.id} value={account.id}>{account.code} · {account.name}</option>)}</Select></div>
      </DialogBody>
      <DialogFooter><Button disabled={saving} onClick={onClose} type="button" variant="ghost">Cancel</Button><Button disabled={saving || !references || !canConfigure(references)} type="submit">{saving ? <><Loader2 className="size-4 animate-spin" />Saving…</> : "Save fee structure"}</Button></DialogFooter>
    </form>
  </DialogShell>;
}

function LifecycleDrawer({ action, onClose, onDone }: { action: LifecycleAction | null; onClose: () => void; onDone: () => void }) {
  const [pending, setPending] = useState(false);
  const activate = action?.kind === "activate";
  const submit = async () => {
    if (!action) return;
    setPending(true);
    const response = activate
      ? await feesService.activateFeeStructure(action.record.id, action.record.version)
      : await feesService.retireFeeStructure(action.record.id, action.record.version);
    setPending(false);
    if (!response.success) { toast.error(responseMessage(response, `Fee structure could not be ${activate ? "activated" : "retired"}`)); return; }
    toast.success(`Fee structure ${activate ? "activated" : "retired"}`);
    onDone();
  };
  return <DialogShell onClose={pending ? () => undefined : onClose} open={action !== null}>
    <DialogHeader onClose={pending ? undefined : onClose} title={activate ? "Activate fee structure?" : "Retire fee structure?"} />
    <DialogBody><div className="flex gap-4"><span className="flex size-10 shrink-0 items-center justify-center rounded-[9px] bg-[var(--brand-subtle)] text-[var(--brand-strong)]">{activate ? <Power className="size-5" /> : <Archive className="size-5" />}</span><p className="text-sm leading-6 text-[var(--text-muted)]">{activate ? `${action?.record.code ?? "This draft"} will become available for future billing records and can no longer be edited.` : `${action?.record.code ?? "This structure"} will remain in billing history but cannot be used for new billing records.`}</p></div></DialogBody>
    <DialogFooter><Button disabled={pending} onClick={onClose} type="button" variant="secondary">Cancel</Button><Button disabled={pending} onClick={() => void submit()} type="button">{pending ? <Loader2 className="size-4 animate-spin" /> : null}{activate ? "Activate" : "Retire"}</Button></DialogFooter>
  </DialogShell>;
}

function AcademicScope({ record, references }: { record: FeeStructure; references: FeesReferenceData | null }) {
  const year = references?.academic_years.find((item) => item.id === record.academic_year_id)?.name ?? "Academic year";
  const term = references?.academic_terms.find((item) => item.id === record.academic_term_id)?.code;
  const grade = references?.grade_levels.find((item) => item.id === record.grade_level_id)?.code;
  return <span className="text-sm text-[var(--text-body)]">{year}<span className="mt-1 block text-xs text-[var(--text-subtle)]">{[term ?? "All terms", grade ?? "All grades"].join(" · ")}</span></span>;
}

function PostingAccounts({ record, references }: { record: FeeStructure; references: FeesReferenceData | null }) {
  const receivable = references?.receivable_accounts.find((item) => item.id === record.receivable_account_id)?.code ?? "—";
  const revenue = references?.revenue_accounts.find((item) => item.id === record.revenue_account_id)?.code ?? "—";
  return <span className="font-tabular text-xs text-[var(--text-muted)]">Dr {receivable}<span className="mt-1 block">Cr {revenue}</span></span>;
}

function canConfigure(references: FeesReferenceData) {
  return references.academic_years.length > 0 && references.currencies.length > 0 && references.receivable_accounts.length > 0 && references.revenue_accounts.length > 0;
}

function exactAmount(amountMinor: number, minorUnits: number) {
  const value = String(Math.trunc(amountMinor)).padStart(minorUnits + 1, "0");
  return minorUnits === 0 ? value : `${value.slice(0, -minorUnits)}.${value.slice(-minorUnits)}`;
}

function parseAmount(value: string, minorUnits: number) {
  const normalized = value.trim();
  if (!/^\d+(\.\d*)?$/.test(normalized)) return null;
  const [whole, fraction = ""] = normalized.split(".");
  if (fraction.length > minorUnits) return null;
  const parsed = Number(`${whole}${fraction.padEnd(minorUnits, "0")}`);
  return Number.isSafeInteger(parsed) ? parsed : null;
}

function formatAmount(record: FeeStructure, references: FeesReferenceData | null) {
  const currency = references?.currencies.find((item) => item.id === record.currency_id);
  if (!currency) return String(record.amount_minor);
  return new Intl.NumberFormat(undefined, { style: "currency", currency: currency.code, minimumFractionDigits: currency.minor_units, maximumFractionDigits: currency.minor_units }).format(record.amount_minor / 10 ** currency.minor_units);
}

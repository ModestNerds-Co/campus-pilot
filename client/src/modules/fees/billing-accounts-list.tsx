import { useCallback, useEffect, useMemo, useState } from "react";
import { Edit, Loader2, Plus, ReceiptText, Search } from "lucide-react";
import toast from "react-hot-toast";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Table, TableControlsBar, TableControlsPagination, TableControlsSearch, TableEmpty, TableError, TableLoading, TableScroll, TableWrap, TBody, TD, TH, THead, TR } from "@/components/ui/data-table";
import { DialogBody, DialogFooter, DialogHeader, DialogShell } from "@/components/ui/dialog";
import { Input, Label, Select } from "@/components/ui/input";
import { usePageChrome } from "@/modules/admin/layouts/page-chrome";
import { hasPermission } from "@/modules/users/access-control";
import { useAuthStore } from "@/stores/auth-store";

import { feesService, responseMessage } from "./service";
import type { BillingAccount, BillingAccountStatus, LearnerCandidate } from "./types";

const statusLabels: Record<BillingAccountStatus, string> = {
  active: "Active",
  on_hold: "On hold",
  closed: "Closed",
};

export function BillingAccountsList() {
  const permissions = useAuthStore((state) => state.user?.permissions);
  const canCreate = hasPermission(permissions, "fees:create");
  const canEdit = hasPermission(permissions, "fees:edit");
  const [records, setRecords] = useState<BillingAccount[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [page, setPage] = useState(1);
  const [totalPages, setTotalPages] = useState(1);
  const [search, setSearch] = useState("");
  const [submittedSearch, setSubmittedSearch] = useState("");
  const [status, setStatus] = useState("all");
  const [drawerRecord, setDrawerRecord] = useState<BillingAccount | null | undefined>(undefined);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const response = await feesService.listBillingAccounts({
        page,
        per_page: 25,
        search: submittedSearch || undefined,
        status: status === "all" ? undefined : status,
      });
      if (!response.success || !response.data) throw new Error(responseMessage(response, "Billing accounts could not be loaded"));
      setRecords(response.data.billing_accounts);
      setTotalPages(response.pagination?.total_pages ?? 1);
    } catch (loadError) {
      setError(loadError instanceof Error ? loadError.message : "Billing accounts could not be loaded");
    } finally {
      setLoading(false);
    }
  }, [page, status, submittedSearch]);

  useEffect(() => { void load(); }, [load]);

  usePageChrome("Billing accounts", canCreate ? <Button onClick={() => setDrawerRecord(null)}><Plus className="size-4" />Open billing account</Button> : undefined);
  const filtered = Boolean(submittedSearch || status !== "all");

  return <div className="space-y-6">
    <p className="text-sm text-[var(--text-muted)]">Learner accounts used by Fees and Billing.</p>
    <TableControlsBar>
      <TableControlsSearch onSubmit={(event) => { event.preventDefault(); setPage(1); setSubmittedSearch(search.trim()); }}>
        <Input aria-label="Search billing accounts" leadingIcon={<Search />} onChange={(event) => setSearch(event.target.value)} placeholder="Search account number…" value={search} />
        <Button type="submit" variant="secondary">Search</Button>
      </TableControlsSearch>
      <Select aria-label="Billing account status filter" className="sm:w-40" onChange={(event) => { setPage(1); setStatus(event.target.value); }} value={status}>
        <option value="all">All statuses</option>
        <option value="active">Active</option>
        <option value="on_hold">On hold</option>
        <option value="closed">Closed</option>
      </Select>
      {!loading && records.length > 0 ? <TableControlsPagination onNext={() => setPage((value) => Math.min(totalPages, value + 1))} onPrevious={() => setPage((value) => Math.max(1, value - 1))} page={page} totalPages={totalPages} /> : null}
    </TableControlsBar>
    <TableWrap>
      {loading ? <TableLoading columns={6} label="Loading billing accounts…" /> : error ? <TableError description={error} onRetry={() => void load()} /> : records.length === 0 ? <TableEmpty description={filtered ? "Change the current filters." : canCreate ? "Open the first learner billing account." : "No billing account is linked to your learner record."} icon={<ReceiptText />} title={filtered ? "No billing accounts match these filters" : "No billing accounts"} /> : <TableScroll><Table>
        <THead><tr><TH>Account</TH><TH>Learner</TH><TH>Learner number</TH><TH>Opened</TH><TH>Status</TH>{canEdit ? <TH className="text-right">Actions</TH> : null}</tr></THead>
        <TBody>{records.map((record) => <TR key={record.id}>
          <TD className="font-tabular font-semibold text-[var(--text-strong)]">{record.account_number}</TD>
          <TD><span className="font-medium text-[var(--text-strong)]">{record.learner_name}</span><span className="mt-1 block text-xs capitalize text-[var(--text-subtle)]">{record.learner_status.replace(/_/g, " ")}</span></TD>
          <TD className="font-tabular">{record.learner_number}</TD>
          <TD className="font-tabular">{formatDate(record.opened_on)}</TD>
          <TD><Badge tone={record.status === "active" ? "success" : record.status === "on_hold" ? "warning" : "neutral"}>{statusLabels[record.status]}</Badge></TD>
          {canEdit ? <TD className="text-right"><button aria-label={`Edit ${record.account_number}`} className="inline-flex size-9 items-center justify-center rounded-[var(--radius-md)] hover:bg-[var(--surface-muted)] disabled:cursor-not-allowed disabled:opacity-35" disabled={record.status === "closed"} onClick={() => setDrawerRecord(record)} type="button"><Edit className="size-4" /></button></TD> : null}
        </TR>)}</TBody>
      </Table></TableScroll>}
    </TableWrap>
    <BillingAccountDrawer onClose={() => setDrawerRecord(undefined)} onSaved={() => { setDrawerRecord(undefined); void load(); }} open={drawerRecord !== undefined} record={drawerRecord ?? null} />
  </div>;
}

function BillingAccountDrawer({ onClose, onSaved, open, record }: { onClose: () => void; onSaved: () => void; open: boolean; record: BillingAccount | null }) {
  const [learners, setLearners] = useState<LearnerCandidate[]>([]);
  const [learnerId, setLearnerId] = useState("");
  const [openedOn, setOpenedOn] = useState(localDate());
  const [status, setStatus] = useState<BillingAccountStatus>("active");
  const [loadingReferences, setLoadingReferences] = useState(false);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    if (!open) return;
    setLearnerId("");
    setOpenedOn(record?.opened_on ?? localDate());
    setStatus(record?.status ?? "active");
    if (record) return;
    setLoadingReferences(true);
    void feesService.learnerCandidates().then((response) => {
      if (response.success && response.data) setLearners(response.data.learners);
      else toast.error(responseMessage(response, "Learners could not be loaded"));
    }).catch(() => toast.error("Learners could not be loaded")).finally(() => setLoadingReferences(false));
  }, [open, record]);

  const availableLearners = useMemo(() => learners.filter((learner) => learner.status !== "withdrawn"), [learners]);

  const submit = async (event: React.FormEvent) => {
    event.preventDefault();
    setSaving(true);
    try {
      const response = record
        ? await feesService.updateBillingAccount(record.id, status, record.version)
        : await feesService.createBillingAccount({ learner_id: learnerId, opened_on: openedOn, idempotency_key: crypto.randomUUID() });
      if (!response.success) throw new Error(responseMessage(response, "Billing account could not be saved"));
      toast.success(record ? "Billing account updated" : "Billing account opened");
      onSaved();
    } catch (saveError) {
      toast.error(saveError instanceof Error ? saveError.message : "Billing account could not be saved");
    } finally {
      setSaving(false);
    }
  };

  return <DialogShell onClose={onClose} open={open}>
    <DialogHeader onClose={onClose} title={record ? `Edit ${record.account_number}` : "Open billing account"} />
    <form onSubmit={submit}>
      <DialogBody className="space-y-5">
        {record ? <>
          <section className="border border-[var(--border)] bg-[var(--surface-muted)] p-4">
            <p className="text-sm font-semibold text-[var(--text-strong)]">{record.learner_name}</p>
            <p className="mt-1 font-tabular text-xs text-[var(--text-muted)]">{record.learner_number}</p>
          </section>
          <div><Label htmlFor="billing-status">Status</Label><Select className="mt-1.5" id="billing-status" onChange={(event) => setStatus(event.target.value as BillingAccountStatus)} value={status}><option value="active">Active</option><option value="on_hold">On hold</option><option value="closed">Closed</option></Select></div>
          {status === "closed" ? <p className="border border-[var(--tone-warn-bd)] bg-[var(--tone-warn-bg)] p-4 text-sm leading-6 text-[var(--text-body)]">A closed billing account cannot be reopened or edited.</p> : null}
        </> : <>
          <div><Label htmlFor="billing-learner">Learner</Label><Select className="mt-1.5" data-autofocus="true" disabled={loadingReferences} id="billing-learner" onChange={(event) => setLearnerId(event.target.value)} required value={learnerId}><option value="">{loadingReferences ? "Loading learners…" : "Choose a learner"}</option>{availableLearners.map((learner) => <option key={learner.id} value={learner.id}>{learner.display_name} · {learner.learner_number}</option>)}</Select></div>
          <div><Label htmlFor="billing-opened-on">Opened on</Label><Input className="mt-1.5" id="billing-opened-on" onChange={(event) => setOpenedOn(event.target.value)} required type="date" value={openedOn} /></div>
        </>}
      </DialogBody>
      <DialogFooter><Button disabled={saving} onClick={onClose} type="button" variant="ghost">Cancel</Button><Button disabled={saving || (!record && (!learnerId || loadingReferences))} type="submit">{saving ? <><Loader2 className="size-4 animate-spin" />Saving…</> : record ? "Save account" : "Open account"}</Button></DialogFooter>
    </form>
  </DialogShell>;
}

function localDate() {
  const now = new Date();
  return `${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, "0")}-${String(now.getDate()).padStart(2, "0")}`;
}

function formatDate(value: string) {
  return new Intl.DateTimeFormat(undefined, { day: "2-digit", month: "short", year: "numeric", timeZone: "UTC" }).format(new Date(`${value}T00:00:00Z`));
}

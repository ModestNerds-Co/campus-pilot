import { useCallback, useEffect, useMemo, useState } from "react";
import { Link } from "@tanstack/react-router";
import { Edit, KeyRound, Loader2, MoreVertical, Plus, Search, Trash2, UserRound, UsersRound } from "lucide-react";
import toast from "react-hot-toast";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { ConfirmDrawer } from "@/components/ui/confirm-drawer";
import {
  Table, TableControlsBar, TableControlsPagination, TableControlsSearch, TableEmpty,
  TableError, TableLoading, TableScroll, TableWrap, TBody, TD, TH, THead, TR,
} from "@/components/ui/data-table";
import { DialogBody, DialogFooter, DialogHeader, DialogShell } from "@/components/ui/dialog";
import { Input, Label, Select } from "@/components/ui/input";
import { usePageChrome } from "@/modules/admin/layouts/page-chrome";
import { useAuthStore } from "@/stores/auth-store";

import { responseMessage, sisService } from "./service";
import type { AccountCandidate, DirectoryStatus, Guardian, GuardianInput, Learner, LearnerInput, LearnerStatus } from "./types";

type PeopleKind = "learner" | "guardian";
type Person = Learner | Guardian;

export function SisPeopleList({ kind }: { kind: PeopleKind }) {
  const plural = kind === "learner" ? "Learners" : "Guardians";
  const permissions = useAuthStore((state) => state.user?.permissions ?? []);
  const canCreate = permissions.includes("*") || permissions.includes("sis:create");
  const canEdit = permissions.includes("*") || permissions.includes("sis:edit");
  const canDelete = permissions.includes("*") || permissions.includes("sis:delete");
  const [records, setRecords] = useState<Person[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [search, setSearch] = useState("");
  const [submittedSearch, setSubmittedSearch] = useState("");
  const [status, setStatus] = useState("all");
  const [page, setPage] = useState(1);
  const [totalPages, setTotalPages] = useState(1);
  const [formRecord, setFormRecord] = useState<Person | null | undefined>(undefined);
  const [accountRecord, setAccountRecord] = useState<Person | null>(null);
  const [deleteRecord, setDeleteRecord] = useState<Person | null>(null);
  const [menuId, setMenuId] = useState<string | null>(null);
  const [deleting, setDeleting] = useState(false);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const params = { page, per_page: 20, search: submittedSearch || undefined, status: status === "all" ? undefined : status };
      const response = kind === "learner" ? await sisService.listLearners(params) : await sisService.listGuardians(params);
      if (!response.success || !response.data) throw new Error(responseMessage(response, `${plural} could not be loaded`));
      setRecords("learners" in response.data ? response.data.learners : response.data.guardians);
      setTotalPages(response.pagination?.total_pages ?? 1);
    } catch (loadError) {
      setError(loadError instanceof Error ? loadError.message : `${plural} could not be loaded`);
    } finally {
      setLoading(false);
    }
  }, [kind, page, plural, status, submittedSearch]);

  useEffect(() => { void load(); }, [load]);

  const remove = async () => {
    if (!deleteRecord || deleting) return;
    setDeleting(true);
    const response = kind === "learner" ? await sisService.deleteLearner(deleteRecord.id) : await sisService.deleteGuardian(deleteRecord.id);
    setDeleting(false);
    if (!response.success) return toast.error(responseMessage(response, `${singular(kind)} could not be removed`));
    toast.success(`${capitalise(singular(kind))} removed`);
    setDeleteRecord(null);
    void load();
  };

  usePageChrome(plural, canCreate ? <Button onClick={() => setFormRecord(null)}><Plus className="size-4" />Add {singular(kind)}</Button> : null);
  const filtered = submittedSearch || status !== "all";
  const EmptyIcon = kind === "learner" ? UserRound : UsersRound;

  return <div className="space-y-6">
    <p className="text-sm text-[var(--text-muted)]">{kind === "learner" ? "Learner records are used by admissions, enrolment, classes, and reporting." : "Guardian records can be connected to one or more learners."}</p>
    <TableControlsBar>
      <TableControlsSearch onSubmit={(event) => { event.preventDefault(); setPage(1); setSubmittedSearch(search.trim()); }}>
        <Input aria-label={`Search ${plural.toLowerCase()}`} leadingIcon={<Search />} onChange={(event) => setSearch(event.target.value)} placeholder={`Search ${plural.toLowerCase()}…`} value={search} />
        <Button type="submit" variant="secondary">Search</Button>
      </TableControlsSearch>
      <Select aria-label="Status filter" className="sm:w-40" onChange={(event) => { setPage(1); setStatus(event.target.value); }} value={status}>
        <option value="all">All statuses</option>
        {kind === "learner" ? <><option value="prospective">Prospective</option><option value="active">Active</option><option value="inactive">Inactive</option><option value="graduated">Graduated</option><option value="withdrawn">Withdrawn</option></> : <><option value="active">Active</option><option value="inactive">Inactive</option></>}
      </Select>
      {!loading && records.length > 0 ? <TableControlsPagination onNext={() => setPage((value) => Math.min(totalPages, value + 1))} onPrevious={() => setPage((value) => Math.max(1, value - 1))} page={page} totalPages={totalPages} /> : null}
    </TableControlsBar>
    <TableWrap>{loading ? <TableLoading columns={6} label={`Loading ${plural.toLowerCase()}…`} /> : error ? <TableError description={error} onRetry={() => void load()} /> : records.length === 0 ? <TableEmpty description={filtered ? "Change the current filters." : `Add the first ${singular(kind)}.`} icon={<EmptyIcon />} title={filtered ? `No ${plural.toLowerCase()} match these filters` : `No ${plural.toLowerCase()} yet`} /> : <TableScroll><Table><THead><tr><TH>{kind === "learner" ? "Learner" : "Guardian"}</TH>{kind === "learner" ? <TH>Date of birth</TH> : null}<TH>Contact</TH><TH>Login account</TH><TH>Status</TH><TH className="text-right">Actions</TH></tr></THead><TBody>
      {records.map((record) => (
        <TR key={record.id}>
          <TD>
            {isLearner(record) ? (
              <Link className="font-medium text-[var(--text-strong)] hover:text-[var(--brand-strong)] hover:underline" params={{ learnerId: record.id }} to="/modules/sis/learners/$learnerId">
                {record.display_name}
              </Link>
            ) : <div className="font-medium text-[var(--text-strong)]">{record.display_name}</div>}
            <div className="font-tabular text-xs text-[var(--text-muted)]">{isLearner(record) ? record.learner_number : record.email || record.phone}</div>
          </TD>
          {isLearner(record) ? <TD className="text-[var(--text-muted)]">{formatDate(record.date_of_birth)}</TD> : null}
          <TD className="text-[var(--text-muted)]">{record.email || record.phone || "—"}</TD>
          <TD>{record.account_email ? <span className="text-[var(--text-strong)]">{record.account_email}</span> : <span className="text-[var(--text-muted)]">Not linked</span>}</TD>
          <TD><Badge tone={record.status === "active" ? "success" : record.status === "prospective" ? "warning" : "neutral"}>{displayStatus(record.status)}</Badge></TD>
          <TD className="text-right">
            {canEdit || canDelete ? <div className="relative inline-flex">
              <button aria-label={`${capitalise(singular(kind))} actions`} className="inline-flex size-8 items-center justify-center rounded-[var(--radius-md)] hover:bg-[var(--surface-muted)]" onClick={() => setMenuId(menuId === record.id ? null : record.id)} type="button"><MoreVertical className="size-4" /></button>
              {menuId === record.id ? <div className="absolute right-0 top-9 z-10 w-48 rounded-[var(--radius-lg)] border border-[var(--border)] bg-[var(--surface)] py-1 shadow-[var(--shadow-popover)]">
                {canEdit ? <><button className="flex w-full items-center gap-2 px-4 py-2 text-sm hover:bg-[var(--surface-muted)]" onClick={() => { setFormRecord(record); setMenuId(null); }} type="button"><Edit className="size-4" />Edit</button><button className="flex w-full items-center gap-2 px-4 py-2 text-sm hover:bg-[var(--surface-muted)]" onClick={() => { setAccountRecord(record); setMenuId(null); }} type="button"><KeyRound className="size-4" />{record.account_id ? "Change login" : "Link login"}</button></> : null}
                {canDelete ? <button className="flex w-full items-center gap-2 px-4 py-2 text-sm text-[var(--tone-danger)] hover:bg-[var(--tone-danger-bg)]" onClick={() => { setDeleteRecord(record); setMenuId(null); }} type="button"><Trash2 className="size-4" />Remove</button> : null}
              </div> : null}
            </div> : <span className="text-[var(--text-subtle)]">—</span>}
          </TD>
        </TR>
      ))}
    </TBody></Table></TableScroll>}</TableWrap>
    <SisPersonDrawer kind={kind} onClose={() => setFormRecord(undefined)} onSaved={() => { setFormRecord(undefined); void load(); }} open={formRecord !== undefined} record={formRecord ?? null} />
    <SisAccountDrawer kind={kind} onClose={() => setAccountRecord(null)} onSaved={() => { setAccountRecord(null); void load(); }} open={accountRecord !== null} record={accountRecord} />
    <ConfirmDrawer confirmLabel={`Remove ${singular(kind)}`} description={kind === "learner" ? `Remove ${deleteRecord?.display_name || "this learner"}? Relationships, applications, and enrolments must be removed first.` : `Remove ${deleteRecord?.display_name || "this guardian"}? Learner relationships must be removed first.`} isPending={deleting} onClose={() => setDeleteRecord(null)} onConfirm={() => void remove()} open={deleteRecord !== null} title={`Remove ${singular(kind)}?`} />
  </div>;
}

export function SisPersonDrawer({ kind, onClose, onSaved, open, record }: { kind: PeopleKind; onClose: () => void; onSaved: () => void; open: boolean; record: Person | null }) {
  const [displayName, setDisplayName] = useState("");
  const [firstNames, setFirstNames] = useState("");
  const [surname, setSurname] = useState("");
  const [dateOfBirth, setDateOfBirth] = useState("");
  const [email, setEmail] = useState("");
  const [phone, setPhone] = useState("");
  const [status, setStatus] = useState<string>(kind === "learner" ? "prospective" : "active");
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    if (!open) return;
    setDisplayName(record?.display_name ?? "");
    setFirstNames(record?.first_names ?? "");
    setSurname(record?.surname ?? "");
    setDateOfBirth(record && isLearner(record) ? record.date_of_birth : "");
    setEmail(record?.email ?? "");
    setPhone(record?.phone ?? "");
    setStatus(record?.status ?? (kind === "learner" ? "prospective" : "active"));
  }, [kind, open, record]);

  const submit = async (event: React.FormEvent) => {
    event.preventDefault();
    if (kind === "guardian" && !email.trim() && !phone.trim()) return toast.error("Enter an email address or phone number");
    setSaving(true);
    const common = { display_name: displayName.trim(), first_names: firstNames.trim() || null, surname: surname.trim() || null, email: email.trim() || null, phone: phone.trim() || null };
    const response = kind === "learner"
      ? record ? await sisService.updateLearner(record.id, { ...common, date_of_birth: dateOfBirth, status: status as LearnerStatus }) : await sisService.createLearner({ ...common, date_of_birth: dateOfBirth, status: status as LearnerStatus })
      : record ? await sisService.updateGuardian(record.id, { ...common, status: status as DirectoryStatus }) : await sisService.createGuardian({ ...common, status: status as DirectoryStatus });
    setSaving(false);
    if (!response.success) return toast.error(responseMessage(response, `${capitalise(singular(kind))} could not be saved`));
    toast.success(`${capitalise(singular(kind))} saved`);
    onSaved();
  };

  return <DialogShell onClose={onClose} open={open}><DialogHeader onClose={onClose} title={`${record ? "Edit" : "Add"} ${singular(kind)}`} /><form onSubmit={submit}><DialogBody className="space-y-5">
    {record && isLearner(record) ? <dl><dt className="text-sm font-medium leading-none text-[var(--text-strong)]">Learner number</dt><dd className="mt-1.5 rounded-[var(--radius-md)] border border-[var(--border)] bg-[var(--surface-muted)] px-3 py-2.5 font-tabular text-sm text-[var(--text-strong)]">{record.learner_number}</dd></dl> : null}
    <div><Label>Display name</Label><Input className="mt-1.5" data-autofocus="true" maxLength={200} onChange={(event) => setDisplayName(event.target.value)} required value={displayName} /></div>
    <div className="grid gap-4 sm:grid-cols-2"><div><Label>First names</Label><Input className="mt-1.5" maxLength={120} onChange={(event) => setFirstNames(event.target.value)} value={firstNames} /></div><div><Label>Surname</Label><Input className="mt-1.5" maxLength={120} onChange={(event) => setSurname(event.target.value)} value={surname} /></div></div>
    {kind === "learner" ? <div><Label>Date of birth</Label><Input className="mt-1.5" max={new Date().toISOString().slice(0, 10)} onChange={(event) => setDateOfBirth(event.target.value)} required type="date" value={dateOfBirth} /></div> : null}
    <div className="grid gap-4 sm:grid-cols-2"><div><Label>Email</Label><Input className="mt-1.5" onChange={(event) => setEmail(event.target.value)} type="email" value={email} /></div><div><Label>Phone</Label><Input className="mt-1.5" maxLength={50} onChange={(event) => setPhone(event.target.value)} value={phone} /></div></div>
    <div><Label>Status</Label><Select className="mt-1.5" onChange={(event) => setStatus(event.target.value)} value={status}>{kind === "learner" ? <><option value="prospective">Prospective</option><option value="active">Active</option><option value="inactive">Inactive</option><option value="graduated">Graduated</option><option value="withdrawn">Withdrawn</option></> : <><option value="active">Active</option><option value="inactive">Inactive</option></>}</Select></div>
  </DialogBody><DialogFooter><Button disabled={saving} onClick={onClose} type="button" variant="ghost">Cancel</Button><Button disabled={saving} type="submit">{saving ? <><Loader2 className="size-4 animate-spin" />Saving…</> : "Save"}</Button></DialogFooter></form></DialogShell>;
}

export function SisAccountDrawer({ kind, onClose, onSaved, open, record }: { kind: PeopleKind; onClose: () => void; onSaved: () => void; open: boolean; record: Person | null }) {
  const [accounts, setAccounts] = useState<AccountCandidate[]>([]);
  const [accountId, setAccountId] = useState("");
  const [search, setSearch] = useState("");
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    if (!open || !record) return;
    setAccountId(record.account_id ?? "");
    setSearch("");
    setLoading(true);
    void sisService.listAccountCandidates(kind, record.id).then((response) => { if (response.success && response.data) setAccounts(response.data.accounts); }).finally(() => setLoading(false));
  }, [kind, open, record]);

  const filtered = useMemo(() => {
    const query = search.trim().toLowerCase();
    return query ? accounts.filter((account) => `${account.full_name} ${account.email}`.toLowerCase().includes(query)) : accounts;
  }, [accounts, search]);

  const submit = async (event: React.FormEvent) => {
    event.preventDefault();
    if (!record) return;
    setSaving(true);
    const response = kind === "learner" ? await sisService.linkLearnerAccount(record.id, accountId || null) : await sisService.linkGuardianAccount(record.id, accountId || null);
    setSaving(false);
    if (!response.success) return toast.error(responseMessage(response, "Login account could not be linked"));
    toast.success(accountId ? "Login account linked" : "Login account unlinked");
    onSaved();
  };

  return <DialogShell onClose={onClose} open={open}><DialogHeader onClose={onClose} title="Login account" /><form onSubmit={submit}><DialogBody className="space-y-5">
    <div className="bg-[var(--surface-muted)] p-4"><p className="font-medium text-[var(--text-strong)]">{record?.display_name}</p><p className="mt-1 text-xs text-[var(--text-muted)]">The person record remains available without a login account.</p></div>
    <div><Label htmlFor="account-search">Find an active account</Label><Input className="mt-1.5" id="account-search" leadingIcon={<Search />} onChange={(event) => setSearch(event.target.value)} placeholder="Search name or email…" value={search} /></div>
    <div className="divide-y divide-[var(--border-subtle)] rounded-[var(--radius-lg)] border border-[var(--border)]">
      <label className={`flex cursor-pointer items-start gap-3 p-4 hover:bg-[var(--surface-muted)] ${accountId === "" ? "bg-[var(--brand-soft)]" : ""}`}><input checked={accountId === ""} className="mt-1" name="account" onChange={() => setAccountId("")} type="radio" /><span><span className="block text-sm font-medium text-[var(--text-strong)]">No login account</span><span className="mt-0.5 block text-xs text-[var(--text-muted)]">Keep only the SIS person record.</span></span></label>
      {loading ? <p className="p-4 text-sm text-[var(--text-muted)]">Loading accounts…</p> : filtered.map((account) => <label className={`flex cursor-pointer items-start gap-3 p-4 hover:bg-[var(--surface-muted)] ${accountId === account.id ? "bg-[var(--brand-soft)]" : ""}`} key={account.id}><input checked={accountId === account.id} className="mt-1" name="account" onChange={() => setAccountId(account.id)} type="radio" /><span><span className="block text-sm font-medium text-[var(--text-strong)]">{account.full_name}</span><span className="mt-0.5 block text-xs text-[var(--text-muted)]">{account.email}</span></span></label>)}
    </div>
  </DialogBody><DialogFooter><Button disabled={saving} onClick={onClose} type="button" variant="ghost">Cancel</Button><Button disabled={saving || loading} type="submit">{saving ? <><Loader2 className="size-4 animate-spin" />Saving…</> : "Save account link"}</Button></DialogFooter></form></DialogShell>;
}

function isLearner(record: Person): record is Learner { return "learner_number" in record; }
function singular(kind: PeopleKind) { return kind === "learner" ? "learner" : "guardian"; }
function capitalise(value: string) { return value.charAt(0).toUpperCase() + value.slice(1); }
function displayStatus(value: string) { return value.replace(/_/g, " "); }
function formatDate(value: string) { return new Intl.DateTimeFormat(undefined, { day: "numeric", month: "short", year: "numeric", timeZone: "UTC" }).format(new Date(`${value}T00:00:00Z`)); }

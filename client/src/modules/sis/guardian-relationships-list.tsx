import { useCallback, useEffect, useState } from "react";
import { Edit, Link2, Loader2, MoreVertical, Plus, Search, Trash2 } from "lucide-react";
import toast from "react-hot-toast";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { ConfirmDrawer } from "@/components/ui/confirm-drawer";
import { Table, TableControlsBar, TableControlsPagination, TableControlsSearch, TableEmpty, TableError, TableLoading, TableScroll, TableWrap, TBody, TD, TH, THead, TR } from "@/components/ui/data-table";
import { DialogBody, DialogFooter, DialogHeader, DialogShell } from "@/components/ui/dialog";
import { Input, Label, Select } from "@/components/ui/input";
import { usePageChrome } from "@/modules/admin/layouts/page-chrome";
import { useAuthStore } from "@/stores/auth-store";

import { responseMessage, sisService } from "./service";
import type { DirectoryStatus, Guardian, GuardianRelationship, Learner, RelationshipType } from "./types";

export function GuardianRelationshipsList() {
  const permissions = useAuthStore((state) => state.user?.permissions ?? []);
  const canCreate = permissions.includes("*") || permissions.includes("sis:create");
  const canEdit = permissions.includes("*") || permissions.includes("sis:edit");
  const canDelete = permissions.includes("*") || permissions.includes("sis:delete");
  const canMutate = canEdit || canDelete;
  const [records, setRecords] = useState<GuardianRelationship[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [search, setSearch] = useState("");
  const [submittedSearch, setSubmittedSearch] = useState("");
  const [status, setStatus] = useState("all");
  const [page, setPage] = useState(1);
  const [totalPages, setTotalPages] = useState(1);
  const [drawerRecord, setDrawerRecord] = useState<GuardianRelationship | null | undefined>(undefined);
  const [deleteRecord, setDeleteRecord] = useState<GuardianRelationship | null>(null);
  const [menuId, setMenuId] = useState<string | null>(null);
  const [deleting, setDeleting] = useState(false);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const response = await sisService.listGuardianRelationships({ page, per_page: 20, search: submittedSearch || undefined, status: status === "all" ? undefined : status });
      if (!response.success || !response.data) throw new Error(responseMessage(response, "Guardian relationships could not be loaded"));
      setRecords(response.data.relationships);
      setTotalPages(response.pagination?.total_pages ?? 1);
    } catch (loadError) {
      setError(loadError instanceof Error ? loadError.message : "Guardian relationships could not be loaded");
    } finally { setLoading(false); }
  }, [page, status, submittedSearch]);

  useEffect(() => { void load(); }, [load]);
  useEffect(() => {
    if (drawerRecord !== undefined && !(drawerRecord ? canEdit : canCreate)) setDrawerRecord(undefined);
    if (!canDelete) setDeleteRecord(null);
    if (!canMutate) setMenuId(null);
  }, [canCreate, canDelete, canEdit, canMutate, drawerRecord]);

  const remove = async () => {
    if (!canDelete || !deleteRecord || deleting) return;
    setDeleting(true);
    const response = await sisService.deleteGuardianRelationship(deleteRecord.id);
    setDeleting(false);
    if (!response.success) return toast.error(responseMessage(response, "Relationship could not be removed"));
    toast.success("Guardian relationship removed");
    setDeleteRecord(null);
    void load();
  };

  usePageChrome("Guardian relationships", canCreate ? <Button onClick={() => setDrawerRecord(null)}><Plus className="size-4" />Add relationship</Button> : null);
  const filtered = submittedSearch || status !== "all";

  return <div className="space-y-6">
    <p className="text-sm text-[var(--text-muted)]">Connect each learner to the guardians responsible for collection and communications.</p>
    <TableControlsBar><TableControlsSearch onSubmit={(event) => { event.preventDefault(); setPage(1); setSubmittedSearch(search.trim()); }}><Input aria-label="Search guardian relationships" leadingIcon={<Search />} onChange={(event) => setSearch(event.target.value)} placeholder="Search learner or guardian…" value={search} /><Button type="submit" variant="secondary">Search</Button></TableControlsSearch><Select aria-label="Status filter" className="sm:w-40" onChange={(event) => { setPage(1); setStatus(event.target.value); }} value={status}><option value="all">All statuses</option><option value="active">Active</option><option value="inactive">Inactive</option></Select>{!loading && records.length > 0 ? <TableControlsPagination onNext={() => setPage((value) => Math.min(totalPages, value + 1))} onPrevious={() => setPage((value) => Math.max(1, value - 1))} page={page} totalPages={totalPages} /> : null}</TableControlsBar>
    <TableWrap>{loading ? <TableLoading columns={canMutate ? 6 : 5} label="Loading guardian relationships…" /> : error ? <TableError description={error} onRetry={() => void load()} /> : records.length === 0 ? <TableEmpty description={filtered ? "Change the current filters." : "Add the first learner and guardian connection."} icon={<Link2 />} title={filtered ? "No relationships match these filters" : "No guardian relationships yet"} /> : <TableScroll><Table><THead><tr><TH>Learner</TH><TH>Guardian</TH><TH>Relationship</TH><TH>Responsibilities</TH><TH>Status</TH>{canMutate ? <TH className="text-right">Actions</TH> : null}</tr></THead><TBody>{records.map((record) => <TR key={record.id}><TD><div className="font-medium text-[var(--text-strong)]">{record.learner_name}</div><div className="font-tabular text-xs text-[var(--text-muted)]">{record.learner_number}</div></TD><TD className="font-medium text-[var(--text-strong)]">{record.guardian_name}</TD><TD className="capitalize text-[var(--text-muted)]">{record.relationship_type}</TD><TD className="text-[var(--text-muted)]">{responsibilities(record)}</TD><TD><Badge tone={record.status === "active" ? "success" : "neutral"}>{record.status}</Badge></TD>{canMutate ? <TD className="text-right"><div className="relative inline-flex"><button aria-label="Guardian relationship actions" className="inline-flex size-8 items-center justify-center rounded-[var(--radius-md)] hover:bg-[var(--surface-muted)]" onClick={() => setMenuId(menuId === record.id ? null : record.id)} type="button"><MoreVertical className="size-4" /></button>{menuId === record.id ? <div className="absolute right-0 top-9 z-10 w-40 rounded-[var(--radius-lg)] border border-[var(--border)] bg-[var(--surface)] py-1 shadow-[var(--shadow-popover)]">{canEdit ? <button className="flex w-full items-center gap-2 px-4 py-2 text-sm hover:bg-[var(--surface-muted)]" onClick={() => { setDrawerRecord(record); setMenuId(null); }} type="button"><Edit className="size-4" />Edit</button> : null}{canDelete ? <button className="flex w-full items-center gap-2 px-4 py-2 text-sm text-[var(--tone-danger)] hover:bg-[var(--tone-danger-bg)]" onClick={() => { setDeleteRecord(record); setMenuId(null); }} type="button"><Trash2 className="size-4" />Remove</button> : null}</div> : null}</div></TD> : null}</TR>)}</TBody></Table></TableScroll>}</TableWrap>
    <RelationshipDrawer onClose={() => setDrawerRecord(undefined)} onSaved={() => { setDrawerRecord(undefined); void load(); }} open={drawerRecord !== undefined && (drawerRecord ? canEdit : canCreate)} record={drawerRecord ?? null} />
    <ConfirmDrawer confirmLabel="Remove relationship" description={`Remove the connection between ${deleteRecord?.learner_name || "this learner"} and ${deleteRecord?.guardian_name || "this guardian"}?`} isPending={deleting} onClose={() => setDeleteRecord(null)} onConfirm={() => void remove()} open={canDelete && deleteRecord !== null} title="Remove guardian relationship?" />
  </div>;
}

function RelationshipDrawer({ onClose, onSaved, open, record }: { onClose: () => void; onSaved: () => void; open: boolean; record: GuardianRelationship | null }) {
  const [learners, setLearners] = useState<Learner[]>([]);
  const [guardians, setGuardians] = useState<Guardian[]>([]);
  const [learnerId, setLearnerId] = useState("");
  const [guardianId, setGuardianId] = useState("");
  const [relationshipType, setRelationshipType] = useState<RelationshipType>("guardian");
  const [isPrimary, setIsPrimary] = useState(false);
  const [canCollect, setCanCollect] = useState(false);
  const [receivesCommunications, setReceivesCommunications] = useState(true);
  const [status, setStatus] = useState<DirectoryStatus>("active");
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    if (!open) return;
    setLearnerId(record?.learner_id ?? ""); setGuardianId(record?.guardian_id ?? "");
    setRelationshipType(record?.relationship_type ?? "guardian"); setIsPrimary(record?.is_primary ?? false);
    setCanCollect(record?.can_collect ?? false); setReceivesCommunications(record?.receives_communications ?? true);
    setStatus(record?.status ?? "active");
    if (record) return;
    setLoading(true);
    void Promise.all([sisService.listLearners({ per_page: 100 }), sisService.listGuardians({ per_page: 100, status: "active" })]).then(([learnerResponse, guardianResponse]) => {
      if (learnerResponse.success && learnerResponse.data) setLearners(learnerResponse.data.learners);
      if (guardianResponse.success && guardianResponse.data) setGuardians(guardianResponse.data.guardians);
    }).finally(() => setLoading(false));
  }, [open, record]);

  const submit = async (event: React.FormEvent) => {
    event.preventDefault();
    if (!record && (!learnerId || !guardianId)) return toast.error("Choose a learner and guardian");
    setSaving(true);
    const change = { relationship_type: relationshipType, is_primary: isPrimary, can_collect: canCollect, receives_communications: receivesCommunications, status };
    const response = record ? await sisService.updateGuardianRelationship(record.id, change) : await sisService.createGuardianRelationship({ learner_id: learnerId, guardian_id: guardianId, ...change });
    setSaving(false);
    if (!response.success) return toast.error(responseMessage(response, "Guardian relationship could not be saved"));
    toast.success("Guardian relationship saved"); onSaved();
  };

  return <DialogShell onClose={onClose} open={open}><DialogHeader onClose={onClose} title={`${record ? "Edit" : "Add"} guardian relationship`} /><form onSubmit={submit}><DialogBody className="space-y-5">
    {record ? <div className="bg-[var(--surface-muted)] p-4"><p className="font-medium text-[var(--text-strong)]">{record.learner_name}</p><p className="mt-1 text-sm text-[var(--text-muted)]">{record.guardian_name}</p></div> : <><div><Label>Learner</Label><Select className="mt-1.5" data-autofocus="true" disabled={loading} onChange={(event) => setLearnerId(event.target.value)} required value={learnerId}><option value="">Choose a learner</option>{learners.map((learner) => <option key={learner.id} value={learner.id}>{learner.display_name} · {learner.learner_number}</option>)}</Select></div><div><Label>Guardian</Label><Select className="mt-1.5" disabled={loading} onChange={(event) => setGuardianId(event.target.value)} required value={guardianId}><option value="">Choose a guardian</option>{guardians.map((guardian) => <option key={guardian.id} value={guardian.id}>{guardian.display_name}</option>)}</Select></div></>}
    <div><Label>Relationship</Label><Select className="mt-1.5" onChange={(event) => setRelationshipType(event.target.value as RelationshipType)} value={relationshipType}><option value="mother">Mother</option><option value="father">Father</option><option value="parent">Parent</option><option value="guardian">Guardian</option><option value="carer">Carer</option><option value="sponsor">Sponsor</option><option value="other">Other</option></Select></div>
    <fieldset className="space-y-3"><legend className="text-sm font-medium text-[var(--text-strong)]">Responsibilities</legend><CheckField checked={isPrimary} label="Primary guardian" onChange={setIsPrimary} /><CheckField checked={canCollect} label="May collect learner" onChange={setCanCollect} /><CheckField checked={receivesCommunications} label="Receives communications" onChange={setReceivesCommunications} /></fieldset>
    <div><Label>Status</Label><Select className="mt-1.5" onChange={(event) => setStatus(event.target.value as DirectoryStatus)} value={status}><option value="active">Active</option><option value="inactive">Inactive</option></Select></div>
  </DialogBody><DialogFooter><Button disabled={saving} onClick={onClose} type="button" variant="ghost">Cancel</Button><Button disabled={saving || loading} type="submit">{saving ? <><Loader2 className="size-4 animate-spin" />Saving…</> : "Save relationship"}</Button></DialogFooter></form></DialogShell>;
}

function CheckField({ checked, label, onChange }: { checked: boolean; label: string; onChange: (value: boolean) => void }) { return <label className="flex min-h-10 cursor-pointer items-center gap-3 rounded-[var(--radius-md)] border border-[var(--border)] px-3 text-sm text-[var(--text-body)]"><input checked={checked} onChange={(event) => onChange(event.target.checked)} type="checkbox" />{label}</label>; }
function responsibilities(record: GuardianRelationship) { const values = [record.is_primary ? "Primary" : "", record.can_collect ? "Collection" : "", record.receives_communications ? "Communications" : ""].filter(Boolean); return values.join(" · ") || "None"; }

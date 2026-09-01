import { useCallback, useEffect, useState } from "react";
import { useNavigate } from "@tanstack/react-router";
import { HeartPulse, Plus, Search } from "lucide-react";
import toast from "react-hot-toast";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Table, TableControlsBar, TableControlsPagination, TableEmpty, TableError, TableLoading, TableScroll, TableWrap, TBody, TD, TH, THead, TR } from "@/components/ui/data-table";
import { DialogBody, DialogFooter, DialogHeader, DialogShell } from "@/components/ui/dialog";
import { Input, Label, Select } from "@/components/ui/input";
import { usePageChrome } from "@/modules/admin/layouts/page-chrome";
import { useAuthStore } from "@/stores/auth-store";

import { healthService, responseMessage } from "./service";
import type { HealthReferences, PatientSummary } from "./types";
import { displayValue, statusTone } from "./ui";

export function HealthPatientsWorkspace() {
  const navigate = useNavigate();
  const permissions = useAuthStore((state) => state.user?.permissions ?? []);
  const canCreate = permissions.includes("*") || permissions.includes("health:create");
  const [records, setRecords] = useState<PatientSummary[]>([]);
  const [references, setReferences] = useState<HealthReferences | null>(null);
  const [search, setSearch] = useState("");
  const [status, setStatus] = useState("all");
  const [page, setPage] = useState(1);
  const [totalPages, setTotalPages] = useState(1);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [drawerOpen, setDrawerOpen] = useState(false);

  const load = useCallback(async () => {
    setLoading(true); setError(null);
    try {
      const response = await healthService.patients({ page, per_page: 25, search: search.trim() || undefined, status: status === "all" ? undefined : status });
      if (!response.success || !response.data) throw new Error(responseMessage(response, "Health patients could not be loaded"));
      setRecords(response.data.patients); setTotalPages(response.pagination?.total_pages ?? 1);
    } catch (loadError) { setError(loadError instanceof Error ? loadError.message : "Health patients could not be loaded"); }
    finally { setLoading(false); }
  }, [page, search, status]);
  useEffect(() => { void load(); }, [load]);
  useEffect(() => {
    if (!canCreate) return;
    void healthService.references().then((response) => { if (response.success) setReferences(response.data ?? null); });
  }, [canCreate]);

  usePageChrome("Patients", canCreate ? <Button onClick={() => setDrawerOpen(true)}><Plus className="size-4" />Add patient</Button> : null);

  return <div className="space-y-6">
    <p className="text-sm text-[var(--text-muted)]">Learner and employee identity stays in SIS and HR.</p>
    <TableControlsBar>
      <Input aria-label="Search Health patients" className="sm:w-72" leadingIcon={<Search />} onChange={(event) => { setPage(1); setSearch(event.target.value); }} placeholder="Search name or number" value={search} />
      <Select aria-label="Patient status" className="sm:w-44" onChange={(event) => { setPage(1); setStatus(event.target.value); }} value={status}>
        <option value="all">All statuses</option><option value="active">Active</option><option value="inactive">Inactive</option>
      </Select>
      {!loading && records.length ? <TableControlsPagination onNext={() => setPage((value) => Math.min(totalPages, value + 1))} onPrevious={() => setPage((value) => Math.max(1, value - 1))} page={page} totalPages={totalPages} /> : null}
    </TableControlsBar>
    <TableWrap>
      {loading ? <TableLoading columns={6} label="Loading Health patients…" /> : error ? <TableError description={error} onRetry={() => void load()} /> : records.length === 0 ? <TableEmpty description={search || status !== "all" ? "Change the current filters." : canCreate ? "Add a learner or employee from the campus directory." : "No Health patient record is available."} icon={<HeartPulse />} title={search || status !== "all" ? "No patients match" : "No Health patients yet"} /> :
        <TableScroll><Table className="min-w-[860px]"><THead><tr><TH>Patient</TH><TH>Type</TH><TH>Status</TH><TH>Care alerts</TH><TH>Open visits</TH><TH>Follow-up</TH></tr></THead><TBody>
          {records.map((patient) => <TR className="cursor-pointer" key={patient.id} onClick={() => void navigate({ to: "/modules/health/patients/$patientId", params: { patientId: patient.id } })}>
            <TD><span className="font-medium text-[var(--text-strong)]">{patient.person_name}</span><p className="mt-1 text-xs text-[var(--text-muted)]">{patient.person_number}</p></TD>
            <TD className="text-[var(--text-muted)]">{displayValue(patient.person_kind)}</TD><TD><Badge tone={statusTone(patient.status)}>{displayValue(patient.status)}</Badge></TD>
            <TD className="font-tabular text-[var(--text-muted)]">{patient.active_care_item_count}</TD><TD className="font-tabular text-[var(--text-muted)]">{patient.open_visit_count}</TD><TD className="font-tabular text-[var(--text-muted)]">{patient.open_follow_up_count}</TD>
          </TR>)}
        </TBody></Table></TableScroll>}
    </TableWrap>
    <AddPatientDrawer open={drawerOpen} onClose={() => setDrawerOpen(false)} references={references} onSaved={() => { setDrawerOpen(false); void load(); void healthService.references().then((response) => { if (response.success) setReferences(response.data ?? null); }); }} />
  </div>;
}

function AddPatientDrawer({ open, onClose, references, onSaved }: { open: boolean; onClose: () => void; references: HealthReferences | null; onSaved: () => void }) {
  const [candidate, setCandidate] = useState(""); const [saving, setSaving] = useState(false);
  const candidates = references?.patients.filter((item) => !item.already_patient) ?? [];
  useEffect(() => { if (open) setCandidate(""); }, [open]);
  const save = async (event: React.FormEvent) => {
    event.preventDefault(); const selected = candidates.find((item) => `${item.kind}:${item.id}` === candidate); if (!selected) return;
    setSaving(true);
    try { const response = await healthService.createPatient(selected.kind, selected.id); if (!response.success) throw new Error(responseMessage(response, "Patient could not be added")); toast.success("Health patient added"); onSaved(); }
    catch (error) { toast.error(error instanceof Error ? error.message : "Patient could not be added"); } finally { setSaving(false); }
  };
  return <DialogShell onClose={onClose} open={open}><form onSubmit={(event) => void save(event)}><DialogHeader onClose={onClose} title="Add Health patient" /><DialogBody><div className="space-y-2"><Label htmlFor="health-patient">Learner or employee</Label><Select data-autofocus="true" id="health-patient" onChange={(event) => setCandidate(event.target.value)} required value={candidate}><option value="">Select a person</option>{candidates.map((item) => <option key={`${item.kind}:${item.id}`} value={`${item.kind}:${item.id}`}>{item.display_name} · {item.number} · {displayValue(item.kind)}</option>)}</Select>{candidates.length === 0 ? <p className="text-sm text-[var(--text-muted)]">Everyone in the current directory already has a Health patient record.</p> : null}</div></DialogBody><DialogFooter><Button onClick={onClose} type="button" variant="secondary">Cancel</Button><Button disabled={saving || !candidate} type="submit">{saving ? "Adding…" : "Add patient"}</Button></DialogFooter></form></DialogShell>;
}

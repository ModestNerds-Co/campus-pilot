import { useCallback, useEffect, useState } from "react";
import { CalendarCheck2, Plus, Search } from "lucide-react";
import toast from "react-hot-toast";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Table,
  TableControlsBar,
  TableControlsPagination,
  TableEmpty,
  TableError,
  TableLoading,
  TableScroll,
  TableWrap,
  TBody,
  TD,
  TH,
  THead,
  TR,
} from "@/components/ui/data-table";
import {
  DialogBody,
  DialogFooter,
  DialogHeader,
  DialogShell,
} from "@/components/ui/dialog";
import { Input, Label, Select, Textarea } from "@/components/ui/input";
import { usePageChrome } from "@/modules/admin/layouts/page-chrome";
import { useAuthStore } from "@/stores/auth-store";

import { healthAccessProfile } from "./access";
import { healthService, responseMessage } from "./service";
import type {
  FollowUp,
  FollowUpStatus,
  HealthReferences,
} from "./types";
import { displayValue, statusTone } from "./ui";

export function HealthFollowUpsWorkspace() {
  const user = useAuthStore((state) => state.user);
  const access = healthAccessProfile(user?.permissions ?? [], user?.record_scopes);
  const canManage = access.canManageFollowUps;
  const [followUps, setFollowUps] = useState<FollowUp[]>([]);
  const [references, setReferences] = useState<HealthReferences | null>(null);
  const [search, setSearch] = useState("");
  const [status, setStatus] = useState("all");
  const [page, setPage] = useState(1);
  const [totalPages, setTotalPages] = useState(1);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [drawerOpen, setDrawerOpen] = useState(false);
  const [selected, setSelected] = useState<FollowUp | null>(null);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const response = await healthService.followUps({
        page,
        per_page: 25,
        search: search.trim() || undefined,
        status: status === "all" ? undefined : status,
      });
      if (!response.success || !response.data)
        throw new Error(responseMessage(response, "Health follow-ups could not be loaded"));
      setFollowUps(response.data.follow_ups);
      setTotalPages(response.pagination?.total_pages ?? 1);
    } catch (loadError) {
      setError(loadError instanceof Error ? loadError.message : "Health follow-ups could not be loaded");
    } finally {
      setLoading(false);
    }
  }, [page, search, status]);
  useEffect(() => void load(), [load]);
  useEffect(() => {
    if (!canManage) return;
    void healthService.references().then((response) => {
      if (response.success) setReferences(response.data ?? null);
    });
  }, [canManage]);
  usePageChrome(
    "Follow-ups",
    canManage ? (
      <Button onClick={() => { setSelected(null); setDrawerOpen(true); }}>
        <Plus className="size-4" />
        Add follow-up
      </Button>
    ) : null,
  );

  return (
    <div className="space-y-6">
      <p className="text-sm text-[var(--text-muted)]">Track required actions and assigned staff.</p>
      <TableControlsBar>
        <Input aria-label="Search Health follow-ups" className="sm:w-72" leadingIcon={<Search />} onChange={(event) => { setPage(1); setSearch(event.target.value); }} placeholder="Search patient or purpose" value={search} />
        <Select aria-label="Follow-up status" className="sm:w-44" onChange={(event) => { setPage(1); setStatus(event.target.value); }} value={status}><option value="all">All statuses</option><option value="open">Open</option><option value="completed">Completed</option><option value="cancelled">Cancelled</option></Select>
        {!loading && followUps.length ? <TableControlsPagination onNext={() => setPage((value) => Math.min(totalPages, value + 1))} onPrevious={() => setPage((value) => Math.max(1, value - 1))} page={page} totalPages={totalPages} /> : null}
      </TableControlsBar>
      <TableWrap>
        {loading ? <TableLoading columns={6} label="Loading Health follow-ups…" /> : error ? <TableError description={error} onRetry={() => void load()} /> : followUps.length === 0 ? <TableEmpty description={search || status !== "all" ? "Change the current filters." : "No Health follow-ups are in this scope."} icon={<CalendarCheck2 />} title={search || status !== "all" ? "No follow-ups match" : "No follow-ups yet"} /> : <TableScroll><Table className="min-w-[860px]"><THead><tr><TH>Patient</TH><TH>Due</TH><TH>Purpose</TH><TH>Assigned to</TH><TH>Status</TH><TH>Outcome</TH></tr></THead><TBody>{followUps.map((followUp) => { const editable = canManage && followUp.status === "open"; return <TR className={editable ? "cursor-pointer" : undefined} key={followUp.id} onClick={() => { if (editable) { setSelected(followUp); setDrawerOpen(true); } }}><TD><span className="font-medium text-[var(--text-strong)]">{followUp.patient_name}</span><p className="mt-1 text-xs text-[var(--text-muted)]">{followUp.patient_number}</p></TD><TD className="whitespace-nowrap text-[var(--text-muted)]">{followUp.due_on}</TD><TD className="max-w-80 text-[var(--text-body)]">{followUp.purpose}</TD><TD className="text-[var(--text-muted)]">{followUp.assigned_employee_name || "Unassigned"}</TD><TD><Badge tone={statusTone(followUp.status)}>{displayValue(followUp.status)}</Badge></TD><TD className="max-w-64 truncate text-[var(--text-muted)]">{followUp.outcome || "—"}</TD></TR>; })}</TBody></Table></TableScroll>}
      </TableWrap>
      <FollowUpDrawer followUp={selected} onClose={() => setDrawerOpen(false)} onSaved={() => { setDrawerOpen(false); void load(); }} open={drawerOpen} references={references} />
    </div>
  );
}

function FollowUpDrawer({ open, onClose, onSaved, followUp, references }: { open: boolean; onClose: () => void; onSaved: () => void; followUp: FollowUp | null; references: HealthReferences | null }) {
  const [patientId, setPatientId] = useState("");
  const [employeeId, setEmployeeId] = useState("");
  const [dueOn, setDueOn] = useState("");
  const [purpose, setPurpose] = useState("");
  const [status, setStatus] = useState<FollowUpStatus>("open");
  const [outcome, setOutcome] = useState("");
  const [saving, setSaving] = useState(false);
  useEffect(() => {
    if (!open) return;
    setPatientId(followUp?.patient_id ?? "");
    setEmployeeId(followUp?.assigned_employee_id ?? "");
    setDueOn(followUp?.due_on ?? new Date().toISOString().slice(0, 10));
    setPurpose(followUp?.purpose ?? "");
    setStatus(followUp?.status ?? "open");
    setOutcome(followUp?.outcome ?? "");
  }, [followUp, open]);
  const save = async (event: React.FormEvent) => {
    event.preventDefault(); setSaving(true);
    try {
      const response = followUp
        ? await healthService.updateFollowUp(followUp, { assigned_employee_id: employeeId || null, due_on: dueOn, purpose, status, outcome: outcome.trim() || null })
        : await healthService.createFollowUp({ patient_id: patientId, visit_id: null, assigned_employee_id: employeeId || null, due_on: dueOn, purpose });
      if (!response.success) throw new Error(responseMessage(response, "Follow-up could not be saved"));
      toast.success("Follow-up saved"); onSaved();
    } catch (error) { toast.error(error instanceof Error ? error.message : "Follow-up could not be saved"); } finally { setSaving(false); }
  };
  const patients = references?.patients.filter((patient) => patient.already_patient) ?? [];
  return <DialogShell onClose={onClose} open={open}><form onSubmit={(event) => void save(event)}><DialogHeader onClose={onClose} title={followUp ? "Update follow-up" : "Add follow-up"} /><DialogBody><div className="space-y-5">
    <Field label="Patient">{followUp ? <Input disabled value={`${followUp.patient_name} · ${followUp.patient_number}`} /> : <Select data-autofocus="true" onChange={(event) => setPatientId(event.target.value)} required value={patientId}><option value="">Select a patient</option>{patients.map((patient) => <option key={patient.id} value={patient.id}>{patient.display_name} · {patient.number}</option>)}</Select>}</Field>
    <Field label="Assigned employee"><Select onChange={(event) => setEmployeeId(event.target.value)} value={employeeId}><option value="">Unassigned</option>{references?.employees.map((employee) => <option key={employee.id} value={employee.id}>{employee.display_name} · {employee.number}</option>)}</Select></Field><Field label="Due on"><Input onChange={(event) => setDueOn(event.target.value)} required type="date" value={dueOn} /></Field><Field label="Purpose"><Textarea maxLength={1000} onChange={(event) => setPurpose(event.target.value)} required rows={5} value={purpose} /></Field>{followUp ? <><Field label="Status"><Select onChange={(event) => setStatus(event.target.value as FollowUpStatus)} value={status}><option value="open">Open</option><option value="completed">Completed</option><option value="cancelled">Cancelled</option></Select></Field><Field label="Outcome"><Textarea maxLength={2000} onChange={(event) => setOutcome(event.target.value)} required={status !== "open"} rows={5} value={outcome} /></Field></> : null}
  </div></DialogBody><DialogFooter><Button onClick={onClose} type="button" variant="secondary">Cancel</Button><Button disabled={saving || !patientId || (status !== "open" && !outcome.trim())} type="submit">{saving ? "Saving…" : "Save"}</Button></DialogFooter></form></DialogShell>;
}

function Field({ label, children }: { label: string; children: React.ReactNode }) { return <div className="space-y-2"><Label>{label}</Label>{children}</div>; }

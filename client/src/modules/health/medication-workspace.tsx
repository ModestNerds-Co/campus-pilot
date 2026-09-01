import { useCallback, useEffect, useState } from "react";
import { Pill, Plus, Search } from "lucide-react";
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

import {
  healthService,
  responseMessage,
  type MedicationPlanPayload,
} from "./service";
import type {
  HealthReferences,
  MedicationAdministration,
  MedicationPlan,
  MedicationPlanStatus,
} from "./types";
import { dateTime, displayValue, statusTone } from "./ui";

export function HealthMedicationWorkspace() {
  const permissions = useAuthStore((state) => state.user?.permissions ?? []);
  const canManage = allowed(permissions, "health:medication");
  const [plans, setPlans] = useState<MedicationPlan[]>([]);
  const [administrations, setAdministrations] = useState<MedicationAdministration[]>([]);
  const [references, setReferences] = useState<HealthReferences | null>(null);
  const [search, setSearch] = useState("");
  const [status, setStatus] = useState("all");
  const [page, setPage] = useState(1);
  const [totalPages, setTotalPages] = useState(1);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [planOpen, setPlanOpen] = useState(false);
  const [selected, setSelected] = useState<MedicationPlan | null>(null);
  const [administer, setAdminister] = useState<MedicationPlan | null>(null);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const [planResponse, historyResponse] = await Promise.all([
        healthService.medicationPlans({
          page,
          per_page: 25,
          search: search.trim() || undefined,
          status: status === "all" ? undefined : status,
        }),
        healthService.administrations({ page: 1, per_page: 25 }),
      ]);
      if (!planResponse.success || !planResponse.data)
        throw new Error(responseMessage(planResponse, "Medication plans could not be loaded"));
      if (!historyResponse.success || !historyResponse.data)
        throw new Error(responseMessage(historyResponse, "Medication history could not be loaded"));
      setPlans(planResponse.data.medication_plans);
      setAdministrations(historyResponse.data.administrations);
      setTotalPages(planResponse.pagination?.total_pages ?? 1);
    } catch (loadError) {
      setError(loadError instanceof Error ? loadError.message : "Medication records could not be loaded");
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
    "Medication",
    canManage ? (
      <Button onClick={() => { setSelected(null); setPlanOpen(true); }}>
        <Plus className="size-4" />
        Add plan
      </Button>
    ) : null,
  );

  return (
    <div className="space-y-8">
      <section className="space-y-5">
        <div><h2 className="text-lg font-semibold text-[var(--text-strong)]">Medication plans</h2><p className="mt-1 text-sm text-[var(--text-muted)]">Authorization and administration instructions.</p></div>
        <TableControlsBar>
          <Input aria-label="Search medication plans" className="sm:w-72" leadingIcon={<Search />} onChange={(event) => { setPage(1); setSearch(event.target.value); }} placeholder="Search patient or medication" value={search} />
          <Select aria-label="Medication plan status" className="sm:w-44" onChange={(event) => { setPage(1); setStatus(event.target.value); }} value={status}><option value="all">All statuses</option><option value="active">Active</option><option value="suspended">Suspended</option><option value="ended">Ended</option></Select>
          {!loading && plans.length ? <TableControlsPagination onNext={() => setPage((value) => Math.min(totalPages, value + 1))} onPrevious={() => setPage((value) => Math.max(1, value - 1))} page={page} totalPages={totalPages} /> : null}
        </TableControlsBar>
        <TableWrap>
          {loading ? <TableLoading columns={6} label="Loading medication plans…" /> : error ? <TableError description={error} onRetry={() => void load()} /> : plans.length === 0 ? <TableEmpty description={search || status !== "all" ? "Change the current filters." : "No medication plans are in this scope."} icon={<Pill />} title={search || status !== "all" ? "No plans match" : "No medication plans yet"} /> : <TableScroll><Table className="min-w-[900px]"><THead><tr><TH>Patient</TH><TH>Medication</TH><TH>Dose and route</TH><TH>Schedule</TH><TH>Dates</TH><TH>Status</TH><TH /></tr></THead><TBody>{plans.map((plan) => <TR className={canManage ? "cursor-pointer" : undefined} key={plan.id} onClick={() => { if (canManage) { setSelected(plan); setPlanOpen(true); } }}><TD><span className="font-medium text-[var(--text-strong)]">{plan.patient_name}</span><p className="mt-1 text-xs text-[var(--text-muted)]">{plan.patient_number}</p></TD><TD className="font-medium text-[var(--text-strong)]">{plan.medication_name}</TD><TD className="text-[var(--text-muted)]">{plan.dosage} · {plan.route}</TD><TD className="text-[var(--text-muted)]">{plan.schedule}</TD><TD className="whitespace-nowrap text-[var(--text-muted)]">{plan.starts_on}{plan.ends_on ? ` – ${plan.ends_on}` : ""}</TD><TD><Badge tone={statusTone(plan.status)}>{displayValue(plan.status)}</Badge></TD><TD>{canManage && plan.status === "active" ? <Button onClick={(event) => { event.stopPropagation(); setAdminister(plan); }} size="sm" type="button" variant="secondary">Record dose</Button> : null}</TD></TR>)}</TBody></Table></TableScroll>}
        </TableWrap>
      </section>
      <section className="space-y-4"><div><h2 className="text-lg font-semibold text-[var(--text-strong)]">Administration history</h2><p className="mt-1 text-sm text-[var(--text-muted)]">Recorded entries cannot be edited or deleted.</p></div><TableWrap>{loading ? <TableLoading columns={5} label="Loading administration history…" /> : administrations.length === 0 ? <TableEmpty description="No medication administration has been recorded." icon={<Pill />} title="No administration history" /> : <TableScroll><Table className="min-w-[760px]"><THead><tr><TH>Patient</TH><TH>Medication</TH><TH>Administered</TH><TH>Dose</TH><TH>Outcome</TH></tr></THead><TBody>{administrations.map((entry) => <TR key={entry.id}><TD><span className="font-medium text-[var(--text-strong)]">{entry.patient_name}</span><p className="mt-1 text-xs text-[var(--text-muted)]">{entry.patient_number}</p></TD><TD>{entry.medication_name}</TD><TD className="whitespace-nowrap text-[var(--text-muted)]">{dateTime(entry.administered_at)}</TD><TD className="text-[var(--text-muted)]">{entry.dose}</TD><TD><Badge tone={statusTone(entry.outcome)}>{displayValue(entry.outcome)}</Badge></TD></TR>)}</TBody></Table></TableScroll>}</TableWrap></section>
      <MedicationPlanDrawer onClose={() => setPlanOpen(false)} onSaved={() => { setPlanOpen(false); void load(); }} open={planOpen} plan={selected} references={references} />
      <AdministrationDrawer onClose={() => setAdminister(null)} onSaved={() => { setAdminister(null); void load(); }} plan={administer} />
    </div>
  );
}

function MedicationPlanDrawer({ open, onClose, onSaved, plan, references }: { open: boolean; onClose: () => void; onSaved: () => void; plan: MedicationPlan | null; references: HealthReferences | null }) {
  const [patientId, setPatientId] = useState("");
  const [name, setName] = useState("");
  const [dosage, setDosage] = useState("");
  const [route, setRoute] = useState("");
  const [schedule, setSchedule] = useState("");
  const [instructions, setInstructions] = useState("");
  const [authorization, setAuthorization] = useState("");
  const [startsOn, setStartsOn] = useState("");
  const [endsOn, setEndsOn] = useState("");
  const [status, setStatus] = useState<MedicationPlanStatus>("active");
  const [saving, setSaving] = useState(false);
  useEffect(() => {
    if (!open) return;
    setPatientId(plan?.patient_id ?? ""); setName(plan?.medication_name ?? ""); setDosage(plan?.dosage ?? ""); setRoute(plan?.route ?? ""); setSchedule(plan?.schedule ?? ""); setInstructions(plan?.instructions ?? ""); setAuthorization(plan?.authorization_reference ?? ""); setStartsOn(plan?.starts_on ?? new Date().toISOString().slice(0, 10)); setEndsOn(plan?.ends_on ?? ""); setStatus(plan?.status ?? "active");
  }, [open, plan]);
  const save = async (event: React.FormEvent) => {
    event.preventDefault(); setSaving(true);
    const payload: MedicationPlanPayload = { patient_id: patientId, medication_name: name, dosage, route, schedule, instructions: instructions.trim() || null, authorization_reference: authorization, starts_on: startsOn, ends_on: endsOn || null };
    try {
      const response = plan ? await healthService.updateMedicationPlan(plan, { ...payload, status }) : await healthService.createMedicationPlan(payload);
      if (!response.success) throw new Error(responseMessage(response, "Medication plan could not be saved"));
      toast.success("Medication plan saved"); onSaved();
    } catch (error) { toast.error(error instanceof Error ? error.message : "Medication plan could not be saved"); } finally { setSaving(false); }
  };
  const patients = references?.patients.filter((patient) => patient.already_patient) ?? [];
  return <DialogShell onClose={onClose} open={open}><form onSubmit={(event) => void save(event)}><DialogHeader onClose={onClose} title={plan ? "Edit medication plan" : "Add medication plan"} /><DialogBody><div className="space-y-5">
    <Field label="Patient">{plan ? <Input disabled value={`${plan.patient_name} · ${plan.patient_number}`} /> : <Select data-autofocus="true" onChange={(event) => setPatientId(event.target.value)} required value={patientId}><option value="">Select a patient</option>{patients.map((patient) => <option key={patient.id} value={patient.id}>{patient.display_name} · {patient.number}</option>)}</Select>}</Field>
    <Field label="Medication"><Input maxLength={200} onChange={(event) => setName(event.target.value)} required value={name} /></Field><Field label="Dose"><Input maxLength={160} onChange={(event) => setDosage(event.target.value)} required value={dosage} /></Field><Field label="Route"><Input maxLength={80} onChange={(event) => setRoute(event.target.value)} placeholder="Oral, topical, inhaled…" required value={route} /></Field><Field label="Schedule"><Input maxLength={300} onChange={(event) => setSchedule(event.target.value)} required value={schedule} /></Field><Field label="Instructions"><Textarea maxLength={2000} onChange={(event) => setInstructions(event.target.value)} rows={4} value={instructions} /></Field><Field label="Authorization reference"><Input maxLength={300} onChange={(event) => setAuthorization(event.target.value)} required value={authorization} /></Field><div className="grid grid-cols-2 gap-4"><Field label="Starts on"><Input onChange={(event) => setStartsOn(event.target.value)} required type="date" value={startsOn} /></Field><Field label="Ends on"><Input onChange={(event) => setEndsOn(event.target.value)} type="date" value={endsOn} /></Field></div>{plan ? <Field label="Status"><Select onChange={(event) => setStatus(event.target.value as MedicationPlanStatus)} value={status}><option value="active">Active</option><option value="suspended">Suspended</option><option value="ended">Ended</option></Select></Field> : null}
  </div></DialogBody><DialogFooter><Button onClick={onClose} type="button" variant="secondary">Cancel</Button><Button disabled={saving || !patientId} type="submit">{saving ? "Saving…" : "Save"}</Button></DialogFooter></form></DialogShell>;
}

function AdministrationDrawer({ plan, onClose, onSaved }: { plan: MedicationPlan | null; onClose: () => void; onSaved: () => void }) {
  const [administeredAt, setAdministeredAt] = useState("");
  const [dose, setDose] = useState("");
  const [outcome, setOutcome] = useState<"given" | "refused" | "missed" | "held">("given");
  const [note, setNote] = useState("");
  const [saving, setSaving] = useState(false);
  useEffect(() => { if (plan) { setAdministeredAt(localDateTime()); setDose(plan.dosage); setOutcome("given"); setNote(""); } }, [plan]);
  if (!plan) return null;
  const save = async (event: React.FormEvent) => { event.preventDefault(); setSaving(true); try { const response = await healthService.recordAdministration(plan.id, { administered_at: new Date(administeredAt).toISOString(), dose, outcome, note: note.trim() || null }); if (!response.success) throw new Error(responseMessage(response, "Medication administration could not be recorded")); toast.success("Medication administration recorded"); onSaved(); } catch (error) { toast.error(error instanceof Error ? error.message : "Medication administration could not be recorded"); } finally { setSaving(false); } };
  return <DialogShell onClose={onClose} open><form onSubmit={(event) => void save(event)}><DialogHeader onClose={onClose} title="Record medication" /><DialogBody><div className="space-y-5"><div className="rounded-[var(--radius-lg)] bg-[var(--surface-muted)] p-4"><p className="font-medium text-[var(--text-strong)]">{plan.patient_name}</p><p className="mt-1 text-sm text-[var(--text-muted)]">{plan.medication_name} · {plan.dosage}</p></div><Field label="Administered at"><Input data-autofocus="true" onChange={(event) => setAdministeredAt(event.target.value)} required type="datetime-local" value={administeredAt} /></Field><Field label="Dose"><Input maxLength={160} onChange={(event) => setDose(event.target.value)} required value={dose} /></Field><Field label="Outcome"><Select onChange={(event) => setOutcome(event.target.value as typeof outcome)} value={outcome}><option value="given">Given</option><option value="refused">Refused</option><option value="missed">Missed</option><option value="held">Held</option></Select></Field><Field label="Note"><Textarea maxLength={2000} onChange={(event) => setNote(event.target.value)} rows={4} value={note} /></Field></div></DialogBody><DialogFooter><Button onClick={onClose} type="button" variant="secondary">Cancel</Button><Button disabled={saving} type="submit">{saving ? "Recording…" : "Record"}</Button></DialogFooter></form></DialogShell>;
}

function Field({ label, children }: { label: string; children: React.ReactNode }) { return <div className="space-y-2"><Label>{label}</Label>{children}</div>; }
function allowed(permissions: string[], permission: string) { return permissions.includes("*") || permissions.includes(permission); }
function localDateTime() { const now = new Date(); return new Date(now.getTime() - now.getTimezoneOffset() * 60_000).toISOString().slice(0, 16); }

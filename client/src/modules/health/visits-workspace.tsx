import { useCallback, useEffect, useState } from "react";
import { ClipboardPlus, Plus, Search } from "lucide-react";
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

import { healthService, responseMessage } from "./service";
import type {
  HealthReferences,
  Visit,
  VisitCategory,
  VisitDisposition,
} from "./types";
import { dateTime, displayValue, statusTone } from "./ui";

export function HealthVisitsWorkspace() {
  const permissions = useAuthStore((state) => state.user?.permissions ?? []);
  const canCreate = allowed(permissions, "health:create");
  const canEdit = allowed(permissions, "health:edit");
  const [visits, setVisits] = useState<Visit[]>([]);
  const [references, setReferences] = useState<HealthReferences | null>(null);
  const [search, setSearch] = useState("");
  const [status, setStatus] = useState("all");
  const [page, setPage] = useState(1);
  const [totalPages, setTotalPages] = useState(1);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [createOpen, setCreateOpen] = useState(false);
  const [selected, setSelected] = useState<Visit | null>(null);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const response = await healthService.visits({
        page,
        per_page: 25,
        search: search.trim() || undefined,
        status: status === "all" ? undefined : status,
      });
      if (!response.success || !response.data)
        throw new Error(responseMessage(response, "Clinic visits could not be loaded"));
      setVisits(response.data.visits);
      setTotalPages(response.pagination?.total_pages ?? 1);
    } catch (loadError) {
      setError(loadError instanceof Error ? loadError.message : "Clinic visits could not be loaded");
    } finally {
      setLoading(false);
    }
  }, [page, search, status]);

  useEffect(() => void load(), [load]);
  useEffect(() => {
    if (!canCreate) return;
    void healthService.references().then((response) => {
      if (response.success) setReferences(response.data ?? null);
    });
  }, [canCreate]);
  usePageChrome(
    "Clinic visits",
    canCreate ? (
      <Button onClick={() => setCreateOpen(true)}>
        <Plus className="size-4" />
        Check in patient
      </Button>
    ) : null,
  );

  return (
    <div className="space-y-6">
      <p className="text-sm text-[var(--text-muted)]">Record clinic attendance, care given, and disposition.</p>
      <TableControlsBar>
        <Input
          aria-label="Search clinic visits"
          className="sm:w-72"
          leadingIcon={<Search />}
          onChange={(event) => {
            setPage(1);
            setSearch(event.target.value);
          }}
          placeholder="Search patient or concern"
          value={search}
        />
        <Select
          aria-label="Visit status"
          className="sm:w-44"
          onChange={(event) => {
            setPage(1);
            setStatus(event.target.value);
          }}
          value={status}
        >
          <option value="all">All statuses</option>
          <option value="open">Open</option>
          <option value="closed">Closed</option>
        </Select>
        {!loading && visits.length ? (
          <TableControlsPagination
            onNext={() => setPage((value) => Math.min(totalPages, value + 1))}
            onPrevious={() => setPage((value) => Math.max(1, value - 1))}
            page={page}
            totalPages={totalPages}
          />
        ) : null}
      </TableControlsBar>
      <TableWrap>
        {loading ? (
          <TableLoading columns={6} label="Loading clinic visits…" />
        ) : error ? (
          <TableError description={error} onRetry={() => void load()} />
        ) : visits.length === 0 ? (
          <TableEmpty
            description={search || status !== "all" ? "Change the current filters." : "No clinic visits are in this scope."}
            icon={<ClipboardPlus />}
            title={search || status !== "all" ? "No visits match" : "No clinic visits yet"}
          />
        ) : (
          <TableScroll>
            <Table className="min-w-[900px]">
              <THead><tr><TH>Patient</TH><TH>Checked in</TH><TH>Category</TH><TH>Concern</TH><TH>Disposition</TH><TH>Status</TH></tr></THead>
              <TBody>
                {visits.map((visit) => (
                  <TR className="cursor-pointer" key={visit.id} onClick={() => setSelected(visit)}>
                    <TD><span className="font-medium text-[var(--text-strong)]">{visit.patient_name}</span><p className="mt-1 text-xs text-[var(--text-muted)]">{visit.patient_number}</p></TD>
                    <TD className="whitespace-nowrap text-[var(--text-muted)]">{dateTime(visit.checked_in_at)}</TD>
                    <TD className="text-[var(--text-muted)]">{displayValue(visit.category)}</TD>
                    <TD className="max-w-80 truncate text-[var(--text-body)]">{visit.presenting_concern}</TD>
                    <TD className="text-[var(--text-muted)]">{displayValue(visit.disposition)}</TD>
                    <TD><Badge tone={statusTone(visit.status)}>{displayValue(visit.status)}</Badge></TD>
                  </TR>
                ))}
              </TBody>
            </Table>
          </TableScroll>
        )}
      </TableWrap>
      <CreateVisitDrawer
        onClose={() => setCreateOpen(false)}
        onSaved={() => {
          setCreateOpen(false);
          void load();
        }}
        open={createOpen}
        references={references}
      />
      <VisitDrawer
        canEdit={canEdit}
        onClose={() => setSelected(null)}
        onSaved={(visit) => {
          setSelected(visit);
          void load();
        }}
        visit={selected}
      />
    </div>
  );
}

function CreateVisitDrawer({ open, onClose, onSaved, references }: { open: boolean; onClose: () => void; onSaved: () => void; references: HealthReferences | null }) {
  const [patientId, setPatientId] = useState("");
  const [checkedInAt, setCheckedInAt] = useState("");
  const [category, setCategory] = useState<VisitCategory>("illness");
  const [concern, setConcern] = useState("");
  const [assessment, setAssessment] = useState("");
  const [careGiven, setCareGiven] = useState("");
  const [saving, setSaving] = useState(false);
  useEffect(() => {
    if (!open) return;
    setPatientId("");
    setCheckedInAt(localDateTime());
    setCategory("illness");
    setConcern("");
    setAssessment("");
    setCareGiven("");
  }, [open]);
  const save = async (event: React.FormEvent) => {
    event.preventDefault();
    setSaving(true);
    try {
      const response = await healthService.createVisit({
        patient_id: patientId,
        checked_in_at: new Date(checkedInAt).toISOString(),
        category,
        presenting_concern: concern,
        assessment: assessment.trim() || null,
        care_given: careGiven.trim() || null,
      });
      if (!response.success) throw new Error(responseMessage(response, "Clinic visit could not be created"));
      toast.success("Patient checked in");
      onSaved();
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "Clinic visit could not be created");
    } finally {
      setSaving(false);
    }
  };
  const patients = references?.patients.filter((patient) => patient.already_patient) ?? [];
  return (
    <DialogShell onClose={onClose} open={open}>
      <form onSubmit={(event) => void save(event)}>
        <DialogHeader onClose={onClose} title="Check in patient" />
        <DialogBody><div className="space-y-5">
          <Field label="Patient"><Select data-autofocus="true" onChange={(event) => setPatientId(event.target.value)} required value={patientId}><option value="">Select a patient</option>{patients.map((patient) => <option key={patient.id} value={patient.id}>{patient.display_name} · {patient.number}</option>)}</Select></Field>
          <Field label="Checked in at"><Input onChange={(event) => setCheckedInAt(event.target.value)} required type="datetime-local" value={checkedInAt} /></Field>
          <Field label="Category"><Select onChange={(event) => setCategory(event.target.value as VisitCategory)} value={category}>{["illness", "injury", "medication", "wellbeing", "follow_up", "other"].map((value) => <option key={value} value={value}>{displayValue(value)}</option>)}</Select></Field>
          <Field label="Presenting concern"><Textarea maxLength={2000} onChange={(event) => setConcern(event.target.value)} required rows={4} value={concern} /></Field>
          <Field label="Assessment"><Textarea maxLength={4000} onChange={(event) => setAssessment(event.target.value)} rows={4} value={assessment} /></Field>
          <Field label="Care given"><Textarea maxLength={4000} onChange={(event) => setCareGiven(event.target.value)} rows={4} value={careGiven} /></Field>
        </div></DialogBody>
        <DialogFooter><Button onClick={onClose} type="button" variant="secondary">Cancel</Button><Button disabled={saving || !patientId} type="submit">{saving ? "Checking in…" : "Check in"}</Button></DialogFooter>
      </form>
    </DialogShell>
  );
}

function VisitDrawer({ visit, canEdit, onClose, onSaved }: { visit: Visit | null; canEdit: boolean; onClose: () => void; onSaved: (visit: Visit) => void }) {
  const [disposition, setDisposition] = useState<VisitDisposition>("returned_to_class");
  const [assessment, setAssessment] = useState("");
  const [careGiven, setCareGiven] = useState("");
  const [saving, setSaving] = useState(false);
  useEffect(() => {
    setDisposition(visit?.disposition ?? "returned_to_class");
    setAssessment(visit?.assessment ?? "");
    setCareGiven(visit?.care_given ?? "");
  }, [visit]);
  if (!visit) return null;
  const closeVisit = async () => {
    setSaving(true);
    try {
      const response = await healthService.closeVisit(visit, disposition, assessment.trim() || null, careGiven.trim() || null);
      if (!response.success || !response.data) throw new Error(responseMessage(response, "Clinic visit could not be closed"));
      toast.success("Clinic visit closed");
      onSaved(response.data);
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "Clinic visit could not be closed");
    } finally {
      setSaving(false);
    }
  };
  return (
    <DialogShell onClose={onClose} open>
      <DialogHeader onClose={saving ? undefined : onClose} title="Clinic visit" />
      <DialogBody><div className="space-y-6">
        <div><div className="flex items-start justify-between gap-4"><div><p className="font-semibold text-[var(--text-strong)]">{visit.patient_name}</p><p className="mt-1 text-sm text-[var(--text-muted)]">{visit.patient_number} · {dateTime(visit.checked_in_at)}</p></div><Badge tone={statusTone(visit.status)}>{displayValue(visit.status)}</Badge></div><p className="mt-5 text-sm font-medium text-[var(--text-strong)]">{displayValue(visit.category)}</p><p className="mt-2 whitespace-pre-wrap text-sm leading-6 text-[var(--text-body)]">{visit.presenting_concern}</p></div>
        {visit.status === "closed" ? <div className="space-y-4 border-t border-[var(--border)] pt-5"><ReadValue label="Disposition" value={displayValue(visit.disposition)} /><ReadValue label="Assessment" value={visit.assessment || "—"} /><ReadValue label="Care given" value={visit.care_given || "—"} /></div> : canEdit ? <div className="space-y-5 border-t border-[var(--border)] pt-5"><Field label="Disposition"><Select onChange={(event) => setDisposition(event.target.value as VisitDisposition)} value={disposition}>{["returned_to_class", "sent_home", "emergency_referral", "guardian_collection", "staff_released", "other"].map((value) => <option key={value} value={value}>{displayValue(value)}</option>)}</Select></Field><Field label="Assessment"><Textarea maxLength={4000} onChange={(event) => setAssessment(event.target.value)} rows={4} value={assessment} /></Field><Field label="Care given"><Textarea maxLength={4000} onChange={(event) => setCareGiven(event.target.value)} rows={4} value={careGiven} /></Field></div> : null}
      </div></DialogBody>
      <DialogFooter><Button onClick={onClose} type="button" variant="secondary">Close</Button>{visit.status === "open" && canEdit ? <Button disabled={saving} onClick={() => void closeVisit()} type="button">{saving ? "Closing…" : "Close visit"}</Button> : null}</DialogFooter>
    </DialogShell>
  );
}

function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return <div className="space-y-2"><Label>{label}</Label>{children}</div>;
}
function ReadValue({ label, value }: { label: string; value: string }) {
  return <div><p className="text-xs font-semibold uppercase tracking-[0.12em] text-[var(--text-subtle)]">{label}</p><p className="mt-1 whitespace-pre-wrap text-sm leading-6 text-[var(--text-body)]">{value}</p></div>;
}
function allowed(permissions: string[], permission: string) {
  return permissions.includes("*") || permissions.includes(permission);
}
function localDateTime() {
  const now = new Date();
  return new Date(now.getTime() - now.getTimezoneOffset() * 60_000).toISOString().slice(0, 16);
}

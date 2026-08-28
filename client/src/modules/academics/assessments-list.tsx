// Term-scoped assessment cycles and weighted teaching-assignment components.

import { useCallback, useEffect, useMemo, useState } from "react";
import {
  CalendarCheck2,
  CheckCircle2,
  Edit,
  FileCheck2,
  Loader2,
  Plus,
  Trash2,
} from "lucide-react";
import toast from "react-hot-toast";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { ConfirmDrawer } from "@/components/ui/confirm-drawer";
import {
  Table,
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
import { DialogBody, DialogFooter, DialogHeader, DialogShell } from "@/components/ui/dialog";
import { Input, Label, Select } from "@/components/ui/input";
import { usePageChrome } from "@/modules/admin/layouts/page-chrome";

import { academicsService, responseMessage } from "./service";
import type {
  AcademicTerm,
  AssessmentComponent,
  AssessmentComponentInput,
  AssessmentCycle,
  AssessmentCycleInput,
  AssessmentCycleStatus,
  AssessmentKind,
  DirectoryStatus,
  TeachingAssignment,
} from "./types";

type Transition = "open" | "closed";

export function AssessmentsList() {
  const [cycles, setCycles] = useState<AssessmentCycle[]>([]);
  const [terms, setTerms] = useState<AcademicTerm[]>([]);
  const [components, setComponents] = useState<AssessmentComponent[]>([]);
  const [assignments, setAssignments] = useState<TeachingAssignment[]>([]);
  const [selectedCycleId, setSelectedCycleId] = useState<string | null>(null);
  const [cyclesLoading, setCyclesLoading] = useState(true);
  const [componentsLoading, setComponentsLoading] = useState(false);
  const [cyclesError, setCyclesError] = useState<string | null>(null);
  const [componentsError, setComponentsError] = useState<string | null>(null);
  const [cycleDrawer, setCycleDrawer] = useState<AssessmentCycle | null | undefined>(undefined);
  const [componentDrawer, setComponentDrawer] = useState<AssessmentComponent | null | undefined>(undefined);
  const [deleteCycle, setDeleteCycle] = useState<AssessmentCycle | null>(null);
  const [deleteComponent, setDeleteComponent] = useState<AssessmentComponent | null>(null);
  const [transition, setTransition] = useState<{ cycle: AssessmentCycle; status: Transition } | null>(null);
  const [pending, setPending] = useState(false);

  const selectedCycle = useMemo(
    () => cycles.find((cycle) => cycle.id === selectedCycleId) ?? null,
    [cycles, selectedCycleId],
  );

  const loadCycles = useCallback(async () => {
    setCyclesLoading(true);
    setCyclesError(null);
    try {
      const [cycleResponse, termResponse] = await Promise.all([
        academicsService.listAssessmentCycles({ per_page: 100 }),
        academicsService.listAcademicTerms({ per_page: 100 }),
      ]);
      if (!cycleResponse.success || !cycleResponse.data) {
        throw new Error(responseMessage(cycleResponse, "Assessment cycles could not be loaded"));
      }
      setCycles(cycleResponse.data.assessment_cycles);
      if (termResponse.success && termResponse.data) setTerms(termResponse.data.terms);
      setSelectedCycleId((current) => {
        if (current && cycleResponse.data?.assessment_cycles.some((cycle) => cycle.id === current)) return current;
        return cycleResponse.data?.assessment_cycles[0]?.id ?? null;
      });
    } catch (error) {
      setCyclesError(error instanceof Error ? error.message : "Assessment cycles could not be loaded");
    } finally {
      setCyclesLoading(false);
    }
  }, []);

  const loadComponents = useCallback(async (cycle: AssessmentCycle | null) => {
    if (!cycle) {
      setComponents([]);
      setAssignments([]);
      return;
    }
    setComponentsLoading(true);
    setComponentsError(null);
    try {
      const [componentResponse, assignmentResponse] = await Promise.all([
        academicsService.listAssessmentComponents(cycle.id, { per_page: 100 }),
        academicsService.listTeachingAssignments({
          per_page: 100,
          academic_year_id: cycle.academic_year_id,
          status: "active",
        }),
      ]);
      if (!componentResponse.success || !componentResponse.data) {
        throw new Error(responseMessage(componentResponse, "Assessment components could not be loaded"));
      }
      setComponents(componentResponse.data.assessment_components);
      if (assignmentResponse.success && assignmentResponse.data) {
        setAssignments(assignmentResponse.data.assignments);
      } else {
        setAssignments([]);
      }
    } catch (error) {
      setComponentsError(error instanceof Error ? error.message : "Assessment components could not be loaded");
    } finally {
      setComponentsLoading(false);
    }
  }, []);

  useEffect(() => { void loadCycles(); }, [loadCycles]);
  useEffect(() => { void loadComponents(selectedCycle); }, [loadComponents, selectedCycle]);

  usePageChrome(
    "Assessments",
    <Button onClick={() => setCycleDrawer(null)}><Plus className="size-4" />Add cycle</Button>,
  );

  const removeCycle = async () => {
    if (!deleteCycle || pending) return;
    setPending(true);
    const response = await academicsService.deleteAssessmentCycle(deleteCycle.id);
    setPending(false);
    if (!response.success) return toast.error(responseMessage(response, "Assessment cycle could not be removed"));
    toast.success("Assessment cycle removed");
    setDeleteCycle(null);
    void loadCycles();
  };

  const removeComponent = async () => {
    if (!deleteComponent || pending) return;
    setPending(true);
    const response = await academicsService.deleteAssessmentComponent(deleteComponent.id);
    setPending(false);
    if (!response.success) return toast.error(responseMessage(response, "Assessment component could not be removed"));
    toast.success("Assessment component removed");
    setDeleteComponent(null);
    void loadComponents(selectedCycle);
    void loadCycles();
  };

  const applyTransition = async () => {
    if (!transition || pending) return;
    setPending(true);
    const { cycle, status } = transition;
    const response = await academicsService.updateAssessmentCycle(cycle.id, {
      academic_term_id: cycle.academic_term_id,
      code: cycle.code,
      name: cycle.name,
      status,
    });
    setPending(false);
    if (!response.success) return toast.error(responseMessage(response, "Assessment cycle could not be updated"));
    toast.success(status === "open" ? "Assessment cycle opened" : "Assessment cycle closed");
    setTransition(null);
    void loadCycles();
  };

  const assignmentWeights = useMemo(() => {
    const totals = new Map<string, number>();
    for (const component of components) {
      if (component.status === "active") {
        totals.set(
          component.teaching_assignment_id,
          (totals.get(component.teaching_assignment_id) ?? 0) + component.weight_basis_points,
        );
      }
    }
    return totals;
  }, [components]);

  return (
    <div className="space-y-8">
      <section aria-labelledby="cycles-heading" className="space-y-4">
        <div>
          <h2 className="text-base font-semibold text-[var(--text-strong)]" id="cycles-heading">Assessment cycles</h2>
          <p className="mt-1 text-sm text-[var(--text-muted)]">Set the assessment structure for an academic term.</p>
        </div>
        <TableWrap>
          {cyclesLoading ? <TableLoading columns={5} label="Loading assessment cycles…" />
            : cyclesError ? <TableError description={cyclesError} onRetry={() => void loadCycles()} />
              : cycles.length === 0 ? <TableEmpty description="Add a cycle for a planned or active academic term." icon={<CalendarCheck2 />} title="No assessment cycles yet" />
                : <TableScroll><Table><THead><tr><TH>Cycle</TH><TH>Term</TH><TH>Components</TH><TH>Status</TH><TH className="text-right">Actions</TH></tr></THead><TBody>{cycles.map((cycle) => {
                  const selected = cycle.id === selectedCycleId;
                  return <TR className={selected ? "bg-[var(--table-row-hover-bg)]" : undefined} key={cycle.id}>
                    <TD><button className="text-left" onClick={() => setSelectedCycleId(cycle.id)} type="button"><span className="block font-medium text-[var(--text-strong)]">{cycle.name}</span><span className="mt-0.5 block text-xs text-[var(--text-muted)]">{cycle.code}</span></button></TD>
                    <TD><span className="text-[var(--text-strong)]">{cycle.academic_term_name}</span><span className="mt-0.5 block text-xs text-[var(--text-muted)]">{cycle.academic_year_name}</span></TD>
                    <TD className="font-tabular text-[var(--text-muted)]">{cycle.component_count}</TD>
                    <TD><CycleStatus status={cycle.status} /></TD>
                    <TD><div className="flex justify-end gap-2">
                      <Button onClick={() => setSelectedCycleId(cycle.id)} size="sm" variant={selected ? "secondary" : "ghost"}>{selected ? "Selected" : "Components"}</Button>
                      {cycle.status === "draft" ? <Button aria-label={`Edit ${cycle.name}`} onClick={() => setCycleDrawer(cycle)} size="icon-sm" variant="ghost"><Edit className="size-4" /></Button> : null}
                      {cycle.status === "draft" ? <Button onClick={() => setTransition({ cycle, status: "open" })} size="sm" variant="secondary">Open</Button> : null}
                      {cycle.status === "open" ? <Button onClick={() => setTransition({ cycle, status: "closed" })} size="sm" variant="secondary">Close</Button> : null}
                      {cycle.status === "draft" ? <Button aria-label={`Remove ${cycle.name}`} onClick={() => setDeleteCycle(cycle)} size="icon-sm" variant="ghost"><Trash2 className="size-4 text-[var(--tone-danger)]" /></Button> : null}
                    </div></TD>
                  </TR>;
                })}</TBody></Table></TableScroll>}
        </TableWrap>
      </section>

      <section aria-labelledby="components-heading" className="space-y-4">
        <div className="flex flex-col gap-3 sm:flex-row sm:items-end sm:justify-between">
          <div>
            <h2 className="text-base font-semibold text-[var(--text-strong)]" id="components-heading">Components</h2>
            <p className="mt-1 text-sm text-[var(--text-muted)]">{selectedCycle ? `${selectedCycle.name} · ${selectedCycle.academic_term_name}` : "Select an assessment cycle."}</p>
          </div>
          {selectedCycle?.status === "draft" ? <Button onClick={() => setComponentDrawer(null)} variant="secondary"><Plus className="size-4" />Add component</Button> : null}
        </div>
        <TableWrap>
          {!selectedCycle ? <TableEmpty description="Choose a cycle above to view its components." icon={<FileCheck2 />} title="No cycle selected" />
            : componentsLoading ? <TableLoading columns={6} label="Loading assessment components…" />
              : componentsError ? <TableError description={componentsError} onRetry={() => void loadComponents(selectedCycle)} />
                : components.length === 0 ? <TableEmpty description={selectedCycle.status === "draft" ? "Add weighted components for each teaching assignment." : "This cycle has no components."} icon={<FileCheck2 />} title="No assessment components yet" />
                  : <TableScroll><Table><THead><tr><TH>Component</TH><TH>Class and subject</TH><TH>Teacher</TH><TH>Marks</TH><TH>Weight</TH><TH>Date</TH><TH>Status</TH><TH className="text-right">Actions</TH></tr></THead><TBody>{components.map((component) => <TR key={component.id}>
                    <TD><span className="font-medium text-[var(--text-strong)]">{component.name}</span><span className="mt-0.5 block text-xs capitalize text-[var(--text-muted)]">{component.code} · {component.assessment_kind}</span></TD>
                    <TD><span className="text-[var(--text-strong)]">{component.class_group_name}</span><span className="mt-0.5 block text-xs text-[var(--text-muted)]">{component.subject_name}</span></TD>
                    <TD className="text-[var(--text-muted)]">{component.teacher_name}</TD>
                    <TD className="font-tabular text-[var(--text-muted)]">{component.maximum_marks}</TD>
                    <TD><span className="font-tabular text-[var(--text-strong)]">{formatWeight(component.weight_basis_points)}</span><span className="mt-0.5 block text-xs text-[var(--text-muted)]">Assignment total {formatWeight(assignmentWeights.get(component.teaching_assignment_id) ?? 0)}</span></TD>
                    <TD className="text-[var(--text-muted)]">{component.occurs_on ?? "Not set"}</TD>
                    <TD><Badge tone={component.status === "active" ? "success" : "neutral"}>{component.status}</Badge></TD>
                    <TD><div className="flex justify-end gap-1">{selectedCycle.status === "draft" ? <><Button aria-label={`Edit ${component.name}`} onClick={() => setComponentDrawer(component)} size="icon-sm" variant="ghost"><Edit className="size-4" /></Button><Button aria-label={`Remove ${component.name}`} onClick={() => setDeleteComponent(component)} size="icon-sm" variant="ghost"><Trash2 className="size-4 text-[var(--tone-danger)]" /></Button></> : null}</div></TD>
                  </TR>)}</TBody></Table></TableScroll>}
        </TableWrap>
      </section>

      <CycleDrawer cycle={cycleDrawer ?? null} onClose={() => setCycleDrawer(undefined)} onSaved={() => { setCycleDrawer(undefined); void loadCycles(); }} open={cycleDrawer !== undefined} terms={terms} />
      <ComponentDrawer assignments={assignments} component={componentDrawer ?? null} cycle={selectedCycle} onClose={() => setComponentDrawer(undefined)} onSaved={() => { setComponentDrawer(undefined); void loadComponents(selectedCycle); void loadCycles(); }} open={componentDrawer !== undefined} />
      <ConfirmDrawer confirmLabel="Remove cycle" description={`Remove ${deleteCycle?.name ?? "this assessment cycle"}? A cycle with components cannot be removed.`} isPending={pending} onClose={() => setDeleteCycle(null)} onConfirm={() => void removeCycle()} open={deleteCycle !== null} title="Remove assessment cycle?" />
      <ConfirmDrawer confirmLabel="Remove component" description={`Remove ${deleteComponent?.name ?? "this assessment component"} from the draft cycle?`} isPending={pending} onClose={() => setDeleteComponent(null)} onConfirm={() => void removeComponent()} open={deleteComponent !== null} title="Remove assessment component?" />
      <TransitionDrawer cycle={transition?.cycle ?? null} isPending={pending} onClose={() => setTransition(null)} onConfirm={() => void applyTransition()} open={transition !== null} status={transition?.status ?? "open"} />
    </div>
  );
}

function CycleDrawer({ cycle, onClose, onSaved, open, terms }: { cycle: AssessmentCycle | null; onClose: () => void; onSaved: () => void; open: boolean; terms: AcademicTerm[] }) {
  const [form, setForm] = useState<AssessmentCycleInput>({ academic_term_id: "", code: "", name: "", status: "draft" });
  const [saving, setSaving] = useState(false);
  useEffect(() => {
    if (!open) return;
    setForm(cycle ? { academic_term_id: cycle.academic_term_id, code: cycle.code, name: cycle.name, status: cycle.status } : { academic_term_id: terms.find((term) => term.status === "active")?.id ?? terms.find((term) => term.status === "planned")?.id ?? "", code: "", name: "", status: "draft" });
  }, [cycle, open, terms]);
  const submit = async (event: React.FormEvent) => {
    event.preventDefault();
    setSaving(true);
    const response = cycle ? await academicsService.updateAssessmentCycle(cycle.id, form) : await academicsService.createAssessmentCycle(form);
    setSaving(false);
    if (!response.success) return toast.error(responseMessage(response, "Assessment cycle could not be saved"));
    toast.success("Assessment cycle saved");
    onSaved();
  };
  return <DialogShell onClose={onClose} open={open}><DialogHeader onClose={onClose} title={cycle ? "Edit assessment cycle" : "Add assessment cycle"} /><form onSubmit={submit}><DialogBody className="space-y-5">
    <div><Label>Academic term</Label><Select className="mt-1.5" disabled={Boolean(cycle?.component_count)} onChange={(event) => setForm((current) => ({ ...current, academic_term_id: event.target.value }))} required value={form.academic_term_id}><option value="">Choose a term</option>{terms.filter((term) => term.status !== "closed" || term.id === form.academic_term_id).map((term) => <option key={term.id} value={term.id}>{term.academic_year_name} · {term.name} · {term.status}</option>)}</Select>{cycle?.component_count ? <p className="mt-1.5 text-xs text-[var(--text-muted)]">Remove the cycle components before changing its term.</p> : null}</div>
    <div><Label>Code</Label><Input className="mt-1.5" maxLength={40} onChange={(event) => setForm((current) => ({ ...current, code: event.target.value }))} required value={form.code} /></div>
    <div><Label>Name</Label><Input className="mt-1.5" maxLength={120} onChange={(event) => setForm((current) => ({ ...current, name: event.target.value }))} required value={form.name} /></div>
  </DialogBody><DialogFooter><Button disabled={saving} onClick={onClose} type="button" variant="ghost">Cancel</Button><Button disabled={saving} type="submit">{saving ? <><Loader2 className="size-4 animate-spin" />Saving…</> : "Save cycle"}</Button></DialogFooter></form></DialogShell>;
}

function ComponentDrawer({ assignments, component, cycle, onClose, onSaved, open }: { assignments: TeachingAssignment[]; component: AssessmentComponent | null; cycle: AssessmentCycle | null; onClose: () => void; onSaved: () => void; open: boolean }) {
  const [form, setForm] = useState<AssessmentComponentInput>({ teaching_assignment_id: "", code: "", name: "", assessment_kind: "test", maximum_marks: 100, weight_basis_points: 10000, occurs_on: null, status: "active" });
  const [weightPercent, setWeightPercent] = useState("100");
  const [saving, setSaving] = useState(false);
  useEffect(() => {
    if (!open) return;
    const next = component ? { teaching_assignment_id: component.teaching_assignment_id, code: component.code, name: component.name, assessment_kind: component.assessment_kind, maximum_marks: component.maximum_marks, weight_basis_points: component.weight_basis_points, occurs_on: component.occurs_on, status: component.status } : { teaching_assignment_id: assignments[0]?.id ?? "", code: "", name: "", assessment_kind: "test" as AssessmentKind, maximum_marks: 100, weight_basis_points: 10000, occurs_on: null, status: "active" as DirectoryStatus };
    setForm(next);
    setWeightPercent(String(next.weight_basis_points / 100));
  }, [assignments, component, open]);
  const field = <K extends keyof AssessmentComponentInput>(key: K, value: AssessmentComponentInput[K]) => setForm((current) => ({ ...current, [key]: value }));
  const submit = async (event: React.FormEvent) => {
    event.preventDefault();
    if (!cycle) return;
    const basisPoints = Math.round(Number(weightPercent) * 100);
    if (!Number.isFinite(basisPoints) || basisPoints < 1 || basisPoints > 10000) return toast.error("Weight must be between 0.01% and 100%");
    setSaving(true);
    const payload = { ...form, weight_basis_points: basisPoints };
    const response = component ? await academicsService.updateAssessmentComponent(component.id, payload) : await academicsService.createAssessmentComponent(cycle.id, payload);
    setSaving(false);
    if (!response.success) return toast.error(responseMessage(response, "Assessment component could not be saved"));
    toast.success("Assessment component saved");
    onSaved();
  };
  return <DialogShell onClose={onClose} open={open}><DialogHeader onClose={onClose} title={component ? "Edit assessment component" : "Add assessment component"} /><form onSubmit={submit}><DialogBody className="space-y-5">
    <div><Label>Teaching assignment</Label><Select className="mt-1.5" onChange={(event) => field("teaching_assignment_id", event.target.value)} required value={form.teaching_assignment_id}><option value="">Choose an assignment</option>{assignments.map((assignment) => <option key={assignment.id} value={assignment.id}>{assignment.class_group_name} · {assignment.subject_name} · {assignment.teacher_name}</option>)}</Select>{assignments.length === 0 ? <p className="mt-1.5 text-xs text-[var(--text-muted)]">Add an active teaching assignment for this academic year first.</p> : null}</div>
    <div className="grid gap-4 sm:grid-cols-2"><div><Label>Code</Label><Input className="mt-1.5" maxLength={40} onChange={(event) => field("code", event.target.value)} required value={form.code} /></div><div><Label>Type</Label><Select className="mt-1.5" onChange={(event) => field("assessment_kind", event.target.value as AssessmentKind)} value={form.assessment_kind}><option value="assignment">Assignment</option><option value="quiz">Quiz</option><option value="test">Test</option><option value="project">Project</option><option value="exam">Exam</option><option value="practical">Practical</option><option value="other">Other</option></Select></div></div>
    <div><Label>Name</Label><Input className="mt-1.5" maxLength={120} onChange={(event) => field("name", event.target.value)} required value={form.name} /></div>
    <div className="grid gap-4 sm:grid-cols-2"><div><Label>Maximum marks</Label><Input className="mt-1.5" max={100000} min={1} onChange={(event) => field("maximum_marks", Number(event.target.value))} required type="number" value={form.maximum_marks} /></div><div><Label>Weight (%)</Label><Input className="mt-1.5" max={100} min={0.01} onChange={(event) => setWeightPercent(event.target.value)} required step={0.01} type="number" value={weightPercent} /></div></div>
    <div><Label>Assessment date</Label><Input className="mt-1.5" onChange={(event) => field("occurs_on", event.target.value || null)} type="date" value={form.occurs_on ?? ""} /></div>
    <div><Label>Status</Label><Select className="mt-1.5" onChange={(event) => field("status", event.target.value as DirectoryStatus)} value={form.status}><option value="active">Active</option><option value="inactive">Inactive</option></Select></div>
  </DialogBody><DialogFooter><Button disabled={saving} onClick={onClose} type="button" variant="ghost">Cancel</Button><Button disabled={saving || assignments.length === 0} type="submit">{saving ? <><Loader2 className="size-4 animate-spin" />Saving…</> : "Save component"}</Button></DialogFooter></form></DialogShell>;
}

function TransitionDrawer({ cycle, isPending, onClose, onConfirm, open, status }: { cycle: AssessmentCycle | null; isPending: boolean; onClose: () => void; onConfirm: () => void; open: boolean; status: Transition }) {
  const opening = status === "open";
  return <DialogShell onClose={isPending ? () => undefined : onClose} open={open}><DialogHeader onClose={isPending ? undefined : onClose} title={opening ? "Open assessment cycle?" : "Close assessment cycle?"} /><DialogBody><div className="flex gap-4"><span className="flex size-10 shrink-0 items-center justify-center rounded-[9px] bg-[var(--badge-info-bg)] text-[var(--badge-info-text)]"><CheckCircle2 className="size-5" /></span><div className="space-y-2 text-sm leading-6 text-[var(--text-muted)]"><p>{opening ? `Open ${cycle?.name ?? "this cycle"} for mark capture? Its active component weights must total 100% for every teaching assignment.` : `Close ${cycle?.name ?? "this cycle"}? Closed cycles cannot be reopened.`}</p><p>{opening ? "Cycle details and components cannot be changed after opening." : "Finish any outstanding mark work before closing."}</p></div></div></DialogBody><DialogFooter><Button data-autofocus="true" disabled={isPending} onClick={onClose} type="button" variant="secondary">Cancel</Button><Button disabled={isPending} onClick={onConfirm} type="button">{isPending ? <Loader2 className="size-4 animate-spin" /> : null}{opening ? "Open cycle" : "Close cycle"}</Button></DialogFooter></DialogShell>;
}

function CycleStatus({ status }: { status: AssessmentCycleStatus }) {
  return <Badge dot tone={status === "open" ? "success" : status === "closed" ? "neutral" : "info"}>{status}</Badge>;
}

function formatWeight(basisPoints: number) {
  return `${(basisPoints / 100).toLocaleString(undefined, { maximumFractionDigits: 2 })}%`;
}

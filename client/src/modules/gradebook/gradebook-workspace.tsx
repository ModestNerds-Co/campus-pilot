import { useCallback, useEffect, useMemo, useState } from "react";
import { Link, useNavigate } from "@tanstack/react-router";
import { BookOpenCheck, Loader2, Plus, Search } from "lucide-react";
import toast from "react-hot-toast";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Table, TableControlsBar, TableEmpty, TableError, TableLoading, TableScroll,
  TableWrap, TBody, TD, TH, THead, TR,
} from "@/components/ui/data-table";
import { DialogBody, DialogFooter, DialogHeader, DialogShell } from "@/components/ui/dialog";
import { Input, Label, Select } from "@/components/ui/input";
import { usePageChrome } from "@/modules/admin/layouts/page-chrome";
import { useAuthStore } from "@/stores/auth-store";

import { gradebookService, responseMessage } from "./service";
import type { GradebookComponentReference, GradebookSheetStatus } from "./types";

export function GradebookWorkspace() {
  const navigate = useNavigate();
  const permissions = useAuthStore((state) => state.user?.permissions ?? []);
  const canCreate = permissions.includes("*") || permissions.includes("academics:teach");
  const canManage = permissions.includes("*") || permissions.includes("academics:manage");
  const [components, setComponents] = useState<GradebookComponentReference[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const [status, setStatus] = useState<"all" | "not_started" | GradebookSheetStatus>("all");
  const [cycleId, setCycleId] = useState("all");
  const [selected, setSelected] = useState<GradebookComponentReference | null>(null);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const response = await gradebookService.references();
      if (!response.success || !response.data) throw new Error(responseMessage(response, "Gradebook could not be loaded"));
      setComponents(response.data.components);
    } catch (loadError) {
      setError(loadError instanceof Error ? loadError.message : "Gradebook could not be loaded");
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => { void load(); }, [load]);
  usePageChrome("Gradebook");

  const cycles = useMemo(() => Array.from(new Map(components.map((component) => [component.assessment_cycle_id, component.assessment_cycle_name])).entries()), [components]);
  const filtered = useMemo(() => components.filter((component) => {
    const matchesQuery = !query.trim() || [component.assessment_component_name, component.assessment_component_code, component.class_group_name, component.subject_name, component.teacher_name].some((value) => value.toLowerCase().includes(query.trim().toLowerCase()));
    const componentStatus = component.mark_sheet_status ?? "not_started";
    return matchesQuery && (status === "all" || componentStatus === status) && (cycleId === "all" || component.assessment_cycle_id === cycleId);
  }), [components, cycleId, query, status]);

  return <div className="space-y-6">
    <p className="text-sm text-[var(--text-muted)]">{canManage ? "Capture, review, and publish learner marks." : "Capture and submit marks for your assigned assessment components."}</p>

    <TableControlsBar>
      <div className="relative min-w-0 flex-1 sm:min-w-64 sm:max-w-md"><Search className="pointer-events-none absolute left-3 top-1/2 size-4 -translate-y-1/2 text-[var(--text-subtle)]" /><Input aria-label="Search gradebook" className="pl-9" onChange={(event) => setQuery(event.target.value)} placeholder="Search subject, class, teacher, or component" value={query} /></div>
      <Select aria-label="Assessment cycle filter" className="sm:w-56" onChange={(event) => setCycleId(event.target.value)} value={cycleId}><option value="all">All assessment cycles</option>{cycles.map(([id, name]) => <option key={id} value={id}>{name}</option>)}</Select>
      <Select aria-label="Mark sheet status filter" className="sm:w-44" onChange={(event) => setStatus(event.target.value as typeof status)} value={status}><option value="all">All statuses</option><option value="not_started">Not started</option><option value="draft">Draft</option><option value="submitted">Submitted</option><option value="published">Published</option></Select>
    </TableControlsBar>

    <TableWrap>
      {loading ? <TableLoading columns={7} label="Loading gradebook…" /> : error ? <TableError description={error} onRetry={() => void load()} /> : filtered.length === 0 ? <TableEmpty description={components.length === 0 ? (canManage ? "Open an assessment cycle and add active components first." : "No assessment components are currently assigned to you.") : "Change the current filters."} icon={<BookOpenCheck />} title={components.length === 0 ? "No assessment components are ready" : "No components match these filters"} /> : <TableScroll><Table className="min-w-[1020px]"><THead><tr><TH>Assessment</TH><TH>Class</TH><TH>Subject</TH><TH>Teacher</TH><TH>Weight</TH><TH>Status</TH><TH className="w-36">Action</TH></tr></THead><TBody>
        {filtered.map((component) => <TR key={component.assessment_component_id}>
          <TD><p className="font-medium text-[var(--text-strong)]">{component.assessment_component_name}</p><p className="mt-1 text-xs text-[var(--text-muted)]">{component.assessment_cycle_name} · {component.assessment_component_code} · {component.maximum_marks} marks</p></TD>
          <TD className="text-[var(--text-body)]">{component.class_group_name}</TD>
          <TD className="text-[var(--text-body)]">{component.subject_name}</TD>
          <TD className="text-[var(--text-muted)]">{component.teacher_name}</TD>
          <TD className="font-tabular text-[var(--text-muted)]">{formatBasisPoints(component.weight_basis_points)}</TD>
          <TD>{component.mark_sheet_status ? <Badge tone={statusTone(component.mark_sheet_status)}>{displayValue(component.mark_sheet_status)}</Badge> : <Badge>Not started</Badge>}</TD>
          <TD>{component.mark_sheet_id ? <Link className="text-sm font-semibold text-[var(--brand-strong)] hover:underline" params={{ markSheetId: component.mark_sheet_id }} to="/modules/academics/gradebook/mark-sheets/$markSheetId">Open marks</Link> : canCreate && component.assessment_cycle_status === "open" ? <Button onClick={() => setSelected(component)} size="sm" variant="secondary"><Plus className="size-4" />Start marks</Button> : <span className="text-xs text-[var(--text-subtle)]">Unavailable</span>}</TD>
        </TR>)}
      </TBody></Table></TableScroll>}
    </TableWrap>

    <CreateMarkSheetDrawer component={selected} onClose={() => setSelected(null)} onCreated={(id) => { setSelected(null); void navigate({ to: "/modules/academics/gradebook/mark-sheets/$markSheetId", params: { markSheetId: id } }); }} />
  </div>;
}

function CreateMarkSheetDrawer({ component, onClose, onCreated }: { component: GradebookComponentReference | null; onClose: () => void; onCreated: (id: string) => void }) {
  const [rosterOn, setRosterOn] = useState("");
  const [saving, setSaving] = useState(false);
  useEffect(() => { if (component) setRosterOn(component.occurs_on ?? component.academic_term_starts_on); }, [component]);
  if (!component) return null;

  const submit = async (event: React.FormEvent) => {
    event.preventDefault();
    if (saving) return;
    setSaving(true);
    try {
      const response = await gradebookService.createMarkSheet(component.assessment_component_id, rosterOn);
      if (!response.success || !response.data) throw new Error(responseMessage(response, "Mark sheet could not be created"));
      toast.success("Mark sheet created");
      onCreated(response.data.id);
    } catch (saveError) {
      toast.error(saveError instanceof Error ? saveError.message : "Mark sheet could not be created");
    } finally {
      setSaving(false);
    }
  };

  return <DialogShell onClose={saving ? () => undefined : onClose} open><DialogHeader onClose={saving ? undefined : onClose} title="Start mark sheet" /><form onSubmit={submit}><DialogBody className="space-y-5">
    <div><Label>Assessment</Label><p className="mt-2 font-medium text-[var(--text-strong)]">{component.assessment_component_name}</p><p className="mt-1 text-sm text-[var(--text-muted)]">{component.subject_name} · {component.class_group_name}</p></div>
    <div><Label htmlFor="gradebook-roster-date">Roster date</Label><Input className="mt-1.5" data-autofocus="true" disabled={Boolean(component.occurs_on)} id="gradebook-roster-date" max={component.academic_term_ends_on} min={component.academic_term_starts_on} onChange={(event) => setRosterOn(event.target.value)} required type="date" value={rosterOn} /><p className="mt-2 text-xs leading-5 text-[var(--text-muted)]">Learners enrolled in this class on this date will be added to the mark sheet.</p></div>
  </DialogBody><DialogFooter><Button disabled={saving} onClick={onClose} type="button" variant="secondary">Cancel</Button><Button disabled={saving || !rosterOn} type="submit">{saving ? <><Loader2 className="size-4 animate-spin" />Creating…</> : "Create mark sheet"}</Button></DialogFooter></form></DialogShell>;
}

function statusTone(status: GradebookSheetStatus): "warning" | "info" | "success" { return status === "published" ? "success" : status === "submitted" ? "info" : "warning"; }
function displayValue(value: string) { return value.replace(/_/g, " ").replace(/^./, (letter) => letter.toUpperCase()); }
function formatBasisPoints(value: number) { return `${(value / 100).toFixed(value % 100 === 0 ? 0 : 2)}%`; }

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Link, useNavigate } from "@tanstack/react-router";
import { ClipboardCheck, Loader2, Plus } from "lucide-react";
import toast from "react-hot-toast";

import { Button } from "@/components/ui/button";
import {
  Table, TableControlsBar, TableControlsPagination, TableEmpty, TableError,
  TableLoading, TableScroll, TableWrap, TBody, TD, TH, THead, TR,
} from "@/components/ui/data-table";
import { DialogBody, DialogFooter, DialogHeader } from "@/components/ui/dialog";
import { Input, Label, Select, Textarea } from "@/components/ui/input";
import { usePageChrome } from "@/modules/admin/layouts/page-chrome";
import { useAuthStore } from "@/stores/auth-store";

import { GuardedDrawer } from "./guarded-drawer";
import { learningService, responseMessage } from "./service";
import type { LearningAssignment, LearningAssignmentStatus, LearningSpace } from "./types";
import { formatHundredths, formatLearningDateTime, LearningStatusBadge, parseHundredths } from "./ui";

export interface LearningAssignmentsSearchState {
  page: number;
  status: "all" | LearningAssignmentStatus;
}

export type LearningAssignmentsSearchChange = (next: LearningAssignmentsSearchState) => void;

export function LearningAssignmentsWorkspace({ onSearchChange, search, spaceId }: {
  onSearchChange: LearningAssignmentsSearchChange;
  search: LearningAssignmentsSearchState;
  spaceId: string;
}) {
  const navigate = useNavigate();
  const permissions = useAuthStore((state) => state.user?.permissions ?? []);
  const canTeach = permissions.includes("*") || permissions.includes("learning:teach");
  const [space, setSpace] = useState<LearningSpace | null>(null);
  const [assignments, setAssignments] = useState<LearningAssignment[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [totalPages, setTotalPages] = useState(1);
  const [createOpen, setCreateOpen] = useState(false);
  const requestRef = useRef(0);

  const load = useCallback(async () => {
    const requestId = ++requestRef.current;
    setLoading(true);
    setError(null);
    try {
      const [spaceResponse, assignmentResponse] = await Promise.all([
        learningService.space(spaceId),
        learningService.assignments(spaceId, {
          page: search.page,
          per_page: 25,
          status: search.status === "all" ? undefined : search.status,
        }),
      ]);
      if (!spaceResponse.success || !spaceResponse.data) throw new Error(responseMessage(spaceResponse, "Learning space could not be loaded"));
      if (!assignmentResponse.success || !assignmentResponse.data) throw new Error(responseMessage(assignmentResponse, "Assignments could not be loaded"));
      if (requestId !== requestRef.current) return;
      setSpace(spaceResponse.data);
      setAssignments(assignmentResponse.data.assignments);
      setTotalPages(Math.max(1, assignmentResponse.pagination?.total_pages ?? 1));
    } catch (loadError) {
      if (requestId !== requestRef.current) return;
      setError(loadError instanceof Error ? loadError.message : "Assignments could not be loaded");
    } finally {
      if (requestId === requestRef.current) setLoading(false);
    }
  }, [search.page, search.status, spaceId]);

  useEffect(() => {
    void load();
    return () => { requestRef.current += 1; };
  }, [load]);

  const publishedUnits = useMemo(
    () => space?.units.filter((unit) => unit.status === "published") ?? [],
    [space],
  );
  const canCreate = canTeach && space?.status === "published" && publishedUnits.length > 0;

  usePageChrome(
    "Assignments",
    canCreate ? <Button onClick={() => setCreateOpen(true)}><Plus className="size-4" />New assignment</Button> : null,
  );

  return <div className="space-y-6">
    <Link className="text-sm font-medium text-[var(--text-muted)] hover:text-[var(--text-strong)]" params={{ spaceId }} to="/modules/learning/spaces/$spaceId">← {space?.title ?? "Learning space"}</Link>
    <div className="flex flex-wrap items-end justify-between gap-4">
      <div><p className="text-sm font-medium text-[var(--text-strong)]">{space ? `${space.subject_name} · ${space.class_group_name}` : "Space assignments"}</p><p className="mt-1 text-sm text-[var(--text-muted)]">Work published for this class space.</p></div>
      {canTeach && space && !canCreate ? <p className="max-w-md text-xs text-[var(--text-muted)]">Publish the space and at least one unit before creating an assignment.</p> : null}
    </div>
    <TableControlsBar>
      <Select aria-label="Assignment status" className="sm:w-48" onChange={(event) => onSearchChange({ page: 1, status: event.target.value as LearningAssignmentsSearchState["status"] })} value={search.status}>
        <option value="all">All statuses</option>{canTeach ? <option value="draft">Draft</option> : null}<option value="published">Published</option><option value="closed">Closed</option>
      </Select>
      {!loading && assignments.length ? <TableControlsPagination onNext={() => onSearchChange({ ...search, page: Math.min(totalPages, search.page + 1) })} onPrevious={() => onSearchChange({ ...search, page: Math.max(1, search.page - 1) })} page={search.page} totalPages={totalPages} /> : null}
    </TableControlsBar>
    <TableWrap>
      {loading ? <TableLoading columns={canTeach ? 6 : 4} label="Loading assignments…" /> : error ? <TableError description={error} onRetry={() => void load()} /> : assignments.length === 0 ? <TableEmpty description={search.status !== "all" ? "Change the current status filter." : canTeach ? "Create the first assignment in a published unit." : "Published assignments will appear here."} icon={<ClipboardCheck />} title={search.status !== "all" ? "No assignments match" : "No assignments yet"} /> : <TableScroll><Table className="min-w-[820px]"><THead><tr><TH>Assignment</TH><TH>Due</TH><TH>Maximum</TH><TH>Status</TH>{canTeach ? <><TH>Submissions</TH><TH>Recipients</TH></> : null}</tr></THead><TBody>
        {assignments.map((assignment) => <TR key={assignment.id}><TD><Link className="font-semibold text-[var(--text-strong)] hover:text-[var(--brand-strong)] hover:underline" params={{ assignmentId: assignment.id, spaceId }} search={{ tab: canTeach ? "submissions" : "work", submission_page: 1, submission_status: "all" }} to="/modules/learning/spaces/$spaceId/assignments/$assignmentId">{assignment.title}</Link><p className="mt-1 text-xs text-[var(--text-muted)]">Position {assignment.position}</p></TD><TD className="whitespace-nowrap text-[var(--text-muted)]">{formatLearningDateTime(assignment.due_at)}</TD><TD className="font-tabular text-[var(--text-muted)]">{formatHundredths(assignment.max_score_hundredths)}</TD><TD><LearningStatusBadge status={assignment.status} /></TD>{canTeach ? <><TD className="font-tabular text-[var(--text-muted)]">{assignment.submission_count}</TD><TD className="font-tabular text-[var(--text-muted)]">{assignment.recipient_count || "—"}</TD></> : null}</TR>)}
      </TBody></Table></TableScroll>}
    </TableWrap>
    <CreateAssignmentDrawer assignmentCount={assignments.length} onClose={() => setCreateOpen(false)} onCreated={(assignment) => { setCreateOpen(false); void navigate({ to: "/modules/learning/spaces/$spaceId/assignments/$assignmentId", params: { spaceId, assignmentId: assignment.id }, search: { tab: "brief", submission_page: 1, submission_status: "all" } }); }} open={createOpen} units={publishedUnits} />
  </div>;
}

function CreateAssignmentDrawer({ assignmentCount, onClose, onCreated, open, units }: {
  assignmentCount: number;
  onClose: () => void;
  onCreated: (assignment: LearningAssignment) => void;
  open: boolean;
  units: LearningSpace["units"];
}) {
  const [unitId, setUnitId] = useState("");
  const [position, setPosition] = useState(1);
  const [title, setTitle] = useState("");
  const [instructions, setInstructions] = useState("");
  const [dueAt, setDueAt] = useState("");
  const [maximum, setMaximum] = useState("");
  const [saving, setSaving] = useState(false);
  const editorSessionRef = useRef(false);

  useEffect(() => {
    if (!open) { editorSessionRef.current = false; return; }
    if (editorSessionRef.current) return;
    editorSessionRef.current = true;
    setUnitId(units[0]?.id ?? ""); setPosition(assignmentCount + 1); setTitle(""); setInstructions(""); setDueAt(""); setMaximum("");
  }, [assignmentCount, open, units]);

  const dirty = Boolean(title || instructions || dueAt || maximum || (unitId && unitId !== units[0]?.id));
  const maximumHundredths = parseHundredths(maximum);
  const submit = async (event: React.FormEvent) => {
    event.preventDefault();
    if (!unitId || !dueAt || maximumHundredths === null || maximumHundredths <= 0 || saving) return;
    setSaving(true);
    try {
      const response = await learningService.createAssignment(unitId, {
        position,
        title: title.trim(),
        instructions: instructions.trim(),
        due_at: new Date(dueAt).toISOString(),
        max_score_hundredths: maximumHundredths,
      });
      if (!response.success || !response.data) throw new Error(responseMessage(response, "Assignment could not be created"));
      toast.success("Assignment draft created"); onCreated(response.data);
    } catch (saveError) {
      toast.error(saveError instanceof Error ? saveError.message : "Assignment could not be created");
    } finally { setSaving(false); }
  };

  return <GuardedDrawer dirty={dirty} discardDescription="The unsaved assignment brief and due date will be lost." onClose={onClose} open={open} pending={saving} panelClassName="sm:max-w-[720px]">
    {(requestClose) => <><DialogHeader onClose={saving ? undefined : requestClose} title="New assignment" /><form onSubmit={submit}><DialogBody className="space-y-5">
      <div><Label htmlFor="assignment-unit">Unit</Label><Select className="mt-1.5" data-autofocus="true" id="assignment-unit" onChange={(event) => setUnitId(event.target.value)} required value={unitId}>{units.map((unit) => <option key={unit.id} value={unit.id}>{unit.position}. {unit.title}</option>)}</Select></div>
      <div><Label htmlFor="assignment-title">Title</Label><Input className="mt-1.5" id="assignment-title" maxLength={200} onChange={(event) => setTitle(event.target.value)} required value={title} /></div>
      <div><Label htmlFor="assignment-instructions">Instructions</Label><Textarea className="mt-1.5 min-h-40" id="assignment-instructions" maxLength={20000} onChange={(event) => setInstructions(event.target.value)} required value={instructions} /></div>
      <div className="grid gap-5 sm:grid-cols-2"><div><Label htmlFor="assignment-due">Due</Label><Input className="mt-1.5" id="assignment-due" onChange={(event) => setDueAt(event.target.value)} required type="datetime-local" value={dueAt} /></div><div><Label htmlFor="assignment-maximum">Maximum score</Label><Input className="mt-1.5" id="assignment-maximum" inputMode="decimal" min="0.01" onChange={(event) => setMaximum(event.target.value)} placeholder="100.00" required step="0.01" type="number" value={maximum} /></div></div>
      <div><Label htmlFor="assignment-position">Position</Label><Input className="mt-1.5" id="assignment-position" min={1} onChange={(event) => setPosition(Number(event.target.value))} required type="number" value={position} /></div>
    </DialogBody><DialogFooter><Button disabled={saving} onClick={requestClose} type="button" variant="secondary">Cancel</Button><Button disabled={saving || !unitId || !title.trim() || !instructions.trim() || !dueAt || maximumHundredths === null || maximumHundredths <= 0} type="submit">{saving ? <><Loader2 className="size-4 animate-spin" />Creating…</> : "Create assignment"}</Button></DialogFooter></form></>}
  </GuardedDrawer>;
}

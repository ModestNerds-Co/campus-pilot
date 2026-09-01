import { useCallback, useEffect, useRef, useState } from "react";
import { Link, useNavigate } from "@tanstack/react-router";
import { BookOpenCheck, Loader2, Plus, Search, TriangleAlert } from "lucide-react";
import toast from "react-hot-toast";

import { SearchableSelect } from "@/components/searchable-select";
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
import type { LearningReferenceData, LearningSpaceStatus, LearningSpaceSummary } from "./types";
import { formatLearningDate, LearningStatusBadge } from "./ui";

export interface LearningSpacesSearchState {
  page: number;
  q: string;
  status: "all" | LearningSpaceStatus;
}

export type LearningSpacesSearchChange = (
  next: LearningSpacesSearchState,
  options?: { replace?: boolean },
) => void;

export function LearningSpacesWorkspace({ onSearchChange, search }: {
  onSearchChange: LearningSpacesSearchChange;
  search: LearningSpacesSearchState;
}) {
  const navigate = useNavigate();
  const permissions = useAuthStore((state) => state.user?.permissions ?? []);
  const canTeach = permissions.includes("*") || permissions.includes("learning:teach");
  const [records, setRecords] = useState<LearningSpaceSummary[]>([]);
  const [references, setReferences] = useState<LearningReferenceData | null>(null);
  const [referencesLoading, setReferencesLoading] = useState(false);
  const [referencesError, setReferencesError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [totalPages, setTotalPages] = useState(1);
  const [createOpen, setCreateOpen] = useState(false);
  const listRequestRef = useRef(0);
  const referencesRequestRef = useRef(0);

  const load = useCallback(async () => {
    const requestId = ++listRequestRef.current;
    setLoading(true);
    setError(null);
    try {
      const response = await learningService.spaces({
        page: search.page,
        per_page: 25,
        search: search.q.trim() || undefined,
        status: search.status === "all" ? undefined : search.status,
      });
      if (!response.success || !response.data) throw new Error(responseMessage(response, "Learning spaces could not be loaded"));
      if (requestId !== listRequestRef.current) return;
      setRecords(response.data.spaces);
      setTotalPages(Math.max(1, response.pagination?.total_pages ?? 1));
    } catch (loadError) {
      if (requestId !== listRequestRef.current) return;
      setError(loadError instanceof Error ? loadError.message : "Learning spaces could not be loaded");
    } finally {
      if (requestId === listRequestRef.current) setLoading(false);
    }
  }, [search.page, search.q, search.status]);

  const loadReferences = useCallback(async () => {
    if (!canTeach) return;
    const requestId = ++referencesRequestRef.current;
    setReferencesLoading(true);
    setReferencesError(null);
    try {
      const response = await learningService.references();
      if (!response.success || !response.data) throw new Error(responseMessage(response, "Teaching assignments could not be loaded"));
      if (requestId === referencesRequestRef.current) setReferences(response.data);
    } catch (referenceError) {
      if (requestId !== referencesRequestRef.current) return;
      setReferencesError(referenceError instanceof Error ? referenceError.message : "Teaching assignments could not be loaded");
    } finally {
      if (requestId === referencesRequestRef.current) setReferencesLoading(false);
    }
  }, [canTeach]);

  useEffect(() => {
    void load();
    return () => { listRequestRef.current += 1; };
  }, [load]);
  useEffect(() => {
    void loadReferences();
    return () => { referencesRequestRef.current += 1; };
  }, [loadReferences]);

  usePageChrome("Learning spaces", canTeach ? <Button onClick={() => setCreateOpen(true)}><Plus className="size-4" />New space</Button> : null);
  const filtered = Boolean(search.q.trim() || search.status !== "all");

  return <div className="space-y-6">
    <p className="text-sm text-[var(--text-muted)]">Class learning spaces available to your account.</p>
    <TableControlsBar>
      <Input aria-label="Search learning spaces" className="sm:w-72" leadingIcon={<Search />} onChange={(event) => onSearchChange({ ...search, page: 1, q: event.target.value }, { replace: true })} placeholder="Search spaces" value={search.q} />
      <Select aria-label="Status filter" className="sm:w-44" onChange={(event) => onSearchChange({ ...search, page: 1, status: event.target.value as LearningSpacesSearchState["status"] })} value={search.status}>
        <option value="all">All statuses</option><option value="draft">Draft</option><option value="published">Published</option><option value="archived">Archived</option>
      </Select>
      {!loading && records.length ? <TableControlsPagination onNext={() => onSearchChange({ ...search, page: Math.min(totalPages, search.page + 1) })} onPrevious={() => onSearchChange({ ...search, page: Math.max(1, search.page - 1) })} page={search.page} totalPages={totalPages} /> : null}
    </TableControlsBar>
    <TableWrap>
      {loading ? <TableLoading columns={6} label="Loading learning spaces…" /> : error ? <TableError description={error} onRetry={() => void load()} /> : records.length === 0 ? <TableEmpty description={filtered ? "Change the current filters." : canTeach ? "Create a space from an active teaching assignment." : "No published spaces are available for your classes."} icon={<BookOpenCheck />} title={filtered ? "No spaces match" : "No learning spaces yet"} /> : <TableScroll><Table className="min-w-[880px]"><THead><tr><TH>Space</TH><TH>Class</TH><TH>Teacher</TH><TH>Status</TH><TH>Units</TH><TH>Updated</TH></tr></THead><TBody>
        {records.map((record) => <TR key={record.id}><TD><Link className="font-semibold text-[var(--text-strong)] hover:text-[var(--brand-strong)] hover:underline" params={{ spaceId: record.id }} to="/modules/learning/spaces/$spaceId">{record.title}</Link><p className="mt-1 text-xs text-[var(--text-muted)]">{record.subject_name} · {record.academic_term_name}</p></TD><TD>{record.class_group_name}</TD><TD className="text-[var(--text-muted)]">{record.teacher_name}</TD><TD><LearningStatusBadge status={record.status} /></TD><TD className="font-tabular text-[var(--text-muted)]">{record.published_unit_count} / {record.unit_count} published</TD><TD className="text-[var(--text-muted)]">{formatLearningDate(record.updated_at)}</TD></TR>)}
      </TBody></Table></TableScroll>}
    </TableWrap>
    <CreateSpaceDrawer onClose={() => setCreateOpen(false)} onCreated={(id) => { setCreateOpen(false); void navigate({ to: "/modules/learning/spaces/$spaceId", params: { spaceId: id } }); }} onRetryReferences={() => void loadReferences()} open={createOpen} references={references} referencesError={referencesError} referencesLoading={referencesLoading} />
  </div>;
}

function CreateSpaceDrawer({ onClose, onCreated, onRetryReferences, open, references, referencesError, referencesLoading }: {
  onClose: () => void;
  onCreated: (id: string) => void;
  onRetryReferences: () => void;
  open: boolean;
  references: LearningReferenceData | null;
  referencesError: string | null;
  referencesLoading: boolean;
}) {
  const [assignmentId, setAssignmentId] = useState<string | null>(null);
  const [title, setTitle] = useState("");
  const [summary, setSummary] = useState("");
  const [saving, setSaving] = useState(false);
  const editorSessionRef = useRef(false);

  useEffect(() => {
    if (!open) { editorSessionRef.current = false; return; }
    if (editorSessionRef.current) return;
    editorSessionRef.current = true;
    setAssignmentId(null); setTitle(""); setSummary("");
  }, [open]);

  const dirty = Boolean(assignmentId || title || summary);
  const submit = async (event: React.FormEvent) => {
    event.preventDefault();
    if (!references?.active_term || !assignmentId || saving) return;
    setSaving(true);
    try {
      const response = await learningService.createSpace({ teaching_assignment_id: assignmentId, academic_term_id: references.active_term.id, title: title.trim(), summary: summary.trim() || null });
      if (!response.success || !response.data) throw new Error(responseMessage(response, "Learning space could not be created"));
      toast.success("Learning space created"); onCreated(response.data.id);
    } catch (saveError) {
      toast.error(saveError instanceof Error ? saveError.message : "Learning space could not be created");
    } finally { setSaving(false); }
  };

  return <GuardedDrawer dirty={dirty} discardDescription="The unsaved teaching assignment, title, and summary will be lost." onClose={onClose} open={open} pending={saving}>
    {(requestClose) => <><DialogHeader onClose={saving ? undefined : requestClose} title="New learning space" />
      {referencesLoading && !references ? <DialogBody><div className="flex items-center gap-2 text-sm text-[var(--text-muted)]"><Loader2 className="size-4 animate-spin" />Loading teaching assignments…</div></DialogBody> : referencesError && !references ? <DialogBody><div className="border border-[var(--tone-danger-bd)] bg-[var(--tone-danger-bg)] p-4"><div className="flex gap-3"><TriangleAlert className="mt-0.5 size-4 shrink-0 text-[var(--tone-danger)]" /><p className="text-sm text-[var(--tone-danger)]">{referencesError}</p></div><Button className="mt-3" onClick={onRetryReferences} type="button" variant="secondary">Try again</Button></div></DialogBody> : !references?.active_term ? <DialogBody><p className="text-sm leading-6 text-[var(--text-muted)]">An active academic term is required before a learning space can be created.</p></DialogBody> : <form onSubmit={submit}><DialogBody className="space-y-5">
        <div><Label>Academic term</Label><p className="mt-2 text-sm font-medium text-[var(--text-strong)]">{references.active_term.name}</p><p className="mt-1 text-xs text-[var(--text-muted)]">{references.active_term.academic_year_name}</p></div>
        <div><Label htmlFor="learning-assignment">Teaching assignment</Label><SearchableSelect allowClear={false} className="mt-1.5" id="learning-assignment" onChange={setAssignmentId} options={references.assignments.map((item) => ({ id: item.id, value: `${item.subject_name} · ${item.class_group_name}`, label: item.teacher_name, description: item.academic_year_name }))} placeholder="Choose a teaching assignment" value={assignmentId} />{references.assignments.length === 0 ? <p className="mt-2 text-xs text-[var(--tone-danger)]">No active teaching assignments are available.</p> : null}</div>
        <div><Label htmlFor="learning-title">Title</Label><Input className="mt-1.5" data-autofocus="true" id="learning-title" maxLength={200} onChange={(event) => setTitle(event.target.value)} required value={title} /></div>
        <div><Label htmlFor="learning-summary">Summary</Label><Textarea className="mt-1.5 min-h-32" id="learning-summary" maxLength={4000} onChange={(event) => setSummary(event.target.value)} value={summary} /></div>
      </DialogBody><DialogFooter><Button disabled={saving} onClick={requestClose} type="button" variant="secondary">Cancel</Button><Button disabled={saving || !assignmentId || !title.trim()} type="submit">{saving ? <><Loader2 className="size-4 animate-spin" />Creating…</> : "Create space"}</Button></DialogFooter></form>}
    </>}
  </GuardedDrawer>;
}

export const StatusBadge = LearningStatusBadge;
export { formatLearningDate as dateTime };
export function label(value: string) { return value.replace(/_/g, " ").replace(/^./, (letter) => letter.toUpperCase()); }

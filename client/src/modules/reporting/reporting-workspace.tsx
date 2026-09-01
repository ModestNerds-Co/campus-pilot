import { useCallback, useEffect, useMemo, useState } from "react";
import { Link, useNavigate } from "@tanstack/react-router";
import { BarChart3, FilePlus2, Loader2, Pencil, Plus, Search, Settings2, Trash2 } from "lucide-react";
import toast from "react-hot-toast";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { ConfirmDrawer } from "@/components/ui/confirm-drawer";
import {
  Table, TableControlsBar, TableControlsPagination, TableEmpty, TableError, TableLoading,
  TableScroll, TableWrap, TBody, TD, TH, THead, TR,
} from "@/components/ui/data-table";
import { DialogBody, DialogFooter, DialogHeader, DialogShell } from "@/components/ui/dialog";
import { Input, Label, Select, Textarea } from "@/components/ui/input";
import { usePageChrome } from "@/modules/admin/layouts/page-chrome";
import { useAuthStore } from "@/stores/auth-store";

import { reportingService, responseMessage } from "./service";
import type {
  AcademicReportBatchStatus, GradingBandInput, GradingScheme, PaginationMeta,
  ReportBatchSummary, ReportingReferenceData, ReportingSource,
} from "./types";

type ReportingTab = "reports" | "grading";
type SchemeAction = { kind: "retire" | "delete"; scheme: GradingScheme } | null;

export function ReportingWorkspace() {
  const navigate = useNavigate();
  const permissions = useAuthStore((state) => state.user?.permissions ?? []);
  const canManage = permissions.includes("*") || permissions.includes("academics:manage");
  const canDelete = permissions.includes("*") || permissions.includes("academics:delete");
  const [references, setReferences] = useState<ReportingReferenceData | null>(null);
  const [schemes, setSchemes] = useState<GradingScheme[]>([]);
  const [reports, setReports] = useState<ReportBatchSummary[]>([]);
  const [pagination, setPagination] = useState<PaginationMeta | null>(null);
  const [page, setPage] = useState(1);
  const [status, setStatus] = useState<"all" | AcademicReportBatchStatus>("all");
  const [query, setQuery] = useState("");
  const [tab, setTab] = useState<ReportingTab>("reports");
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [generateOpen, setGenerateOpen] = useState(false);
  const [schemeDrawer, setSchemeDrawer] = useState<GradingScheme | null | undefined>(undefined);
  const [schemeAction, setSchemeAction] = useState<SchemeAction>(null);
  const [actionPending, setActionPending] = useState(false);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const [referenceResponse, reportResponse, schemeResponse] = await Promise.all([
        reportingService.references(),
        reportingService.listReportBatches({ page, per_page: 25, status: status === "all" ? undefined : status }),
        canManage ? reportingService.listGradingSchemes() : Promise.resolve(null),
      ]);
      if (!referenceResponse.success || !referenceResponse.data) throw new Error(responseMessage(referenceResponse, "Reporting could not be loaded"));
      if (!reportResponse.success || !reportResponse.data) throw new Error(responseMessage(reportResponse, "Academic reports could not be loaded"));
      if (schemeResponse && (!schemeResponse.success || !schemeResponse.data)) throw new Error(responseMessage(schemeResponse, "Grading schemes could not be loaded"));
      setReferences(referenceResponse.data);
      setReports(reportResponse.data.report_batches);
      setPagination(reportResponse.pagination);
      setSchemes(schemeResponse?.data ?? referenceResponse.data.grading_schemes);
    } catch (loadError) {
      setError(loadError instanceof Error ? loadError.message : "Reporting could not be loaded");
    } finally {
      setLoading(false);
    }
  }, [canManage, page, status]);

  useEffect(() => { void load(); }, [load]);

  const filtered = useMemo(() => reports.filter((report) => !query.trim() || [report.assessment_cycle_name, report.class_group_name, report.academic_term_name, report.academic_year_name, report.grading_scheme_name].some((value) => value.toLowerCase().includes(query.trim().toLowerCase()))), [query, reports]);

  usePageChrome("Progress & reporting", <div className="flex flex-wrap items-center gap-2">
    {canManage ? <Button onClick={() => setSchemeDrawer(null)} variant="secondary"><Settings2 className="size-4" />New grading scheme</Button> : null}
    {canManage ? <Button disabled={!references?.sources.length || !references?.grading_schemes.length} onClick={() => setGenerateOpen(true)}><FilePlus2 className="size-4" />Generate reports</Button> : null}
  </div>);

  const runSchemeAction = async () => {
    if (!schemeAction || actionPending) return;
    setActionPending(true);
    try {
      const response = schemeAction.kind === "retire"
        ? await reportingService.retireGradingScheme(schemeAction.scheme.id, schemeAction.scheme.version)
        : await reportingService.deleteGradingScheme(schemeAction.scheme.id, schemeAction.scheme.version);
      if (!response.success) throw new Error(responseMessage(response, "Grading scheme could not be updated"));
      toast.success(schemeAction.kind === "retire" ? "Grading scheme retired" : "Grading scheme deleted");
      setSchemeAction(null);
      await load();
    } catch (actionError) {
      toast.error(actionError instanceof Error ? actionError.message : "Grading scheme could not be updated");
    } finally {
      setActionPending(false);
    }
  };

  return <div className="space-y-6">
    <p className="text-sm text-[var(--text-muted)]">Prepare report cards from published marks and submitted attendance.</p>

    {canManage ? <div className="flex gap-1 border-b border-[var(--border)]" role="tablist">
      <TabButton active={tab === "reports"} onClick={() => setTab("reports")}>Reports</TabButton>
      <TabButton active={tab === "grading"} onClick={() => setTab("grading")}>Grading schemes</TabButton>
    </div> : null}

    {tab === "reports" ? <>
      <TableControlsBar>
        <div className="relative min-w-0 flex-1 sm:min-w-64 sm:max-w-md"><Search className="pointer-events-none absolute left-3 top-1/2 size-4 -translate-y-1/2 text-[var(--text-subtle)]" /><Input aria-label="Search academic reports" className="pl-9" onChange={(event) => setQuery(event.target.value)} placeholder="Search cycle, class, term, or scheme" value={query} /></div>
        <Select aria-label="Report status filter" className="sm:w-44" onChange={(event) => { setPage(1); setStatus(event.target.value as typeof status); }} value={status}><option value="all">All statuses</option><option value="draft">Draft</option><option value="reviewed">Reviewed</option><option value="published">Published</option></Select>
        {pagination && pagination.total_pages > 1 ? <TableControlsPagination onNext={() => setPage((current) => current + 1)} onPrevious={() => setPage((current) => current - 1)} page={page} totalPages={pagination.total_pages} /> : null}
      </TableControlsBar>
      <TableWrap>
        {loading ? <TableLoading columns={7} label="Loading academic reports…" /> : error ? <TableError description={error} onRetry={() => void load()} /> : filtered.length === 0 ? <TableEmpty description={reports.length === 0 ? reportEmptyDescription(references, canManage) : "Change the current filters."} icon={<BarChart3 />} title={reports.length === 0 ? "No academic reports yet" : "No reports match these filters"} /> : <TableScroll><Table className="min-w-[980px]"><THead><tr><TH>Reporting period</TH><TH>Class</TH><TH>Grading</TH><TH>Learners</TH><TH>Results</TH><TH>Status</TH><TH className="w-28">Action</TH></tr></THead><TBody>
          {filtered.map((report) => <TR key={report.id}>
            <TD><p className="font-medium text-[var(--text-strong)]">{report.assessment_cycle_name}</p><p className="mt-1 text-xs text-[var(--text-muted)]">{report.academic_term_name} · {report.academic_year_name}</p></TD>
            <TD className="text-[var(--text-body)]">{report.class_group_name}</TD>
            <TD><p className="text-[var(--text-body)]">{report.grading_scheme_name}</p><p className="mt-1 text-xs text-[var(--text-muted)]">Version {report.grading_scheme_version}</p></TD>
            <TD className="font-tabular text-[var(--text-muted)]">{report.learner_count}</TD>
            <TD><p className="font-tabular text-[var(--text-body)]">{report.graded_subject_count} graded</p>{report.incomplete_subject_count > 0 ? <p className="mt-1 text-xs text-[var(--tone-danger)]">{report.incomplete_subject_count} incomplete</p> : <p className="mt-1 text-xs text-[var(--text-muted)]">Complete</p>}</TD>
            <TD><Badge tone={reportStatusTone(report.status)}>{displayValue(report.status)}</Badge></TD>
            <TD><Link className="text-sm font-semibold text-[var(--brand-strong)] hover:underline" params={{ reportBatchId: report.id }} to="/modules/academics/reporting/report-batches/$reportBatchId">Open</Link></TD>
          </TR>)}
        </TBody></Table></TableScroll>}
      </TableWrap>
    </> : <GradingSchemesTable canDelete={canDelete} loading={loading} onDelete={(scheme) => setSchemeAction({ kind: "delete", scheme })} onEdit={(scheme) => setSchemeDrawer(scheme)} onRetire={(scheme) => setSchemeAction({ kind: "retire", scheme })} schemes={schemes} />}

    <GenerateReportDrawer onClose={() => setGenerateOpen(false)} onCreated={(id) => { setGenerateOpen(false); void navigate({ to: "/modules/academics/reporting/report-batches/$reportBatchId", params: { reportBatchId: id } }); }} open={generateOpen} references={references} />
    <GradingSchemeDrawer onClose={() => setSchemeDrawer(undefined)} onSaved={() => { setSchemeDrawer(undefined); void load(); }} open={schemeDrawer !== undefined} scheme={schemeDrawer ?? null} />
    <ConfirmDrawer confirmLabel={schemeAction?.kind === "retire" ? "Retire grading scheme" : "Delete grading scheme"} description={schemeAction?.kind === "retire" ? `Retire ${schemeAction.scheme.name}? Existing reports retain their grading snapshot.` : `Delete ${schemeAction?.scheme.name ?? "this grading scheme"}? Only unused schemes can be deleted.`} isPending={actionPending} onClose={() => setSchemeAction(null)} onConfirm={() => void runSchemeAction()} open={schemeAction !== null} title={schemeAction?.kind === "retire" ? "Retire grading scheme?" : "Delete grading scheme?"} />
  </div>;
}

function GradingSchemesTable({ canDelete, loading, onDelete, onEdit, onRetire, schemes }: { canDelete: boolean; loading: boolean; onDelete: (scheme: GradingScheme) => void; onEdit: (scheme: GradingScheme) => void; onRetire: (scheme: GradingScheme) => void; schemes: GradingScheme[] }) {
  return <TableWrap>{loading ? <TableLoading columns={5} label="Loading grading schemes…" /> : schemes.length === 0 ? <TableEmpty description="Create a grading scheme before generating reports." icon={<Settings2 />} title="No grading schemes yet" /> : <TableScroll><Table className="min-w-[760px]"><THead><tr><TH>Scheme</TH><TH>Bands</TH><TH>Default</TH><TH>Status</TH><TH className="text-right">Actions</TH></tr></THead><TBody>
    {schemes.map((scheme) => <TR key={scheme.id}><TD><p className="font-medium text-[var(--text-strong)]">{scheme.name}</p><p className="mt-1 max-w-md text-xs text-[var(--text-muted)]">{scheme.description || `Version ${scheme.version}`}</p></TD><TD className="font-tabular text-[var(--text-muted)]">{scheme.bands.length}</TD><TD>{scheme.is_default ? <Badge tone="brand">Default</Badge> : <span className="text-[var(--text-subtle)]">—</span>}</TD><TD><Badge tone={scheme.status === "active" ? "success" : "neutral"}>{displayValue(scheme.status)}</Badge></TD><TD className="text-right"><div className="inline-flex items-center gap-1"><Button aria-label={`Edit ${scheme.name}`} disabled={scheme.status !== "active"} onClick={() => onEdit(scheme)} size="icon-sm" variant="ghost"><Pencil className="size-4" /></Button>{scheme.status === "active" ? <Button onClick={() => onRetire(scheme)} size="sm" variant="secondary">Retire</Button> : null}{canDelete ? <Button aria-label={`Delete ${scheme.name}`} onClick={() => onDelete(scheme)} size="icon-sm" variant="ghost"><Trash2 className="size-4" /></Button> : null}</div></TD></TR>)}
  </TBody></Table></TableScroll>}</TableWrap>;
}

function GenerateReportDrawer({ onClose, onCreated, open, references }: { onClose: () => void; onCreated: (id: string) => void; open: boolean; references: ReportingReferenceData | null }) {
  const [sourceKey, setSourceKey] = useState("");
  const [schemeId, setSchemeId] = useState("");
  const [saving, setSaving] = useState(false);
  useEffect(() => { if (open) { setSourceKey(sourceValue(references?.sources[0])); setSchemeId(references?.grading_schemes.find((scheme) => scheme.is_default)?.id ?? references?.grading_schemes[0]?.id ?? ""); } }, [open, references]);
  const source = references?.sources.find((item) => sourceValue(item) === sourceKey);
  const submit = async (event: React.FormEvent) => {
    event.preventDefault();
    if (!source || !schemeId || saving) return;
    setSaving(true);
    try {
      const response = await reportingService.generateReportBatch({ assessment_cycle_id: source.assessment_cycle_id, class_group_id: source.class_group_id, grading_scheme_id: schemeId, idempotency_key: crypto.randomUUID() });
      if (!response.success || !response.data) throw new Error(responseMessage(response, "Academic reports could not be generated"));
      toast.success("Academic reports generated");
      onCreated(response.data.id);
    } catch (saveError) { toast.error(saveError instanceof Error ? saveError.message : "Academic reports could not be generated"); } finally { setSaving(false); }
  };
  return <DialogShell onClose={saving ? () => undefined : onClose} open={open}><DialogHeader onClose={saving ? undefined : onClose} title="Generate academic reports" /><form onSubmit={submit}><DialogBody className="space-y-5">
    <div><Label htmlFor="reporting-source">Assessment cycle and class</Label><Select className="mt-1.5" data-autofocus="true" id="reporting-source" onChange={(event) => setSourceKey(event.target.value)} required value={sourceKey}><option value="">Choose a report-ready class</option>{references?.sources.map((item) => <option key={sourceValue(item)} value={sourceValue(item)}>{item.assessment_cycle_name} · {item.class_group_name}</option>)}</Select>{source ? <p className="mt-2 text-xs leading-5 text-[var(--text-muted)]">{source.academic_term_name} · {source.published_sheet_count} published mark sheets</p> : null}</div>
    <div><Label htmlFor="reporting-scheme">Grading scheme</Label><Select className="mt-1.5" id="reporting-scheme" onChange={(event) => setSchemeId(event.target.value)} required value={schemeId}><option value="">Choose a grading scheme</option>{references?.grading_schemes.map((scheme) => <option key={scheme.id} value={scheme.id}>{scheme.name}{scheme.is_default ? " · Default" : ""}</option>)}</Select></div>
    <p className="border border-[var(--border)] bg-[var(--surface-muted)] p-4 text-sm leading-6 text-[var(--text-muted)]">The report snapshot uses published Gradebook marks, the class roster at term end, and submitted Attendance registers for the term.</p>
  </DialogBody><DialogFooter><Button disabled={saving} onClick={onClose} type="button" variant="secondary">Cancel</Button><Button disabled={saving || !source || !schemeId} type="submit">{saving ? <Loader2 className="size-4 animate-spin" /> : null}{saving ? "Generating…" : "Generate reports"}</Button></DialogFooter></form></DialogShell>;
}

type BandDraft = { code: string; label: string; minimum: string; isPass: boolean };
function GradingSchemeDrawer({ onClose, onSaved, open, scheme }: { onClose: () => void; onSaved: () => void; open: boolean; scheme: GradingScheme | null }) {
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [isDefault, setIsDefault] = useState(false);
  const [bands, setBands] = useState<BandDraft[]>(emptyBands());
  const [saving, setSaving] = useState(false);
  useEffect(() => { if (!open) return; setName(scheme?.name ?? ""); setDescription(scheme?.description ?? ""); setIsDefault(scheme?.is_default ?? false); setBands(scheme ? scheme.bands.map((band) => ({ code: band.code, label: band.label, minimum: formatBasisPointsInput(band.minimum_basis_points), isPass: band.is_pass })) : emptyBands()); }, [open, scheme]);
  const submit = async (event: React.FormEvent) => {
    event.preventDefault();
    const parsed = parseBands(bands);
    if (!parsed) { toast.error("Enter at least two unique grade bands, including a 0% boundary"); return; }
    setSaving(true);
    try {
      const payload = { name: name.trim(), description: description.trim() || null, is_default: isDefault, bands: parsed };
      const response = scheme ? await reportingService.updateGradingScheme(scheme.id, { expected_version: scheme.version, ...payload }) : await reportingService.createGradingScheme(payload);
      if (!response.success || !response.data) throw new Error(responseMessage(response, "Grading scheme could not be saved"));
      toast.success(scheme ? "Grading scheme updated" : "Grading scheme created");
      onSaved();
    } catch (saveError) { toast.error(saveError instanceof Error ? saveError.message : "Grading scheme could not be saved"); } finally { setSaving(false); }
  };
  return <DialogShell onClose={saving ? () => undefined : onClose} open={open}><DialogHeader onClose={saving ? undefined : onClose} title={scheme ? "Edit grading scheme" : "New grading scheme"} /><form onSubmit={submit}><DialogBody className="space-y-6">
    <div><Label htmlFor="grading-scheme-name">Name</Label><Input className="mt-1.5" data-autofocus="true" id="grading-scheme-name" maxLength={150} onChange={(event) => setName(event.target.value)} required value={name} /></div>
    <div><Label htmlFor="grading-scheme-description">Description</Label><Textarea className="mt-1.5" id="grading-scheme-description" maxLength={1000} onChange={(event) => setDescription(event.target.value)} value={description} /></div>
    <label className="flex items-start gap-3 border border-[var(--border)] bg-[var(--surface-muted)] p-4"><input checked={isDefault} className="mt-0.5 size-4 accent-[var(--brand-strong)]" onChange={(event) => setIsDefault(event.target.checked)} type="checkbox" /><span><span className="block text-sm font-medium text-[var(--text-strong)]">Default scheme</span><span className="mt-1 block text-xs leading-5 text-[var(--text-muted)]">Preselect this scheme when generating reports.</span></span></label>
    <section><div className="flex items-center justify-between gap-4"><div><h3 className="font-semibold text-[var(--text-strong)]">Grade bands</h3><p className="mt-1 text-xs text-[var(--text-muted)]">Each result uses the highest matching minimum percentage.</p></div><Button onClick={() => setBands((current) => [...current, { code: "", label: "", minimum: "", isPass: false }])} size="sm" type="button" variant="secondary"><Plus className="size-4" />Add band</Button></div><div className="mt-4 space-y-3">{bands.map((band, index) => <div className="grid gap-3 border border-[var(--border)] p-4 sm:grid-cols-[0.7fr_1.2fr_0.8fr_auto]" key={index}><div><Label htmlFor={`band-code-${index}`}>Code</Label><Input className="mt-1.5" id={`band-code-${index}`} maxLength={30} onChange={(event) => updateBand(setBands, index, { code: event.target.value })} required value={band.code} /></div><div><Label htmlFor={`band-label-${index}`}>Label</Label><Input className="mt-1.5" id={`band-label-${index}`} maxLength={100} onChange={(event) => updateBand(setBands, index, { label: event.target.value })} required value={band.label} /></div><div><Label htmlFor={`band-minimum-${index}`}>Minimum %</Label><Input className="mt-1.5" id={`band-minimum-${index}`} inputMode="decimal" max="100" min="0" onChange={(event) => updateBand(setBands, index, { minimum: event.target.value })} required step="0.01" type="number" value={band.minimum} /></div><div className="flex items-end justify-between gap-3 sm:flex-col sm:items-center sm:justify-end"><label className="flex items-center gap-2 pb-2 text-xs text-[var(--text-muted)]"><input checked={band.isPass} className="size-4 accent-[var(--brand-strong)]" onChange={(event) => updateBand(setBands, index, { isPass: event.target.checked })} type="checkbox" />Pass</label><Button aria-label={`Remove band ${index + 1}`} disabled={bands.length <= 2} onClick={() => setBands((current) => current.filter((_, itemIndex) => itemIndex !== index))} size="icon-sm" type="button" variant="ghost"><Trash2 className="size-4" /></Button></div></div>)}</div></section>
  </DialogBody><DialogFooter><Button disabled={saving} onClick={onClose} type="button" variant="secondary">Cancel</Button><Button disabled={saving || !name.trim()} type="submit">{saving ? <Loader2 className="size-4 animate-spin" /> : null}{saving ? "Saving…" : "Save grading scheme"}</Button></DialogFooter></form></DialogShell>;
}

function TabButton({ active, children, onClick }: { active: boolean; children: React.ReactNode; onClick: () => void }) { return <button aria-selected={active} className={`border-b-2 px-4 py-3 text-sm font-medium ${active ? "border-[var(--brand-strong)] text-[var(--brand-strong)]" : "border-transparent text-[var(--text-muted)] hover:text-[var(--text-strong)]"}`} onClick={onClick} role="tab" type="button">{children}</button>; }
function reportEmptyDescription(references: ReportingReferenceData | null, canManage: boolean) { if (!canManage) return "No reports are available to this account."; if (!references?.grading_schemes.length) return "Create a grading scheme first."; if (!references.sources.length) return "Close an assessment cycle after publishing all mark sheets."; return "Generate the first report batch."; }
function sourceValue(source?: ReportingSource) { return source ? `${source.assessment_cycle_id}:${source.class_group_id}` : ""; }
function reportStatusTone(status: AcademicReportBatchStatus): "warning" | "info" | "success" { return status === "published" ? "success" : status === "reviewed" ? "info" : "warning"; }
function displayValue(value: string) { return value.replace(/_/g, " ").replace(/^./, (letter) => letter.toUpperCase()); }
function formatBasisPointsInput(value: number) { return (value / 100).toFixed(value % 100 === 0 ? 0 : 2); }
function emptyBands(): BandDraft[] { return [{ code: "", label: "", minimum: "0", isPass: false }, { code: "", label: "", minimum: "", isPass: false }]; }
function updateBand(setBands: React.Dispatch<React.SetStateAction<BandDraft[]>>, index: number, patch: Partial<BandDraft>) { setBands((current) => current.map((band, itemIndex) => itemIndex === index ? { ...band, ...patch } : band)); }
function parseBands(bands: BandDraft[]): GradingBandInput[] | null { const parsed = bands.map((band) => ({ code: band.code.trim(), label: band.label.trim(), minimum_basis_points: Math.round(Number(band.minimum) * 100), is_pass: band.isPass })); const boundaries = new Set(parsed.map((band) => band.minimum_basis_points)); const codes = new Set(parsed.map((band) => band.code.toLowerCase())); if (parsed.length < 2 || parsed.some((band) => !band.code || !band.label || !Number.isInteger(band.minimum_basis_points) || band.minimum_basis_points < 0 || band.minimum_basis_points > 10000) || !boundaries.has(0) || boundaries.size !== parsed.length || codes.size !== parsed.length) return null; return parsed; }

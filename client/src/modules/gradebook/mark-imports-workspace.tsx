/**
 * Full-page staged CSV/XLSX mark import for one scoped Gradebook sheet.
 * The browser receives normalized preview rows only; source bytes remain on the API.
 */
import { useCallback, useEffect, useState } from "react";
import { Link } from "@tanstack/react-router";
import { ArrowLeft, CheckCircle2, FileSpreadsheet, Loader2, Upload } from "lucide-react";
import toast from "react-hot-toast";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Table,
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
import { DialogBody, DialogFooter, DialogHeader, DialogShell } from "@/components/ui/dialog";
import { Input, Label, Select } from "@/components/ui/input";
import { usePageChrome } from "@/modules/admin/layouts/page-chrome";
import { useAuthStore } from "@/stores/auth-store";

import { gradebookService, responseMessage } from "./service";
import type {
  GradebookMarkImportDecimalSeparator,
  GradebookMarkImportMapping,
  GradebookMarkImportPreview,
  GradebookMarkImportPreviewRow,
  GradebookMarkImportRecord,
  GradebookSheet,
} from "./types";

const MAPPING_FIELDS = [
  ["learner_number", "Learner number", true],
  ["mark", "Mark", false],
  ["status", "Status", false],
  ["note", "Note", false],
] as const;

const HEADER_ALIASES: Record<string, string[]> = {
  learner_number: ["learnernumber", "studentnumber", "studentid", "admissionnumber", "admissionno"],
  mark: ["mark", "marks", "score", "result"],
  status: ["status", "markstatus", "resultstatus"],
  note: ["note", "notes", "comment", "comments", "remark", "remarks"],
};

export function MarkImportsWorkspace({ markSheetId }: { markSheetId: string }) {
  const permissions = useAuthStore((state) => state.user?.permissions ?? []);
  const canImport = permissions.includes("*") || permissions.includes("academics:teach") || permissions.includes("academics:manage");
  const [sheet, setSheet] = useState<GradebookSheet | null>(null);
  const [records, setRecords] = useState<GradebookMarkImportRecord[]>([]);
  const [selected, setSelected] = useState<GradebookMarkImportRecord | null>(null);
  const [preview, setPreview] = useState<GradebookMarkImportPreview | null>(null);
  const [mapping, setMapping] = useState<Record<string, string>>({});
  const [separator, setSeparator] = useState<GradebookMarkImportDecimalSeparator>("dot");
  const [file, setFile] = useState<File | null>(null);
  const [previewPage, setPreviewPage] = useState(1);
  const [loading, setLoading] = useState(true);
  const [previewLoading, setPreviewLoading] = useState(false);
  const [working, setWorking] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [confirmOpen, setConfirmOpen] = useState(false);

  usePageChrome("Import marks");

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const [sheetResponse, importsResponse] = await Promise.all([
        gradebookService.readMarkSheet(markSheetId),
        gradebookService.listMarkImports(markSheetId, { page: 1, per_page: 100 }),
      ]);
      if (!sheetResponse.success || !sheetResponse.data) throw new Error(responseMessage(sheetResponse, "Mark sheet could not be loaded"));
      if (!importsResponse.success || !importsResponse.data) throw new Error(responseMessage(importsResponse, "Mark imports could not be loaded"));
      setSheet(sheetResponse.data);
      setRecords(importsResponse.data.imports);
      setSelected((current) => importsResponse.data?.imports.find((record) => record.id === current?.id) ?? importsResponse.data?.imports[0] ?? null);
    } catch (loadError) {
      setError(loadError instanceof Error ? loadError.message : "Mark imports could not be loaded");
    } finally {
      setLoading(false);
    }
  }, [markSheetId]);

  useEffect(() => { void load(); }, [load]);

  useEffect(() => {
    setPreview(null);
    setPreviewPage(1);
    if (!selected) { setMapping({}); return; }
    setMapping(autoMapping(selected.source_headers));
    setSeparator("dot");
    if (!selected.latest_preview_id) return;
    setPreviewLoading(true);
    void gradebookService.readMarkImportPreview(markSheetId, selected.id, { page: 1, per_page: 100 })
      .then((response) => {
        if (!response.success || !response.data) throw new Error(responseMessage(response, "Import preview could not be loaded"));
        setPreview(response.data);
        setMapping(response.data.mapping.columns);
        setSeparator(response.data.mapping.decimal_separator);
      })
      .catch((previewError: unknown) => toast.error(messageOf(previewError, "Import preview could not be loaded")))
      .finally(() => setPreviewLoading(false));
  }, [markSheetId, selected]);

  const upload = async (event: React.FormEvent) => {
    event.preventDefault();
    if (!sheet || sheet.status !== "draft" || !canImport || !file || working) return;
    setWorking(true);
    try {
      const response = await gradebookService.uploadMarkImport(markSheetId, file);
      if (!response.success || !response.data) throw new Error(responseMessage(response, "Mark file could not be uploaded"));
      setRecords((current) => [response.data!, ...current]);
      setSelected(response.data);
      setFile(null);
      toast.success("Mark file uploaded");
    } catch (uploadError) {
      toast.error(messageOf(uploadError, "Mark file could not be uploaded"));
    } finally {
      setWorking(false);
    }
  };

  const createPreview = async () => {
    if (!sheet || !selected || selected.status === "committed" || !canImport || working) return;
    const columns = Object.fromEntries(Object.entries(mapping).filter(([, header]) => header));
    if (!columns.learner_number || (!columns.mark && !columns.status)) {
      toast.error("Map learner number and at least one of mark or status");
      return;
    }
    setWorking(true);
    try {
      const payload: GradebookMarkImportMapping = { columns, decimal_separator: separator, expected_sheet_version: sheet.version };
      const response = await gradebookService.createMarkImportPreview(markSheetId, selected.id, payload);
      if (!response.success || !response.data) throw new Error(responseMessage(response, "Mark mapping could not be previewed"));
      setPreview(response.data);
      setPreviewPage(1);
      setSelected((current) => current ? { ...current, latest_preview_id: response.data!.id, mapping_version: response.data!.mapping_version, ready_rows: response.data!.ready_rows, invalid_rows: response.data!.invalid_rows, duplicate_rows: response.data!.duplicate_rows, status: "preview_ready" } : current);
      setRecords((current) => current.map((record) => record.id === selected.id ? { ...record, latest_preview_id: response.data!.id, mapping_version: response.data!.mapping_version, ready_rows: response.data!.ready_rows, invalid_rows: response.data!.invalid_rows, duplicate_rows: response.data!.duplicate_rows, status: "preview_ready" } : record));
      toast.success("Preview ready");
    } catch (previewError) {
      toast.error(messageOf(previewError, "Mark mapping could not be previewed"));
    } finally {
      setWorking(false);
    }
  };

  const openPreviewPage = async (page: number) => {
    if (!selected || previewLoading) return;
    setPreviewLoading(true);
    try {
      const response = await gradebookService.readMarkImportPreview(markSheetId, selected.id, { page, per_page: 100 });
      if (!response.success || !response.data) throw new Error(responseMessage(response, "Import preview could not be loaded"));
      setPreview(response.data);
      setPreviewPage(page);
    } catch (previewError) {
      toast.error(messageOf(previewError, "Import preview could not be loaded"));
    } finally {
      setPreviewLoading(false);
    }
  };

  const commit = async () => {
    if (!sheet || !selected || !preview || preview.ready_rows === 0 || working) return;
    setWorking(true);
    try {
      const response = await gradebookService.commitMarkImport(markSheetId, selected.id, preview.id);
      if (!response.success || !response.data) throw new Error(responseMessage(response, "Marks could not be imported"));
      toast.success(`${response.data.updated_rows} marks imported`);
      setConfirmOpen(false);
      await load();
    } catch (commitError) {
      toast.error(messageOf(commitError, "Marks could not be imported"));
    } finally {
      setWorking(false);
    }
  };

  const previewPages = Math.max(1, Math.ceil((preview?.total_rows ?? 0) / 100));
  const mappingLocked = !canImport || !sheet || sheet.status !== "draft" || selected?.status === "committed";
  const selectedCommitted = selected?.status === "committed";

  if (loading) return <div aria-label="Loading mark imports" className="flex min-h-64 items-center justify-center border border-[var(--border)] bg-[var(--surface)]" role="status"><Loader2 className="size-6 animate-spin text-[var(--brand-strong)]" /></div>;
  if (error || !sheet) return <TableWrap><TableError description={error ?? "Mark sheet could not be loaded"} onRetry={() => void load()} /></TableWrap>;

  return <div className="space-y-6">
    <Link className="inline-flex items-center gap-2 text-sm font-medium text-[var(--text-muted)] hover:text-[var(--text-strong)]" params={{ markSheetId }} to="/modules/academics/gradebook/mark-sheets/$markSheetId"><ArrowLeft className="size-4" />Mark sheet</Link>

    <section className="border border-[var(--border)] bg-[var(--surface)] p-5 sm:p-6">
      <div className="flex flex-col gap-4 sm:flex-row sm:items-start sm:justify-between">
        <div><p className="text-xs font-semibold uppercase tracking-[0.14em] text-[var(--brand-strong)]">{sheet.subject_name} · {sheet.class_group_name}</p><h1 className="mt-2 text-2xl font-semibold tracking-[-0.03em] text-[var(--text-strong)]">{sheet.assessment_component_name}</h1><p className="mt-2 text-sm text-[var(--text-muted)]">Match learner numbers in a CSV or XLSX file to this sheet's {sheet.marks.length} learners.</p></div>
        <Badge tone={sheet.status === "draft" ? "warning" : sheet.status === "submitted" ? "info" : "success"}>{displayStatus(sheet.status)}</Badge>
      </div>
    </section>

    {sheet.status !== "draft" ? <section className="border border-[var(--tone-warn-bd)] bg-[var(--badge-warning-bg)] p-4 text-sm text-[var(--badge-warning-text)]">This mark sheet is {sheet.status}. Existing import evidence remains available, but new previews and commits require a draft sheet.</section> : null}

    {sheet.status === "draft" && canImport ? <section className="border border-[var(--border)] bg-[var(--surface)] p-5 sm:p-6">
      <div><h2 className="text-base font-semibold text-[var(--text-strong)]">Upload file</h2><p className="mt-1 text-sm text-[var(--text-muted)]">Maximum 5 MB and 5,000 data rows. Include learner number plus mark or status.</p></div>
      <form className="mt-5 flex flex-col gap-3 sm:flex-row sm:items-end" onSubmit={upload}>
        <div className="min-w-0 flex-1"><Label htmlFor="gradebook-mark-import-file">CSV or XLSX file</Label><Input accept=".csv,.xlsx,text/csv,application/vnd.openxmlformats-officedocument.spreadsheetml.sheet" className="mt-1.5 h-auto py-2" id="gradebook-mark-import-file" onChange={(event) => setFile(event.target.files?.[0] ?? null)} type="file" /></div>
        <Button disabled={!file || working} type="submit">{working ? <Loader2 className="size-4 animate-spin" /> : <Upload className="size-4" />}{working ? "Uploading…" : "Upload"}</Button>
      </form>
    </section> : null}

    <section className="space-y-3"><div><h2 className="text-lg font-semibold text-[var(--text-strong)]">Import history</h2><p className="mt-1 text-sm text-[var(--text-muted)]">Open a file to map its columns or review its results.</p></div>
      <TableWrap>{records.length === 0 ? <TableEmpty description="Upload a file to begin." icon={<FileSpreadsheet />} title="No mark imports yet" /> : <TableScroll><Table className="min-w-[760px]"><THead><tr><TH>File</TH><TH>Rows</TH><TH>Preview</TH><TH>Status</TH><TH className="text-right">Action</TH></tr></THead><TBody>{records.map((record) => <TR className={selected?.id === record.id ? "bg-[var(--table-row-hover-bg)]" : undefined} key={record.id}><TD><p className="max-w-[320px] truncate font-medium text-[var(--text-strong)]">{record.file_name}</p><p className="mt-1 text-xs text-[var(--text-muted)]">{formatDateTime(record.created_at)} · {formatBytes(record.source_size_bytes)}</p></TD><TD className="font-tabular">{record.source_row_count}</TD><TD><PreviewTotals record={record} /></TD><TD><Badge tone={importTone(record.status)}>{displayStatus(record.status)}</Badge></TD><TD className="text-right"><Button onClick={() => setSelected(record)} size="sm" variant="secondary">{record.status === "uploaded" ? "Map columns" : "Open"}</Button></TD></TR>)}</TBody></Table></TableScroll>}</TableWrap>
    </section>

    {selected ? <section className="space-y-5 border border-[var(--border)] bg-[var(--surface)] p-5 sm:p-6">
      <div className="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between"><div><h2 className="text-lg font-semibold text-[var(--text-strong)]">{selected.file_name}</h2><p className="mt-1 text-sm text-[var(--text-muted)]">Map destination fields to unique columns from this file.</p></div><Badge tone={importTone(selected.status)}>{displayStatus(selected.status)}</Badge></div>

      {!selectedCommitted ? <div className="grid gap-4 border-y border-[var(--border)] py-5 md:grid-cols-2">
        {MAPPING_FIELDS.map(([key, label, required]) => <div key={key}><Label htmlFor={`mark-import-${key}`}>{label}{required ? " *" : ""}</Label><Select className="mt-1.5" disabled={mappingLocked || working} id={`mark-import-${key}`} onChange={(event) => { setMapping((current) => ({ ...current, [key]: event.target.value })); setPreview(null); }} value={mapping[key] ?? ""}><option value="">Do not import</option>{selected.source_headers.map((header) => <option disabled={Object.entries(mapping).some(([mappedKey, mappedHeader]) => mappedKey !== key && mappedHeader === header)} key={header} value={header}>{header}</option>)}</Select></div>)}
        <div><Label htmlFor="mark-import-decimal">Decimal separator</Label><Select className="mt-1.5" disabled={mappingLocked || working} id="mark-import-decimal" onChange={(event) => { setSeparator(event.target.value as GradebookMarkImportDecimalSeparator); setPreview(null); }} value={separator}><option value="dot">Dot (12.50)</option><option value="comma">Comma (12,50)</option></Select></div>
      </div> : null}

      {!selectedCommitted ? <div className="flex justify-end"><Button disabled={mappingLocked || working} onClick={() => void createPreview()} type="button">{working ? <Loader2 className="size-4 animate-spin" /> : null}{working ? "Checking…" : preview ? "Refresh preview" : "Create preview"}</Button></div> : null}

      {previewLoading ? <TableWrap><TableLoading columns={5} label="Loading preview…" /></TableWrap> : preview ? <PreviewTable page={previewPage} preview={preview} onPage={(page) => void openPreviewPage(page)} pageCount={previewPages} /> : <div className="border border-dashed border-[var(--border)] p-8 text-center text-sm text-[var(--text-muted)]">Create a preview to validate the mapped rows.</div>}

      {preview && !selectedCommitted && sheet.status === "draft" && canImport ? <div className="flex justify-end"><Button disabled={preview.ready_rows === 0 || working} onClick={() => setConfirmOpen(true)} type="button">Import {preview.ready_rows} ready {preview.ready_rows === 1 ? "row" : "rows"}</Button></div> : null}
      {selectedCommitted ? <div className="flex items-start gap-3 border border-[var(--status-success-border)] bg-[var(--status-success-soft)] p-4 text-[var(--status-success-text)]"><CheckCircle2 className="mt-0.5 size-5 shrink-0" /><div><p className="text-sm font-semibold">Import committed</p><p className="mt-1 text-xs">{selected.updated_rows ?? 0} marks updated · {selected.skipped_rows ?? 0} rows skipped.</p></div></div> : null}
    </section> : null}

    <CommitDrawer onClose={() => setConfirmOpen(false)} onConfirm={() => void commit()} open={confirmOpen} pending={working} readyRows={preview?.ready_rows ?? 0} />
  </div>;
}

function PreviewTable({ onPage, page, pageCount, preview }: { onPage: (page: number) => void; page: number; pageCount: number; preview: GradebookMarkImportPreview }) {
  return <div className="space-y-4">
    <div className="grid grid-cols-3 gap-3"><Summary label="Ready" tone="success" value={preview.ready_rows} /><Summary label="Invalid" tone="danger" value={preview.invalid_rows} /><Summary label="Duplicates" tone="warning" value={preview.duplicate_rows} /></div>
    <TableWrap><TableScroll><Table className="min-w-[800px]"><THead><tr><TH>Row</TH><TH>Learner</TH><TH>Status</TH><TH>Mark</TH><TH>Result</TH></tr></THead><TBody>{preview.rows.map((row) => <PreviewRow key={row.id} row={row} />)}</TBody></Table></TableScroll></TableWrap>
    {pageCount > 1 ? <TableControlsPagination onNext={() => onPage(Math.min(pageCount, page + 1))} onPrevious={() => onPage(Math.max(1, page - 1))} page={page} totalPages={pageCount} /> : null}
  </div>;
}

function PreviewRow({ row }: { row: GradebookMarkImportPreviewRow }) {
  const data = row.canonical_data;
  return <TR><TD className="font-tabular">{row.row_number}</TD><TD><p className="font-medium text-[var(--text-strong)]">{data.learner_name ?? "Unresolved learner"}</p><p className="mt-1 text-xs text-[var(--text-muted)]">{data.learner_number ?? "No learner number"}</p></TD><TD>{data.mark_status ? displayStatus(data.mark_status) : "—"}</TD><TD className="font-tabular">{data.marks_awarded_hundredths === undefined || data.marks_awarded_hundredths === null ? "—" : formatHundredths(data.marks_awarded_hundredths)}</TD><TD><Badge tone={rowTone(row.outcome)}>{displayStatus(row.outcome)}</Badge>{row.issues.length ? <p className="mt-2 max-w-md text-xs leading-5 text-[var(--text-muted)]">{row.issues.join(" ")}</p> : null}</TD></TR>;
}

function CommitDrawer({ onClose, onConfirm, open, pending, readyRows }: { onClose: () => void; onConfirm: () => void; open: boolean; pending: boolean; readyRows: number }) {
  return <DialogShell onClose={pending ? () => undefined : onClose} open={open}><DialogHeader onClose={pending ? undefined : onClose} title="Import marks?" /><DialogBody><div className="flex gap-4"><span className="flex size-10 shrink-0 items-center justify-center rounded-[9px] bg-[var(--badge-info-bg)] text-[var(--badge-info-text)]"><FileSpreadsheet className="size-5" /></span><p className="text-sm leading-6 text-[var(--text-muted)]">Update {readyRows} {readyRows === 1 ? "mark" : "marks"} from this frozen preview? Invalid and duplicate rows will be skipped.</p></div></DialogBody><DialogFooter><Button disabled={pending} onClick={onClose} type="button" variant="secondary">Cancel</Button><Button disabled={pending} onClick={onConfirm} type="button">{pending ? <Loader2 className="size-4 animate-spin" /> : null}{pending ? "Importing…" : "Import marks"}</Button></DialogFooter></DialogShell>;
}

function autoMapping(headers: string[]) {
  const normalized = headers.map((header) => [header, normalizeHeader(header)] as const);
  return Object.fromEntries(Object.entries(HEADER_ALIASES).flatMap(([field, aliases]) => {
    const match = normalized.find(([, value]) => aliases.includes(value));
    return match ? [[field, match[0]]] : [];
  }));
}

function normalizeHeader(value: string) { return value.toLowerCase().replace(/[^a-z0-9]/g, ""); }
function formatHundredths(value: number) { return `${Math.floor(value / 100)}.${String(value % 100).padStart(2, "0")}`; }
function displayStatus(value: string) { return value.replace(/_/g, " ").replace(/^./, (letter) => letter.toUpperCase()); }
function formatDateTime(value: string) { return new Intl.DateTimeFormat(undefined, { dateStyle: "medium", timeStyle: "short" }).format(new Date(value)); }
function formatBytes(value: number) { return value < 1024 ? `${value} B` : value < 1024 * 1024 ? `${(value / 1024).toFixed(1)} KB` : `${(value / (1024 * 1024)).toFixed(1)} MB`; }
function messageOf(error: unknown, fallback: string) { return error instanceof Error ? error.message : fallback; }
function importTone(status: GradebookMarkImportRecord["status"]): "neutral" | "warning" | "success" { return status === "committed" ? "success" : status === "preview_ready" ? "warning" : "neutral"; }
function rowTone(outcome: GradebookMarkImportPreviewRow["outcome"]): "success" | "danger" | "warning" { return outcome === "ready" ? "success" : outcome === "invalid" ? "danger" : "warning"; }
function Summary({ label, tone, value }: { label: string; tone: "success" | "danger" | "warning"; value: number }) { return <div className="border border-[var(--border)] bg-[var(--surface-muted)] p-4"><p className="text-xs font-medium uppercase tracking-[0.12em] text-[var(--text-muted)]">{label}</p><p className={`mt-2 font-tabular text-xl font-semibold ${tone === "success" ? "text-[var(--tone-success)]" : tone === "danger" ? "text-[var(--tone-danger)]" : "text-[var(--tone-warn)]"}`}>{value}</p></div>; }
function PreviewTotals({ record }: { record: GradebookMarkImportRecord }) { return record.ready_rows === null ? <span className="text-xs text-[var(--text-muted)]">Not previewed</span> : <span className="text-xs text-[var(--text-muted)]"><span className="font-tabular font-medium text-[var(--text-strong)]">{record.ready_rows}</span> ready · {record.invalid_rows ?? 0} invalid · {record.duplicate_rows ?? 0} duplicate</span>; }

/**
 * Owns the operational SIS CSV/XLSX import workspace.
 * Source parsing and commit authority remain server-owned; this UI only uploads,
 * maps, reviews immutable previews, and explicitly commits ready rows.
 */
import { useCallback, useEffect, useMemo, useState } from "react";
import {
  CheckCircle2,
  FileSpreadsheet,
  Loader2,
  Upload,
} from "lucide-react";
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
import { DialogBody, DialogFooter, DialogHeader, DialogShell } from "@/components/ui/dialog";
import { Input, Label, Select } from "@/components/ui/input";
import { usePageChrome } from "@/modules/admin/layouts/page-chrome";
import { useAuthStore } from "@/stores/auth-store";

import { responseMessage, sisService } from "./service";
import type {
  ImportDateFormat,
  SisImportMapping,
  SisImportPreview,
  SisImportPreviewRow,
  SisImportRecord,
  SisImportTarget,
} from "./types";

const MAPPING_FIELDS = {
  learners: [
    ["learner_number", "Learner number", true],
    ["display_name", "Display name", true],
    ["first_names", "First names", false],
    ["surname", "Surname", false],
    ["date_of_birth", "Date of birth", true],
    ["email", "Email", false],
    ["phone", "Phone", false],
    ["status", "Status", false],
  ],
  guardians: [
    ["display_name", "Display name", true],
    ["first_names", "First names", false],
    ["surname", "Surname", false],
    ["email", "Email", false],
    ["phone", "Phone", false],
    ["status", "Status", false],
  ],
} satisfies Record<SisImportTarget, Array<readonly [string, string, boolean]>>;

const HEADER_ALIASES: Record<string, string[]> = {
  learner_number: ["learnernumber", "studentnumber", "studentid", "admissionnumber", "admissionno"],
  display_name: ["displayname", "name", "fullname", "studentname", "guardianname"],
  first_names: ["firstnames", "firstname", "givenname", "givennames"],
  surname: ["surname", "lastname", "familyname"],
  date_of_birth: ["dateofbirth", "dob", "birthdate"],
  email: ["email", "emailaddress"],
  phone: ["phone", "phonenumber", "mobile", "mobilenumber"],
  status: ["status", "recordstatus"],
};

export function SisImportsWorkspace() {
  const permissions = useAuthStore((state) => state.user?.permissions ?? []);
  const canCreate = permissions.includes("*") || permissions.includes("sis:create");
  const canEdit = permissions.includes("*") || permissions.includes("sis:edit");
  const [records, setRecords] = useState<SisImportRecord[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [page, setPage] = useState(1);
  const [totalPages, setTotalPages] = useState(1);
  const [target, setTarget] = useState<"all" | SisImportTarget>("all");
  const [uploadOpen, setUploadOpen] = useState(false);
  const [selected, setSelected] = useState<SisImportRecord | null>(null);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const response = await sisService.listImports({
        page,
        per_page: 20,
        target: target === "all" ? undefined : target,
      });
      if (!response.success || !response.data) {
        throw new Error(responseMessage(response, "Imports could not be loaded"));
      }
      setRecords(response.data.imports);
      setTotalPages(response.pagination?.total_pages ?? 1);
    } catch (loadError) {
      setError(loadError instanceof Error ? loadError.message : "Imports could not be loaded");
    } finally {
      setLoading(false);
    }
  }, [page, target]);

  useEffect(() => { void load(); }, [load]);

  const action = useMemo(
    () => canCreate ? <Button onClick={() => setUploadOpen(true)}><Upload className="size-4" />New import</Button> : null,
    [canCreate],
  );
  usePageChrome("Data imports", action);

  return (
    <div className="space-y-6">
      <p className="text-sm text-[var(--text-muted)]">Upload learner or guardian records, map the columns, review validation results, then commit the ready rows.</p>
      <TableControlsBar>
        <Select
          aria-label="Import target filter"
          className="sm:w-52"
          onChange={(event) => { setPage(1); setTarget(event.target.value as "all" | SisImportTarget); }}
          value={target}
        >
          <option value="all">All import types</option>
          <option value="learners">Learners</option>
          <option value="guardians">Guardians</option>
        </Select>
        {!loading && records.length > 0 ? (
          <TableControlsPagination
            onNext={() => setPage((value) => Math.min(totalPages, value + 1))}
            onPrevious={() => setPage((value) => Math.max(1, value - 1))}
            page={page}
            totalPages={totalPages}
          />
        ) : null}
      </TableControlsBar>
      <TableWrap>
        {loading ? <TableLoading columns={6} label="Loading imports…" /> : error ? (
          <TableError description={error} onRetry={() => void load()} />
        ) : records.length === 0 ? (
          <TableEmpty description={target === "all" ? "Upload a learner or guardian file." : "No imports match this type."} icon={<FileSpreadsheet />} title="No imports yet" />
        ) : (
          <TableScroll>
            <Table>
              <THead><tr><TH>File</TH><TH>Type</TH><TH>Rows</TH><TH>Preview</TH><TH>Status</TH><TH className="text-right">Action</TH></tr></THead>
              <TBody>
                {records.map((record) => (
                  <TR key={record.id}>
                    <TD><p className="max-w-[280px] truncate font-medium text-[var(--text-strong)]">{record.file_name}</p><p className="mt-1 text-xs text-[var(--text-muted)]">{formatDateTime(record.created_at)} · {formatBytes(record.source_size_bytes)}</p></TD>
                    <TD className="capitalize text-[var(--text-body)]">{record.entity_key}</TD>
                    <TD className="font-tabular text-[var(--text-body)]">{record.source_row_count}</TD>
                    <TD><PreviewTotals record={record} /></TD>
                    <TD><Badge tone={statusTone(record.status)}>{displayStatus(record.status)}</Badge></TD>
                    <TD className="text-right"><Button onClick={() => setSelected(record)} size="sm" variant="secondary">{canEdit && record.status === "uploaded" ? "Map columns" : "Open"}</Button></TD>
                  </TR>
                ))}
              </TBody>
            </Table>
          </TableScroll>
        )}
      </TableWrap>
      <UploadImportDrawer
        canCreate={canCreate}
        onClose={() => setUploadOpen(false)}
        onUploaded={(record) => { setUploadOpen(false); setSelected(record); void load(); }}
        open={canCreate && uploadOpen}
      />
      <ImportReviewDrawer
        canCreate={canCreate}
        canEdit={canEdit}
        onChanged={() => void load()}
        onClose={() => setSelected(null)}
        record={selected}
      />
    </div>
  );
}

function UploadImportDrawer({ canCreate, onClose, onUploaded, open }: { canCreate: boolean; onClose: () => void; onUploaded: (record: SisImportRecord) => void; open: boolean }) {
  const [target, setTarget] = useState<SisImportTarget>("learners");
  const [file, setFile] = useState<File | null>(null);
  const [uploading, setUploading] = useState(false);

  useEffect(() => {
    if (!open) return;
    setTarget("learners");
    setFile(null);
  }, [open]);

  const submit = async (event: React.FormEvent) => {
    event.preventDefault();
    if (!canCreate || !file || uploading) return;
    setUploading(true);
    try {
      const response = await sisService.uploadImport(target, file);
      if (!response.success || !response.data) {
        toast.error(responseMessage(response, "The import could not be uploaded"));
        return;
      }
      toast.success("Import uploaded");
      onUploaded(response.data);
    } catch (error) {
      toast.error(networkMessage(error, "The import could not be uploaded"));
    } finally {
      setUploading(false);
    }
  };

  return (
    <DialogShell onClose={() => !uploading && onClose()} open={open}>
      <DialogHeader onClose={() => !uploading && onClose()} title="New data import" />
      <form onSubmit={submit}>
        <DialogBody className="space-y-6">
          <div><Label htmlFor="import-target">Records</Label><Select className="mt-1.5" id="import-target" onChange={(event) => setTarget(event.target.value as SisImportTarget)} value={target}><option value="learners">Learners</option><option value="guardians">Guardians</option></Select></div>
          <div><Label htmlFor="import-file">CSV or XLSX file</Label><Input accept=".csv,.xlsx,text/csv,application/vnd.openxmlformats-officedocument.spreadsheetml.sheet" className="mt-1.5 h-auto py-2" data-autofocus="true" id="import-file" onChange={(event) => setFile(event.target.files?.[0] ?? null)} required type="file" /><p className="mt-2 text-xs text-[var(--text-muted)]">Maximum 5 MB and 5,000 data rows. The first row must contain unique column names.</p></div>
          {file ? <div className="rounded-[var(--radius-lg)] border border-[var(--border)] bg-[var(--surface-muted)] p-4"><p className="truncate text-sm font-medium text-[var(--text-strong)]">{file.name}</p><p className="mt-1 text-xs text-[var(--text-muted)]">{formatBytes(file.size)}</p></div> : null}
        </DialogBody>
        <DialogFooter><Button disabled={uploading} onClick={onClose} type="button" variant="ghost">Cancel</Button><Button disabled={!file || uploading} type="submit">{uploading ? <><Loader2 className="size-4 animate-spin" />Uploading…</> : "Upload and map"}</Button></DialogFooter>
      </form>
    </DialogShell>
  );
}

function ImportReviewDrawer({ canCreate, canEdit, onChanged, onClose, record }: { canCreate: boolean; canEdit: boolean; onChanged: () => void; onClose: () => void; record: SisImportRecord | null }) {
  const [current, setCurrent] = useState<SisImportRecord | null>(record);
  const [preview, setPreview] = useState<SisImportPreview | null>(null);
  const [mapping, setMapping] = useState<Record<string, string>>({});
  const [dateFormat, setDateFormat] = useState<ImportDateFormat>("yyyy_mm_dd");
  const [mode, setMode] = useState<"mapping" | "preview">("mapping");
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    if (!record) return;
    setCurrent(record);
    setPreview(null);
    setMapping(autoMapping(record.entity_key, record.source_headers));
    setDateFormat("yyyy_mm_dd");
    setMode(record.latest_preview_id ? "preview" : "mapping");
    if (!record.latest_preview_id) return;
    setLoading(true);
    void sisService.getImportPreview(record.id, { page: 1, per_page: 100 }).then((response) => {
      if (response.success && response.data) {
        setPreview(response.data);
        setMapping(response.data.mapping.columns);
        setDateFormat(response.data.mapping.date_format ?? "yyyy_mm_dd");
      } else {
        toast.error(responseMessage(response, "The import preview could not be loaded"));
      }
    }).catch((error: unknown) => {
      toast.error(networkMessage(error, "The import preview could not be loaded"));
    }).finally(() => setLoading(false));
  }, [record]);

  const fields = current ? MAPPING_FIELDS[current.entity_key] : [];
  const validate = async () => {
    if (!canEdit || !current || saving) return;
    setSaving(true);
    try {
      const payload: SisImportMapping = {
        columns: Object.fromEntries(Object.entries(mapping).filter(([, header]) => header)),
        date_format: current.entity_key === "learners" ? dateFormat : null,
      };
      const response = await sisService.createImportPreview(current.id, payload);
      if (!response.success || !response.data) {
        toast.error(responseMessage(response, "The mapping could not be validated"));
        return;
      }
      setPreview(response.data);
      setMode("preview");
      toast.success("Preview ready");
      onChanged();
    } catch (error) {
      toast.error(networkMessage(error, "The mapping could not be validated"));
    } finally {
      setSaving(false);
    }
  };

  const commit = async () => {
    if (!canCreate || !current || !preview || saving) return;
    setSaving(true);
    try {
      const response = await sisService.commitImport(current.id, preview.id);
      if (!response.success || !response.data) {
        toast.error(responseMessage(response, "The import could not be committed"));
        return;
      }
      const refreshed = await sisService.getImport(current.id);
      if (refreshed.success && refreshed.data) setCurrent(refreshed.data);
      toast.success(`${response.data.created_rows} ${current.entity_key} created`);
      onChanged();
    } catch (error) {
      toast.error(networkMessage(error, "The import could not be committed"));
    } finally {
      setSaving(false);
    }
  };

  const isCommitted = current?.status === "committed";
  return (
    <DialogShell onClose={() => !saving && onClose()} open={record !== null} panelClassName="sm:max-w-[780px]">
      <DialogHeader onClose={() => !saving && onClose()} title={current ? `${capitalise(current.entity_key)} import` : "Data import"} />
      <DialogBody className="space-y-6">
        {!current ? null : (
          <>
            <div className="rounded-[var(--radius-lg)] border border-[var(--border)] bg-[var(--surface-muted)] p-4"><div className="flex flex-wrap items-start justify-between gap-3"><div className="min-w-0"><p className="truncate text-sm font-semibold text-[var(--text-strong)]">{current.file_name}</p><p className="mt-1 text-xs text-[var(--text-muted)]">{current.source_row_count} rows · {current.source_headers.length} columns · {current.source_format.toUpperCase()}</p></div><Badge tone={statusTone(current.status)}>{displayStatus(current.status)}</Badge></div></div>
            {loading ? <div className="flex items-center gap-2 py-8 text-sm text-[var(--text-muted)]"><Loader2 className="size-4 animate-spin" />Loading preview…</div> : mode === "mapping" ? (
              <div className="space-y-5">
                <div><h3 className="text-sm font-semibold text-[var(--text-strong)]">Column mapping</h3><p className="mt-1 text-xs leading-5 text-[var(--text-muted)]">Choose the source column for each Campus Pilot field. Unmapped optional fields stay empty.</p></div>
                <div className="space-y-4">
                  {fields.map(([key, label, required]) => (
                    <div className="grid gap-2 sm:grid-cols-[180px_minmax(0,1fr)] sm:items-center" key={key}>
                      <Label htmlFor={`mapping-${key}`}>{label}{required ? <span className="text-[var(--tone-danger)]"> *</span> : null}</Label>
                      <Select disabled={!canEdit} id={`mapping-${key}`} onChange={(event) => setMapping((value) => ({ ...value, [key]: event.target.value }))} value={mapping[key] ?? ""}>
                        <option value="">Do not import</option>
                        {current.source_headers.map((header) => {
                          const usedByAnotherField = Object.entries(mapping).some(([mappedKey, mappedHeader]) => mappedKey !== key && mappedHeader === header);
                          return <option disabled={usedByAnotherField} key={header} value={header}>{header}</option>;
                        })}
                      </Select>
                    </div>
                  ))}
                </div>
                {current.entity_key === "learners" ? <div><Label htmlFor="import-date-format">Date of birth format</Label><Select className="mt-1.5" disabled={!canEdit} id="import-date-format" onChange={(event) => setDateFormat(event.target.value as ImportDateFormat)} value={dateFormat}><option value="yyyy_mm_dd">YYYY-MM-DD</option><option value="dd_mm_yyyy">DD/MM/YYYY</option><option value="mm_dd_yyyy">MM/DD/YYYY</option></Select></div> : null}
              </div>
            ) : preview ? (
              <PreviewContent committed={isCommitted} preview={preview} target={current.entity_key} />
            ) : <p className="text-sm text-[var(--text-muted)]">No preview is available.</p>}
          </>
        )}
      </DialogBody>
      <DialogFooter>
        <Button disabled={saving} onClick={onClose} type="button" variant="ghost">Close</Button>
        {canEdit && current && mode === "preview" && !isCommitted ? <Button disabled={saving} onClick={() => setMode("mapping")} type="button" variant="secondary">Change mapping</Button> : null}
        {canEdit && current && mode === "mapping" && !isCommitted ? <Button disabled={saving} onClick={() => void validate()} type="button">{saving ? <><Loader2 className="size-4 animate-spin" />Validating…</> : "Create preview"}</Button> : null}
        {canCreate && current && mode === "preview" && preview && !isCommitted ? <Button disabled={saving || preview.ready_rows === 0} onClick={() => void commit()} type="button">{saving ? <><Loader2 className="size-4 animate-spin" />Committing…</> : `Commit ${preview.ready_rows} ready rows`}</Button> : null}
      </DialogFooter>
    </DialogShell>
  );
}

function PreviewContent({ committed, preview, target }: { committed: boolean; preview: SisImportPreview; target: SisImportTarget }) {
  return (
    <div className="space-y-5">
      {committed ? <div className="flex items-start gap-3 rounded-[var(--radius-lg)] border border-[var(--status-success-border)] bg-[var(--status-success-bg)] p-4 text-[var(--status-success-text)]"><CheckCircle2 className="mt-0.5 size-5 shrink-0" /><div><p className="text-sm font-semibold">Import committed</p><p className="mt-1 text-xs leading-5">The row results and original source remain available for audit.</p></div></div> : null}
      <div className="grid grid-cols-3 gap-3">
        <SummaryNumber label="Ready" tone="success" value={preview.ready_rows} />
        <SummaryNumber label="Invalid" tone="danger" value={preview.invalid_rows} />
        <SummaryNumber label="Duplicates" tone="warning" value={preview.duplicate_rows} />
      </div>
      <div><h3 className="text-sm font-semibold text-[var(--text-strong)]">Preview rows</h3><p className="mt-1 text-xs text-[var(--text-muted)]">Mapping version {preview.mapping_version}. Invalid and duplicate rows are not created.</p></div>
      <div className="overflow-x-auto rounded-[var(--radius-lg)] border border-[var(--border)]">
        <table className="w-full min-w-[620px] text-sm"><thead className="bg-[var(--table-header-bg)]"><tr><th className="px-4 py-3 text-left text-xs font-medium uppercase tracking-wider text-[var(--table-header-text)]">Row</th><th className="px-4 py-3 text-left text-xs font-medium uppercase tracking-wider text-[var(--table-header-text)]">Record</th><th className="px-4 py-3 text-left text-xs font-medium uppercase tracking-wider text-[var(--table-header-text)]">Result</th></tr></thead><tbody className="divide-y divide-[var(--border-subtle)]">
          {preview.rows.map((row) => <PreviewRow key={row.id} row={row} target={target} />)}
        </tbody></table>
      </div>
      {preview.total_rows > preview.rows.length ? <p className="text-xs text-[var(--text-muted)]">Showing the first {preview.rows.length} of {preview.total_rows} rows.</p> : null}
    </div>
  );
}

function PreviewRow({ row, target }: { row: SisImportPreviewRow; target: SisImportTarget }) {
  return <tr><td className="px-4 py-3 align-top font-tabular text-[var(--text-muted)]">{row.row_number}</td><td className="px-4 py-3 align-top"><p className="font-medium text-[var(--text-strong)]">{previewName(row, target)}</p><p className="mt-1 text-xs text-[var(--text-muted)]">{previewReference(row, target)}</p></td><td className="px-4 py-3 align-top"><Badge tone={row.outcome === "ready" ? "success" : row.outcome === "duplicate" ? "warning" : "danger"}>{displayStatus(row.outcome)}</Badge>{row.issues.length > 0 ? <ul className="mt-2 space-y-1 text-xs leading-4 text-[var(--text-muted)]">{row.issues.map((issue) => <li key={issue}>{issue}</li>)}</ul> : null}</td></tr>;
}

function SummaryNumber({ label, tone, value }: { label: string; tone: "success" | "warning" | "danger"; value: number }) {
  const classes = tone === "success" ? "border-[var(--status-success-border)] bg-[var(--status-success-bg)]" : tone === "warning" ? "border-[var(--status-warning-border)] bg-[var(--status-warning-bg)]" : "border-[var(--status-error-border)] bg-[var(--status-error-bg)]";
  return <div className={`rounded-[var(--radius-lg)] border p-3 ${classes}`}><p className="font-tabular text-xl font-semibold text-[var(--text-strong)]">{value}</p><p className="mt-1 text-xs text-[var(--text-muted)]">{label}</p></div>;
}

function PreviewTotals({ record }: { record: SisImportRecord }) {
  if (record.ready_rows === null) return <span className="text-xs text-[var(--text-muted)]">Not validated</span>;
  return <span className="text-xs text-[var(--text-muted)]"><span className="font-tabular font-medium text-[var(--text-strong)]">{record.ready_rows}</span> ready · {record.invalid_rows ?? 0} invalid · {record.duplicate_rows ?? 0} duplicate</span>;
}

function autoMapping(target: SisImportTarget, headers: string[]) {
  const normalized = new Map(headers.map((header) => [normalizeHeader(header), header]));
  return Object.fromEntries(MAPPING_FIELDS[target].flatMap(([key]) => {
    const header = HEADER_ALIASES[key]?.map((alias) => normalized.get(alias)).find(Boolean);
    return header ? [[key, header]] : [];
  }));
}

function normalizeHeader(value: string) { return value.toLowerCase().replace(/[^a-z0-9]/g, ""); }
function displayStatus(value: string) { return value.replace(/_/g, " "); }
function capitalise(value: string) { return value.charAt(0).toUpperCase() + value.slice(1); }
function networkMessage(error: unknown, fallback: string) { return error instanceof Error && error.message ? error.message : fallback; }
function statusTone(status: SisImportRecord["status"]): "neutral" | "warning" | "success" { return status === "committed" ? "success" : status === "preview_ready" ? "warning" : "neutral"; }
function formatBytes(value: number) { return value < 1024 ? `${value} B` : value < 1024 * 1024 ? `${(value / 1024).toFixed(1)} KB` : `${(value / (1024 * 1024)).toFixed(1)} MB`; }
function formatDateTime(value: string) { return new Intl.DateTimeFormat(undefined, { day: "numeric", month: "short", year: "numeric", hour: "2-digit", minute: "2-digit" }).format(new Date(value)); }
function previewName(row: SisImportPreviewRow, target: SisImportTarget) { const value = row.canonical_data.display_name; return typeof value === "string" ? value : target === "learners" ? "Learner row" : "Guardian row"; }
function previewReference(row: SisImportPreviewRow, target: SisImportTarget) { const value = target === "learners" ? row.canonical_data.learner_number : row.canonical_data.email ?? row.canonical_data.phone; return typeof value === "string" ? value : `Source row ${row.row_number}`; }

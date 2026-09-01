import { useCallback, useEffect, useState } from "react";
import { Plus, Search, ShieldAlert } from "lucide-react";
import toast from "react-hot-toast";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Table,
  TableControlsBar,
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

import { documentRegistryService, responseMessage } from "./service";
import type { LegalHold, RegistryFile } from "./types";
import { dateTime, label } from "./ui";

type DrawerState = { kind: "apply" } | { kind: "release"; hold: LegalHold } | null;

export function DocumentRegistryLegalHoldsWorkspace() {
  const [records, setRecords] = useState<LegalHold[]>([]);
  const [search, setSearch] = useState("");
  const [status, setStatus] = useState("active");
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [drawer, setDrawer] = useState<DrawerState>(null);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const response = await documentRegistryService.legalHolds({
        search: search.trim() || undefined,
        status: status === "all" ? undefined : status,
        per_page: 100,
      });
      if (!response.success || !response.data) {
        throw new Error(responseMessage(response, "Legal holds could not be loaded"));
      }
      setRecords(response.data.legal_holds);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : "Legal holds could not be loaded");
    } finally {
      setLoading(false);
    }
  }, [search, status]);

  useEffect(() => {
    void load();
  }, [load]);
  usePageChrome("Legal holds");

  return (
    <div className="space-y-6">
      <TableControlsBar>
        <Input
          aria-label="Search legal holds"
          className="sm:w-80"
          leadingIcon={<Search />}
          onChange={(event) => setSearch(event.target.value)}
          placeholder="Search document, reference, or reason"
          value={search}
        />
        <Select
          aria-label="Legal hold status"
          className="sm:w-44"
          onChange={(event) => setStatus(event.target.value)}
          value={status}
        >
          <option value="active">Active</option>
          <option value="released">Released</option>
          <option value="all">All statuses</option>
        </Select>
        <Button className="sm:ml-auto" onClick={() => setDrawer({ kind: "apply" })}>
          <Plus className="size-4" />
          Apply legal hold
        </Button>
      </TableControlsBar>

      <TableWrap>
        {loading ? (
          <TableLoading columns={6} label="Loading legal holds…" />
        ) : error ? (
          <TableError description={error} onRetry={() => void load()} />
        ) : records.length === 0 ? (
          <TableEmpty
            description={search ? "Change the current search or status." : "No documents match this legal-hold status."}
            icon={<ShieldAlert />}
            title="No legal holds"
          />
        ) : (
          <TableScroll>
            <Table className="min-w-[980px]">
              <THead>
                <tr>
                  <TH>Document</TH>
                  <TH>Hold reference</TH>
                  <TH>Reason</TH>
                  <TH>Status</TH>
                  <TH>Applied</TH>
                  <TH />
                </tr>
              </THead>
              <TBody>
                {records.map((record) => (
                  <TR key={record.id}>
                    <TD>
                      <span className="font-semibold text-[var(--text-strong)]">{record.file_reference}</span>
                      <p className="mt-1 max-w-72 truncate text-sm text-[var(--text-muted)]">{record.file_title}</p>
                    </TD>
                    <TD>{record.reference ?? "—"}</TD>
                    <TD><p className="max-w-80 line-clamp-2">{record.reason}</p></TD>
                    <TD><Badge tone={record.status === "active" ? "warning" : "success"}>{label(record.status)}</Badge></TD>
                    <TD>{dateTime(record.applied_at)}</TD>
                    <TD>
                      {record.status === "active" ? (
                        <Button onClick={() => setDrawer({ kind: "release", hold: record })} size="sm" variant="secondary">
                          Release
                        </Button>
                      ) : null}
                    </TD>
                  </TR>
                ))}
              </TBody>
            </Table>
          </TableScroll>
        )}
      </TableWrap>

      <LegalHoldDrawer
        drawer={drawer}
        onClose={() => setDrawer(null)}
        onSaved={() => {
          setDrawer(null);
          void load();
        }}
      />
    </div>
  );
}

function LegalHoldDrawer({
  drawer,
  onClose,
  onSaved,
}: {
  drawer: DrawerState;
  onClose: () => void;
  onSaved: () => void;
}) {
  const [files, setFiles] = useState<RegistryFile[]>([]);
  const [fileSearch, setFileSearch] = useState("");
  const [fileId, setFileId] = useState("");
  const [reference, setReference] = useState("");
  const [reason, setReason] = useState("");
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    setReference("");
    setReason("");
    setFileId("");
    setFileSearch("");
    if (drawer?.kind !== "apply") return;
  }, [drawer]);

  useEffect(() => {
    if (drawer?.kind !== "apply") return;
    const timer = window.setTimeout(() => void documentRegistryService.files({ per_page: 100, search: fileSearch.trim() || undefined }).then((response) => {
      if (!response.success || !response.data) return;
      const available = response.data.files.filter((file) => file.status !== "destroyed");
      setFiles(available);
      setFileId((current) => available.some((file) => file.id === current) ? current : (available[0]?.id ?? ""));
    }), 250);
    return () => window.clearTimeout(timer);
  }, [drawer, fileSearch]);

  const save = async (event: React.FormEvent) => {
    event.preventDefault();
    if (!drawer) return;
    setSaving(true);
    try {
      const response = drawer.kind === "apply"
        ? await (async () => {
            const selectedFile = files.find((file) => file.id === fileId);
            if (!selectedFile) throw new Error("Choose a document");
            return documentRegistryService.applyLegalHold(selectedFile, { reference: reference.trim() || null, reason: reason.trim() });
          })()
        : await documentRegistryService.releaseLegalHold(drawer.hold, reason.trim());
      if (!response.success) {
        throw new Error(responseMessage(response, "The legal hold could not be saved"));
      }
      toast.success(drawer.kind === "apply" ? "Legal hold applied" : "Legal hold released");
      onSaved();
    } catch (cause) {
      toast.error(cause instanceof Error ? cause.message : "The legal hold could not be saved");
    } finally {
      setSaving(false);
    }
  };

  const applying = drawer?.kind === "apply";
  return (
    <DialogShell onClose={onClose} open={Boolean(drawer)}>
      <form onSubmit={(event) => void save(event)}>
        <DialogHeader onClose={onClose} title={applying ? "Apply legal hold" : "Release legal hold"} />
        <DialogBody>
          <div className="space-y-5">
            {applying ? (
              <>
                <Field label="Document">
                  <Input leadingIcon={<Search />} onChange={(event) => setFileSearch(event.target.value)} placeholder="Find a document" value={fileSearch} />
                  <Select data-autofocus="true" onChange={(event) => setFileId(event.target.value)} required value={fileId}>
                    {files.length === 0 ? <option value="">No available documents</option> : null}
                    {files.map((file) => <option key={file.id} value={file.id}>{file.reference} · {file.title}</option>)}
                  </Select>
                </Field>
                <Field label="Hold reference">
                  <Input maxLength={120} onChange={(event) => setReference(event.target.value)} placeholder="Case or instruction reference" value={reference} />
                </Field>
              </>
            ) : (
              <div className="rounded-[var(--radius-lg)] border border-[var(--border)] bg-[var(--surface-muted)] p-4">
                <p className="font-semibold text-[var(--text-strong)]">{drawer?.kind === "release" ? drawer.hold.file_reference : ""}</p>
                <p className="mt-1 text-sm text-[var(--text-muted)]">{drawer?.kind === "release" ? drawer.hold.file_title : ""}</p>
              </div>
            )}
            <Field label={applying ? "Reason" : "Release reason"}>
              <Textarea data-autofocus={applying ? undefined : "true"} onChange={(event) => setReason(event.target.value)} required rows={6} value={reason} />
            </Field>
          </div>
        </DialogBody>
        <DialogFooter>
          <Button onClick={onClose} type="button" variant="secondary">Cancel</Button>
          <Button disabled={saving || !reason.trim() || (applying && !fileId)} type="submit">
            {saving ? "Saving…" : applying ? "Apply hold" : "Release hold"}
          </Button>
        </DialogFooter>
      </form>
    </DialogShell>
  );
}

function Field({ label: fieldLabel, children }: { label: string; children: React.ReactNode }) {
  return <div className="space-y-2"><Label>{fieldLabel}</Label>{children}</div>;
}

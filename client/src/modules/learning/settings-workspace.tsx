/** Learning upload settings backed by governed Document Registry classifications. */

import { useCallback, useEffect, useState } from "react";
import { FileLock2, FileUp, Loader2, Pencil } from "lucide-react";
import toast from "react-hot-toast";

import { Button } from "@/components/ui/button";
import { DialogBody, DialogFooter, DialogHeader } from "@/components/ui/dialog";
import { Label, Select } from "@/components/ui/input";
import { usePageChrome } from "@/modules/admin/layouts/page-chrome";

import { GuardedDrawer } from "./guarded-drawer";
import { learningService, responseMessage } from "./service";
import type { LearningSettings, LearningUploadClassificationOption } from "./types";

export function LearningSettingsWorkspace() {
  const [record, setRecord] = useState<LearningSettings | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [open, setOpen] = useState(false);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const response = await learningService.settings();
      if (!response.success || !response.data) {
        throw new Error(responseMessage(response, "Learning settings could not be loaded"));
      }
      setRecord(response.data);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : "Learning settings could not be loaded");
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  usePageChrome(
    "Learning settings",
    record ? (
      <Button onClick={() => setOpen(true)}>
        <Pencil className="size-4" />
        Change
      </Button>
    ) : null,
  );

  return (
    <div className="space-y-6">
      <p className="text-sm text-[var(--text-muted)]">
        Choose where teaching resources and learner submission files are retained.
      </p>
      {loading ? (
        <Panel>
          <Loader2 className="size-5 animate-spin text-[var(--brand-strong)]" />
          Loading settings…
        </Panel>
      ) : error ? (
        <Panel>
          {error}
          <Button className="ml-auto" onClick={() => void load()} size="sm" variant="secondary">
            Retry
          </Button>
        </Panel>
      ) : record ? (
        <div className="grid gap-4 lg:grid-cols-2">
          <SettingCard
            description={
              record.document_series_name
                ? "Teacher uploads are filed under this classification."
                : "Teachers can link existing governed files, but cannot upload new resources."
            }
            icon={<FileUp className="size-5" />}
            label="Teaching resources"
            value={record.document_series_name ?? "Not configured"}
          />
          <SettingCard
            description={
              record.learner_submission_series_name
                ? "Learner files are restricted and retained under this classification."
                : "Assignments can accept text only. Learner file uploads are unavailable."
            }
            icon={<FileLock2 className="size-5" />}
            label="Learner submissions"
            value={record.learner_submission_series_name ?? "Not configured"}
          />
        </div>
      ) : null}
      <SettingsDrawer
        onClose={() => setOpen(false)}
        onSaved={(value) => {
          setRecord(value);
          setOpen(false);
        }}
        open={open}
        record={record}
      />
    </div>
  );
}

function SettingsDrawer({
  onClose,
  onSaved,
  open,
  record,
}: {
  onClose: () => void;
  onSaved: (value: LearningSettings) => void;
  open: boolean;
  record: LearningSettings | null;
}) {
  const [resourceSeries, setResourceSeries] = useState<LearningUploadClassificationOption[]>([]);
  const [submissionSeries, setSubmissionSeries] = useState<LearningUploadClassificationOption[]>([]);
  const [resourceSeriesId, setResourceSeriesId] = useState(record?.document_series_id ?? "");
  const [submissionSeriesId, setSubmissionSeriesId] = useState(
    record?.learner_submission_series_id ?? "",
  );
  const [optionsLoading, setOptionsLoading] = useState(false);
  const [optionsError, setOptionsError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    if (!open) return;
    setResourceSeriesId(record?.document_series_id ?? "");
    setSubmissionSeriesId(record?.learner_submission_series_id ?? "");
    setOptionsLoading(true);
    setOptionsError(null);
    void learningService
      .uploadClassificationOptions()
      .then((response) => {
        if (!response.success || !response.data) {
          throw new Error(responseMessage(response, "Upload classifications could not be loaded"));
        }
        setResourceSeries(response.data.resource_series);
        setSubmissionSeries(response.data.learner_submission_series);
      })
      .catch((cause: unknown) => {
        setOptionsError(
          cause instanceof Error ? cause.message : "Upload classifications could not be loaded",
        );
      })
      .finally(() => setOptionsLoading(false));
  }, [open, record]);

  if (!record) return null;
  const dirty =
    resourceSeriesId !== (record.document_series_id ?? "") ||
    submissionSeriesId !== (record.learner_submission_series_id ?? "");

  const submit = async (event: React.FormEvent) => {
    event.preventDefault();
    setSaving(true);
    try {
      const response = await learningService.updateSettings(
        record,
        resourceSeriesId || null,
        submissionSeriesId || null,
      );
      if (!response.success || !response.data) {
        throw new Error(responseMessage(response, "Learning settings could not be updated"));
      }
      toast.success("Learning settings updated");
      onSaved(response.data);
    } catch (cause) {
      toast.error(cause instanceof Error ? cause.message : "Learning settings could not be updated");
    } finally {
      setSaving(false);
    }
  };

  return (
    <GuardedDrawer
      dirty={dirty}
      discardDescription="Your Learning settings have not been saved."
      onClose={onClose}
      open={open}
      pending={saving}
    >
      {(requestClose) => (
        <>
          <DialogHeader onClose={requestClose} title="Learning upload settings" />
          <form onSubmit={submit}>
            <DialogBody className="space-y-6">
              {optionsLoading ? (
                <p className="flex items-center gap-2 text-sm text-[var(--text-muted)]">
                  <Loader2 className="size-4 animate-spin" />
                  Loading classifications…
                </p>
              ) : optionsError ? (
                <p className="text-sm text-[var(--tone-danger)]">{optionsError}</p>
              ) : null}
              <div>
                <Label htmlFor="learning-resource-series">Teaching resource classification</Label>
                <Select
                  className="mt-1.5"
                  data-autofocus="true"
                  disabled={optionsLoading || Boolean(optionsError)}
                  id="learning-resource-series"
                  onChange={(event) => setResourceSeriesId(event.target.value)}
                  value={resourceSeriesId}
                >
                  <option value="">No direct uploads</option>
                  {resourceSeries.map((item) => (
                    <option key={item.id} value={item.id}>
                      {item.code} · {item.name}
                    </option>
                  ))}
                </Select>
              </div>
              <div>
                <Label htmlFor="learning-submission-series">Learner submission classification</Label>
                <Select
                  className="mt-1.5"
                  id="learning-submission-series"
                  disabled={optionsLoading || Boolean(optionsError)}
                  onChange={(event) => setSubmissionSeriesId(event.target.value)}
                  value={submissionSeriesId}
                >
                  <option value="">No learner file uploads</option>
                  {submissionSeries.map((item) => (
                    <option key={item.id} value={item.id}>
                      {item.code} · {item.name}
                    </option>
                  ))}
                </Select>
                <p className="mt-2 text-xs leading-5 text-[var(--text-muted)]">
                  Only active restricted classifications can retain learner submissions.
                </p>
              </div>
              <p className="text-xs leading-5 text-[var(--text-muted)]">
                Changing a classification does not move existing files or change their retention.
              </p>
            </DialogBody>
            <DialogFooter>
              <Button disabled={saving} onClick={requestClose} type="button" variant="secondary">
                Cancel
              </Button>
              <Button disabled={saving || optionsLoading || Boolean(optionsError) || !dirty} type="submit">
                {saving ? <Loader2 className="size-4 animate-spin" /> : null}
                {saving ? "Saving…" : "Save"}
              </Button>
            </DialogFooter>
          </form>
        </>
      )}
    </GuardedDrawer>
  );
}

function SettingCard({
  description,
  icon,
  label,
  value,
}: {
  description: string;
  icon: React.ReactNode;
  label: string;
  value: string;
}) {
  return (
    <section className="border border-[var(--border)] bg-[var(--surface)] p-6">
      <span className="text-[var(--brand-strong)]">{icon}</span>
      <p className="mt-4 text-xs font-semibold uppercase tracking-[0.14em] text-[var(--text-muted)]">
        {label}
      </p>
      <p className="mt-2 text-lg font-semibold text-[var(--text-strong)]">{value}</p>
      <p className="mt-2 text-sm leading-6 text-[var(--text-muted)]">{description}</p>
    </section>
  );
}

function Panel({ children }: { children: React.ReactNode }) {
  return (
    <div className="flex min-h-40 items-center gap-3 border border-[var(--border)] bg-[var(--surface)] p-6 text-sm text-[var(--text-muted)]">
      {children}
    </div>
  );
}

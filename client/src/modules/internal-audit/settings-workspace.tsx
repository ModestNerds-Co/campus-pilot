// Internal Audit reference sequence settings.

import { useCallback, useEffect, useState } from "react";
import toast from "react-hot-toast";

import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { TableError, TableLoading } from "@/components/ui/data-table";
import { Input, Label } from "@/components/ui/input";
import { usePageChrome } from "@/modules/admin/layouts/page-chrome";
import { useAuthStore } from "@/stores/auth-store";

import { internalAuditService, responseMessage } from "./service";
import type { NumberingPolicy } from "./types";
import { allowed } from "./ui";

export function InternalAuditSettingsWorkspace() {
  const permissions = useAuthStore((state) => state.user?.permissions ?? []);
  const canManage = allowed(permissions, "internal_audit:manage");
  const [record, setRecord] = useState<NumberingPolicy | null>(null);
  const [values, setValues] = useState(() => blankValues());
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const response = await internalAuditService.numbering();
      if (!response.success || !response.data) throw new Error(responseMessage(response, "Numbering settings could not be loaded"));
      setRecord(response.data);
      setValues(toValues(response.data));
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : "Numbering settings could not be loaded");
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => { void load(); }, [load]);
  usePageChrome("Settings");
  if (loading) return <TableLoading columns={1} label="Loading settings…" />;
  if (error || !record) return <TableError description={error ?? "Settings unavailable"} onRetry={() => void load()} />;

  const save = async (event: React.FormEvent) => {
    event.preventDefault();
    setSaving(true);
    try {
      const response = await internalAuditService.updateNumbering(record, {
        plan_prefix: values.planPrefix.trim(),
        engagement_prefix: values.engagementPrefix.trim(),
        finding_prefix: values.findingPrefix.trim(),
        padding: Number(values.padding),
        next_plan_sequence: Number(values.nextPlan),
        next_engagement_sequence: Number(values.nextEngagement),
        next_finding_sequence: Number(values.nextFinding),
      });
      if (!response.success || !response.data) throw new Error(responseMessage(response, "Numbering settings could not be saved"));
      setRecord(response.data);
      setValues(toValues(response.data));
      toast.success("Internal Audit numbering updated");
    } catch (cause) {
      toast.error(cause instanceof Error ? cause.message : "Numbering settings could not be saved");
    } finally {
      setSaving(false);
    }
  };

  return <Card className="max-w-5xl"><CardHeader><CardTitle>Audit references</CardTitle><CardDescription>Plans, engagements, and findings use separate campus sequences.</CardDescription></CardHeader><CardContent><form className="space-y-6" onSubmit={(event) => void save(event)}>
    <div className="grid gap-5 lg:grid-cols-3">
      <SequenceFields disabled={!canManage} next={values.nextPlan} nextReference={record.next_plan_reference} onNext={(value) => setValues({ ...values, nextPlan: value })} onPrefix={(value) => setValues({ ...values, planPrefix: value })} prefix={values.planPrefix} title="Plans" />
      <SequenceFields disabled={!canManage} next={values.nextEngagement} nextReference={record.next_engagement_reference} onNext={(value) => setValues({ ...values, nextEngagement: value })} onPrefix={(value) => setValues({ ...values, engagementPrefix: value })} prefix={values.engagementPrefix} title="Engagements" />
      <SequenceFields disabled={!canManage} next={values.nextFinding} nextReference={record.next_finding_reference} onNext={(value) => setValues({ ...values, nextFinding: value })} onPrefix={(value) => setValues({ ...values, findingPrefix: value })} prefix={values.findingPrefix} title="Findings" />
    </div>
    <Field label="Digits"><Input className="max-w-40" disabled={!canManage} max={12} min={3} onChange={(event) => setValues({ ...values, padding: event.target.value })} required type="number" value={values.padding} /></Field>
    {canManage ? <Button disabled={saving} type="submit">{saving ? "Saving…" : "Save numbering"}</Button> : null}
  </form></CardContent></Card>;
}

function SequenceFields({ disabled, next, nextReference, onNext, onPrefix, prefix, title }: { disabled: boolean; next: string; nextReference: string; onNext: (value: string) => void; onPrefix: (value: string) => void; prefix: string; title: string }) {
  return <section className="rounded-[var(--radius-lg)] border border-[var(--border)] p-4"><h2 className="font-semibold text-[var(--text-strong)]">{title}</h2><p className="mt-1 text-xs text-[var(--text-muted)]">Next: {nextReference}</p><div className="mt-4 space-y-4"><Field label="Prefix"><Input disabled={disabled} maxLength={20} onChange={(event) => onPrefix(event.target.value)} required value={prefix} /></Field><Field label="Next sequence"><Input disabled={disabled} min={1} onChange={(event) => onNext(event.target.value)} required type="number" value={next} /></Field></div></section>;
}

function toValues(record: NumberingPolicy) { return { planPrefix: record.plan_prefix, engagementPrefix: record.engagement_prefix, findingPrefix: record.finding_prefix, padding: String(record.padding), nextPlan: String(record.next_plan_sequence), nextEngagement: String(record.next_engagement_sequence), nextFinding: String(record.next_finding_sequence) }; }
function blankValues() { return { planPrefix: "IAP", engagementPrefix: "IAE", findingPrefix: "IAF", padding: "6", nextPlan: "1", nextEngagement: "1", nextFinding: "1" }; }
function Field({ label: fieldLabel, children }: { label: string; children: React.ReactNode }) { return <div className="space-y-2"><Label>{fieldLabel}</Label>{children}</div>; }

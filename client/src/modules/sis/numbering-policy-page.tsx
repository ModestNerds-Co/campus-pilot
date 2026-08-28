/** Tenant-scoped SIS learner numbering policy and drawer-first editing. */
import React, { useCallback, useEffect, useMemo, useState } from "react";
import { AlertCircle, Hash, Loader2, Pencil, RefreshCw } from "lucide-react";
import toast from "react-hot-toast";

import { Button } from "@/components/ui/button";
import { DialogBody, DialogFooter, DialogHeader, DialogShell } from "@/components/ui/dialog";
import { Input, Label, Textarea } from "@/components/ui/input";
import { usePageChrome } from "@/modules/admin/layouts/page-chrome";
import { useAuthStore } from "@/stores/auth-store";

import { responseMessage, sisService } from "./service";
import type { LearnerNumberingPolicy } from "./types";

const MAX_SEQUENCE = 100_000_000;

export const LearnerNumberingPolicyPage: React.FC = () => {
  const permissions = useAuthStore((state) => state.user?.permissions ?? []);
  const canEdit = permissions.includes("*") || permissions.includes("sis:edit");
  const [policy, setPolicy] = useState<LearnerNumberingPolicy | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [drawerOpen, setDrawerOpen] = useState(false);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const response = await sisService.getLearnerNumberingPolicy();
      if (!response.success || !response.data) {
        throw new Error(responseMessage(response, "Learner numbering could not be loaded"));
      }
      setPolicy(response.data);
    } catch (loadError) {
      setError(loadError instanceof Error ? loadError.message : "Learner numbering could not be loaded");
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => { void load(); }, [load]);
  usePageChrome(
    "SIS settings",
    canEdit && policy && !policy.exhausted
      ? <Button onClick={() => setDrawerOpen(true)}><Pencil className="size-4" />Edit numbering</Button>
      : null,
  );

  return (
    <div className="space-y-6">
      <header>
        <p className="text-[11px] font-semibold uppercase tracking-[0.16em] text-[var(--brand-strong)]">Learner records</p>
        <h1 className="mt-1 text-2xl font-semibold tracking-[-0.03em] text-[var(--text-strong)]">Learner numbering</h1>
        <p className="mt-2 max-w-2xl text-sm leading-6 text-[var(--text-muted)]">Numbers are assigned when a learner is created. Existing numbers do not change.</p>
      </header>

      {loading ? (
        <section className="flex min-h-56 items-center justify-center border border-[var(--border)] bg-[var(--surface)]" aria-label="Loading learner numbering">
          <Loader2 className="size-5 animate-spin text-[var(--brand-strong)]" />
        </section>
      ) : error ? (
        <section className="border border-[var(--tone-danger-bd)] bg-[var(--tone-danger-bg)] p-5">
          <div className="flex items-start gap-3"><AlertCircle className="mt-0.5 size-5 shrink-0 text-[var(--tone-danger)]" /><div><h2 className="font-semibold text-[var(--text-strong)]">Learner numbering is unavailable</h2><p className="mt-1 text-sm text-[var(--text-muted)]">{error}</p></div></div>
          <Button className="mt-4" onClick={() => void load()} variant="secondary"><RefreshCw className="size-4" />Retry</Button>
        </section>
      ) : policy ? (
        <section className="border border-[var(--border)] bg-[var(--surface)]" aria-labelledby="numbering-policy-title">
          <div className="flex flex-wrap items-center justify-between gap-4 border-b border-[var(--border)] px-5 py-4 sm:px-6">
            <div><h2 className="font-semibold text-[var(--text-strong)]" id="numbering-policy-title">Current policy</h2><p className="mt-1 text-xs text-[var(--text-muted)]">Applies to new learner records.</p></div>
            {canEdit && !policy.exhausted ? <Button onClick={() => setDrawerOpen(true)} size="sm" variant="secondary"><Pencil className="size-4" />Edit</Button> : null}
          </div>
          <dl className="grid sm:grid-cols-2 xl:grid-cols-4">
            <PolicyValue label="Next learner number" value={policy.next_number_preview ?? "Sequence exhausted"} emphasized />
            <PolicyValue label="Prefix" value={policy.number_prefix} />
            <PolicyValue label="Minimum digits" value={String(policy.number_padding)} />
            <PolicyValue label="Next sequence" value={policy.exhausted ? "Exhausted" : formatSequence(policy.next_sequence)} />
          </dl>
          {policy.exhausted ? <p className="border-t border-[var(--tone-danger-bd)] bg-[var(--tone-danger-bg)] px-5 py-4 text-sm text-[var(--tone-danger)] sm:px-6">No more learner numbers can be issued from this sequence.</p> : null}
        </section>
      ) : null}

      {policy ? <NumberingPolicyDrawer onClose={() => setDrawerOpen(false)} onSaved={(updated) => { setPolicy(updated); setDrawerOpen(false); }} open={drawerOpen} policy={policy} /> : null}
    </div>
  );
};

const PolicyValue: React.FC<{ emphasized?: boolean; label: string; value: string }> = ({ emphasized, label, value }) => (
  <div className="border-b border-[var(--border)] px-5 py-5 last:border-b-0 sm:border-r sm:px-6 sm:[&:nth-child(2n)]:border-r-0 xl:border-b-0 xl:[&:nth-child(2n)]:border-r xl:last:border-r-0">
    <dt className="text-[11px] font-semibold uppercase tracking-[0.13em] text-[var(--text-subtle)]">{label}</dt>
    <dd className={`mt-2 font-tabular ${emphasized ? "text-xl font-semibold text-[var(--brand-strong)]" : "text-base font-medium text-[var(--text-strong)]"}`}>{value}</dd>
  </div>
);

const NumberingPolicyDrawer: React.FC<{
  onClose: () => void;
  onSaved: (policy: LearnerNumberingPolicy) => void;
  open: boolean;
  policy: LearnerNumberingPolicy;
}> = ({ onClose, onSaved, open, policy }) => {
  const [prefix, setPrefix] = useState(policy.number_prefix);
  const [padding, setPadding] = useState(String(policy.number_padding));
  const [nextSequence, setNextSequence] = useState(String(policy.next_sequence));
  const [reason, setReason] = useState("");
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    if (!open) return;
    setPrefix(policy.number_prefix);
    setPadding(String(policy.number_padding));
    setNextSequence(String(policy.next_sequence));
    setReason("");
  }, [open, policy]);

  const numericPadding = Number(padding);
  const numericSequence = Number(nextSequence);
  const preview = useMemo(() => renderPreview(prefix, numericPadding, numericSequence), [prefix, numericPadding, numericSequence]);
  const willExhaust = numericSequence === MAX_SEQUENCE;
  const sequenceBehind = Number.isInteger(numericSequence) && numericSequence < policy.next_sequence;
  const valid = prefix.trim().length > 0
    && prefix.trim().length <= 32
    && Number.isInteger(numericPadding) && numericPadding >= 1 && numericPadding <= 8
    && Number.isInteger(numericSequence) && numericSequence >= policy.next_sequence && numericSequence <= MAX_SEQUENCE
    && reason.trim().length > 0 && reason.trim().length <= 1_000;

  const submit = async (event: React.FormEvent) => {
    event.preventDefault();
    if (!valid || saving) return;
    setSaving(true);
    try {
      const response = await sisService.updateLearnerNumberingPolicy({
        number_prefix: prefix.trim(),
        number_padding: numericPadding,
        next_sequence: numericSequence,
        expected_version: policy.version,
        reason: reason.trim(),
      });
      if (!response.success || !response.data) {
        toast.error(responseMessage(response, "Learner numbering could not be updated"));
        return;
      }
      toast.success("Learner numbering updated");
      onSaved(response.data);
    } catch {
      toast.error("Learner numbering could not be updated");
    } finally {
      setSaving(false);
    }
  };

  return (
    <DialogShell onClose={() => !saving && onClose()} open={open}>
      <DialogHeader onClose={() => !saving && onClose()} title="Edit learner numbering" />
      <form onSubmit={submit}>
        <DialogBody className="space-y-6">
          <div className="border border-[var(--border)] bg-[var(--surface-muted)] p-4">
            <div className="flex items-center gap-2 text-xs font-semibold uppercase tracking-[0.12em] text-[var(--text-subtle)]"><Hash className="size-4" />Next learner number</div>
            <p className="mt-2 break-all font-tabular text-xl font-semibold text-[var(--brand-strong)]">{willExhaust ? "Sequence will be exhausted" : preview ?? "Enter a valid policy"}</p>
            {willExhaust ? <p className="mt-2 text-xs leading-5 text-[var(--tone-danger)]">No further learner numbers can be issued after this change.</p> : null}
          </div>
          <div><Label htmlFor="learner-number-prefix">Prefix</Label><Input className="mt-1.5" data-autofocus="true" id="learner-number-prefix" maxLength={32} onChange={(event) => setPrefix(event.target.value)} required value={prefix} /></div>
          <div><Label htmlFor="learner-number-padding">Minimum digits</Label><Input className="mt-1.5" id="learner-number-padding" inputMode="numeric" max={8} min={1} onChange={(event) => setPadding(event.target.value)} required type="number" value={padding} /><p className="mt-1.5 text-xs leading-5 text-[var(--text-muted)]">Adds leading zeroes until the sequence reaches this width.</p></div>
          <div><Label htmlFor="learner-number-next">Next sequence</Label><Input aria-invalid={sequenceBehind} className="mt-1.5" id="learner-number-next" inputMode="numeric" max={MAX_SEQUENCE} min={policy.next_sequence} onChange={(event) => setNextSequence(event.target.value)} required type="number" value={nextSequence} />{sequenceBehind ? <p className="mt-1.5 text-xs text-[var(--tone-danger)]">Use {formatSequence(policy.next_sequence)} or a higher number.</p> : <p className="mt-1.5 text-xs leading-5 text-[var(--text-muted)]">Numbers below the current boundary cannot be reused.</p>}</div>
          <div><Label htmlFor="learner-number-reason">Reason</Label><Textarea className="mt-1.5" id="learner-number-reason" maxLength={1_000} onChange={(event) => setReason(event.target.value)} required rows={4} value={reason} /></div>
        </DialogBody>
        <DialogFooter><Button disabled={saving} onClick={onClose} type="button" variant="secondary">Cancel</Button><Button disabled={!valid || saving} type="submit">{saving ? <><Loader2 className="size-4 animate-spin" />Saving…</> : "Save numbering"}</Button></DialogFooter>
      </form>
    </DialogShell>
  );
};

function renderPreview(prefix: string, padding: number, sequence: number) {
  const normalizedPrefix = prefix.trim();
  if (!normalizedPrefix || normalizedPrefix.length > 32 || !Number.isInteger(padding) || padding < 1 || padding > 8 || !Number.isInteger(sequence) || sequence < 1 || sequence > 99_999_999) return null;
  return `${normalizedPrefix}${String(sequence).padStart(padding, "0")}`;
}

function formatSequence(value: number) {
  return new Intl.NumberFormat().format(value);
}

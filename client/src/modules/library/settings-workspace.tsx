import { useEffect, useMemo, useState, type ReactNode } from "react";
import { Loader2, Save } from "lucide-react";
import toast from "react-hot-toast";

import { Button } from "@/components/ui/button";
import { Input, Label, Select } from "@/components/ui/input";
import { usePageChrome } from "@/modules/admin/layouts/page-chrome";

import { libraryService, responseMessage } from "./service";
import type { CurrencyReference, LibrarySettings } from "./types";

export function LibrarySettingsWorkspace() {
  const [settings, setSettings] = useState<LibrarySettings | null>(null);
  const [currencies, setCurrencies] = useState<CurrencyReference[]>([]);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [prefix, setPrefix] = useState("");
  const [nextSequence, setNextSequence] = useState("");
  const [padding, setPadding] = useState("");
  const [learnerDays, setLearnerDays] = useState("");
  const [employeeDays, setEmployeeDays] = useState("");
  const [loanLimit, setLoanLimit] = useState("");
  const [renewals, setRenewals] = useState("");
  const [currencyId, setCurrencyId] = useState("");
  const [fineAmount, setFineAmount] = useState("");
  usePageChrome("Library settings");
  useEffect(() => {
    void Promise.all([libraryService.settings(), libraryService.references()])
      .then(([settingsResponse, referenceResponse]) => {
        if (!settingsResponse.success || !settingsResponse.data)
          throw new Error(
            responseMessage(
              settingsResponse,
              "Library settings could not be loaded",
            ),
          );
        const value = settingsResponse.data;
        setSettings(value);
        setPrefix(value.accession_prefix);
        setNextSequence(String(value.accession_next_sequence));
        setPadding(String(value.accession_padding));
        setLearnerDays(String(value.learner_loan_days));
        setEmployeeDays(String(value.employee_loan_days));
        setLoanLimit(String(value.default_loan_limit));
        setRenewals(String(value.maximum_renewals));
        setCurrencyId(value.fine_currency_id ?? "");
        const units = value.fine_currency_minor_units ?? 2;
        setFineAmount(
          value.overdue_fine_minor
            ? (value.overdue_fine_minor / 10 ** units).toFixed(units)
            : "0",
        );
        if (referenceResponse.success)
          setCurrencies(referenceResponse.data?.currencies ?? []);
      })
      .catch((loadError) =>
        setError(
          loadError instanceof Error
            ? loadError.message
            : "Library settings could not be loaded",
        ),
      )
      .finally(() => setLoading(false));
  }, []);
  const selectedCurrency = useMemo(
    () => currencies.find((currency) => currency.id === currencyId),
    [currencies, currencyId],
  );
  const preview = useMemo(
    () =>
      `${prefix.trim().toUpperCase()}-${String(nextSequence || "1").padStart(Number(padding) || 1, "0")}`,
    [nextSequence, padding, prefix],
  );
  const submit = async (event: React.FormEvent) => {
    event.preventDefault();
    if (saving) return;
    const minorUnits = selectedCurrency?.minor_units ?? 2;
    setSaving(true);
    try {
      const response = await libraryService.updateSettings({
        accession_prefix: prefix,
        accession_next_sequence: Number(nextSequence),
        accession_padding: Number(padding),
        learner_loan_days: Number(learnerDays),
        employee_loan_days: Number(employeeDays),
        default_loan_limit: Number(loanLimit),
        maximum_renewals: Number(renewals),
        fine_currency_id: currencyId || null,
        overdue_fine_minor: Math.round(
          Number(fineAmount || 0) * 10 ** minorUnits,
        ),
      });
      if (!response.success || !response.data)
        throw new Error(
          responseMessage(response, "Library settings could not be saved"),
        );
      setSettings(response.data);
      toast.success("Library settings saved");
    } catch (saveError) {
      toast.error(
        saveError instanceof Error
          ? saveError.message
          : "Library settings could not be saved",
      );
    } finally {
      setSaving(false);
    }
  };
  if (loading)
    return <div className="h-64 animate-pulse bg-[var(--surface-sunken)]" />;
  if (error || !settings)
    return (
      <div className="border border-[var(--tone-danger-bd)] bg-[var(--badge-danger-bg)] p-5 text-sm text-[var(--badge-danger-text)]">
        {error || "Library settings are unavailable."}
      </div>
    );
  return (
    <form className="space-y-8" onSubmit={submit}>
      <p className="text-sm text-[var(--text-muted)]">
        Configure copy numbering, lending periods, limits, and overdue fines.
      </p>
      <section className="border border-[var(--border)] bg-[var(--surface)] p-5 sm:p-6">
        <h2 className="text-base font-semibold text-[var(--text-strong)]">
          Accession numbers
        </h2>
        <p className="mt-1 text-sm text-[var(--text-muted)]">
          Each new physical copy receives the next number once.
        </p>
        <div className="mt-5 grid gap-5 sm:grid-cols-3">
          <Field label="Prefix">
            <Input
              onChange={(event) => setPrefix(event.target.value)}
              required
              value={prefix}
            />
          </Field>
          <Field label="Next sequence">
            <Input
              min="1"
              onChange={(event) => setNextSequence(event.target.value)}
              required
              type="number"
              value={nextSequence}
            />
          </Field>
          <Field label="Padding">
            <Input
              max="8"
              min="1"
              onChange={(event) => setPadding(event.target.value)}
              required
              type="number"
              value={padding}
            />
          </Field>
        </div>
        <div className="mt-5 border-l-2 border-[var(--brand-strong)] bg-[var(--surface-muted)] px-4 py-3">
          <p className="text-xs font-medium uppercase tracking-[0.12em] text-[var(--text-muted)]">
            Next number
          </p>
          <p className="mt-1 font-tabular text-lg font-semibold text-[var(--text-strong)]">
            {preview}
          </p>
        </div>
      </section>
      <section className="border border-[var(--border)] bg-[var(--surface)] p-5 sm:p-6">
        <h2 className="text-base font-semibold text-[var(--text-strong)]">
          Lending policy
        </h2>
        <div className="mt-5 grid gap-5 sm:grid-cols-2">
          <Field label="Learner loan days">
            <Input
              max="365"
              min="1"
              onChange={(event) => setLearnerDays(event.target.value)}
              required
              type="number"
              value={learnerDays}
            />
          </Field>
          <Field label="Employee loan days">
            <Input
              max="365"
              min="1"
              onChange={(event) => setEmployeeDays(event.target.value)}
              required
              type="number"
              value={employeeDays}
            />
          </Field>
          <Field label="Default active-loan limit">
            <Input
              max="100"
              min="1"
              onChange={(event) => setLoanLimit(event.target.value)}
              required
              type="number"
              value={loanLimit}
            />
          </Field>
          <Field label="Maximum renewals">
            <Input
              max="20"
              min="0"
              onChange={(event) => setRenewals(event.target.value)}
              required
              type="number"
              value={renewals}
            />
          </Field>
        </div>
      </section>
      <section className="border border-[var(--border)] bg-[var(--surface)] p-5 sm:p-6">
        <h2 className="text-base font-semibold text-[var(--text-strong)]">
          Overdue fines
        </h2>
        <p className="mt-1 text-sm text-[var(--text-muted)]">
          Set zero to disable automatic overdue-fine assessment.
        </p>
        <div className="mt-5 grid gap-5 sm:grid-cols-2">
          <Field label="Currency">
            <Select
              onChange={(event) => setCurrencyId(event.target.value)}
              value={currencyId}
            >
              <option value="">No fine currency</option>
              {currencies.map((currency) => (
                <option key={currency.id} value={currency.id}>
                  {currency.code}
                </option>
              ))}
            </Select>
          </Field>
          <Field
            label={`Amount per overdue day${selectedCurrency ? ` (${selectedCurrency.code})` : ""}`}
          >
            <Input
              min="0"
              onChange={(event) => setFineAmount(event.target.value)}
              step={
                selectedCurrency ? 1 / 10 ** selectedCurrency.minor_units : 0.01
              }
              type="number"
              value={fineAmount}
            />
          </Field>
        </div>
      </section>
      <div className="flex justify-end">
        <Button disabled={saving} type="submit">
          {saving ? (
            <>
              <Loader2 className="size-4 animate-spin" />
              Saving…
            </>
          ) : (
            <>
              <Save className="size-4" />
              Save settings
            </>
          )}
        </Button>
      </div>
    </form>
  );
}

function Field({
  children,
  label,
}: {
  children: ReactNode;
  label: string;
}) {
  return (
    <div>
      <Label>{label}</Label>
      <div className="mt-1.5">{children}</div>
    </div>
  );
}

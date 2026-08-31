/**
 * Shared presentation helpers for Agent Administration reporting pages.
 * These helpers format only server-owned values and never infer missing operational state.
 */

import React from "react";

import { StatusChip } from "@/components/ui/status";

export function AgentMetric({ label, value, detail }: { label: string; value: React.ReactNode; detail?: string }) {
  return (
    <div className="min-w-0 border-r border-[var(--border-subtle)] px-4 py-4 last:border-r-0 sm:px-5">
      <dt className="text-[11px] font-semibold uppercase tracking-[0.14em] text-[var(--text-muted)]">{label}</dt>
      <dd className="mt-2 text-2xl font-semibold tabular-nums tracking-[-0.035em] text-[var(--text-strong)]">{value}</dd>
      {detail ? <p className="mt-1 text-xs leading-5 text-[var(--text-muted)]">{detail}</p> : null}
    </div>
  );
}

export function AgentStatus({ value }: { value: string }) {
  const tone = statusTone(value);
  return <StatusChip dot tone={tone}>{statusLabel(value)}</StatusChip>;
}

export function statusTone(value: string): "neutral" | "info" | "success" | "warn" | "danger" | "pending" {
  if (["ready", "enabled", "active", "completed", "succeeded", "executable"].includes(value)) return "success";
  if (["queued", "running", "awaiting_approval", "approval_required", "approval_not_released"].includes(value)) return "pending";
  if (["error", "failed", "expired", "revoked", "prohibited", "module_unavailable", "handler_unavailable"].includes(value)) return "danger";
  if (["interrupted", "cancelled", "human_only", "attention"].includes(value)) return "warn";
  return "neutral";
}

export function statusLabel(value: string) {
  return value
    .replace(/_/g, " ")
    .replace(/(^|\s)\S/g, (character: string) => character.toUpperCase());
}

export function formatCount(value: number) {
  return new Intl.NumberFormat().format(value);
}

export function formatTimestamp(value: string | null) {
  if (!value) return "Not recorded";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return "Invalid timestamp";
  return new Intl.DateTimeFormat(undefined, {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(date);
}

export function formatDuration(start: string | null, finish: string | null) {
  if (!start || !finish) return "Not complete";
  const duration = Math.max(0, new Date(finish).getTime() - new Date(start).getTime());
  if (duration < 1_000) return `${duration} ms`;
  if (duration < 60_000) return `${(duration / 1_000).toFixed(1)} s`;
  return `${Math.floor(duration / 60_000)}m ${Math.round((duration % 60_000) / 1_000)}s`;
}

export function formatUsageAmount(
  amount: number,
  currency: string | null,
  exponent: number | null,
) {
  if (!currency || exponent == null) return formatCount(amount);
  const divisor = 10 ** exponent;
  return `${currency} ${(amount / divisor).toLocaleString(undefined, {
    minimumFractionDigits: exponent,
    maximumFractionDigits: exponent,
  })}`;
}

export function ForbiddenPanel({ area }: { area: string }) {
  return (
    <section className="border border-[var(--border)] bg-[var(--surface)] p-6" role="status">
      <h2 className="font-semibold text-[var(--text-strong)]">Access required</h2>
      <p className="mt-2 text-sm leading-6 text-[var(--text-muted)]">
        Your account cannot open {area}.
      </p>
    </section>
  );
}

import { Loader2 } from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";

export function LearningStatusBadge({ status }: { status: string }) {
  const tone = status === "published" || status === "graded"
    ? "success"
    : status === "draft"
      ? "warning"
      : status === "submitted" || status === "revision_requested"
        ? "info"
        : "neutral";
  return <Badge tone={tone}>{learningLabel(status)}</Badge>;
}

export function LearningState({
  busy,
  description,
  onRetry,
  title,
}: {
  busy?: boolean;
  description?: string;
  onRetry?: () => void;
  title: string;
}) {
  return (
    <div className="flex min-h-64 flex-col items-center justify-center border border-[var(--border)] bg-[var(--surface)] p-8 text-center">
      {busy ? <Loader2 className="mb-3 size-6 animate-spin text-[var(--brand-strong)]" /> : null}
      <p className="font-medium text-[var(--text-strong)]">{title}</p>
      {description ? <p className="mt-2 max-w-lg text-sm text-[var(--text-muted)]">{description}</p> : null}
      {onRetry ? <Button className="mt-4" onClick={onRetry} variant="secondary">Retry</Button> : null}
    </div>
  );
}

export function learningLabel(value: string) {
  return value.replace(/_/g, " ").replace(/^./, (letter) => letter.toUpperCase());
}

export function formatLearningDate(value: string) {
  return new Intl.DateTimeFormat(undefined, {
    day: "numeric",
    month: "short",
    year: "numeric",
  }).format(new Date(value));
}

export function formatLearningDateTime(value: string) {
  return new Intl.DateTimeFormat(undefined, {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(new Date(value));
}

export function formatHundredths(value: number | null | undefined) {
  if (value === null || value === undefined) return "—";
  return `${Math.floor(value / 100)}.${String(Math.abs(value % 100)).padStart(2, "0")}`;
}

export function parseHundredths(value: string) {
  const match = value.trim().match(/^(\d+)(?:\.(\d{0,2}))?$/);
  if (!match) return null;
  return Number(match[1]) * 100 + Number((match[2] ?? "").padEnd(2, "0"));
}

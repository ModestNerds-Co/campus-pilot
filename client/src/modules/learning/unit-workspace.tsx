import { useCallback, useEffect, useRef, useState } from "react";
import { Link } from "@tanstack/react-router";
import { ArrowLeft, Download, FileText } from "lucide-react";
import toast from "react-hot-toast";

import { Button } from "@/components/ui/button";
import { usePageChrome } from "@/modules/admin/layouts/page-chrome";

import { learningService, responseMessage } from "./service";
import type { LearningSpace, LearningUnit } from "./types";
import {
  formatLearningDate,
  LearningState,
  LearningStatusBadge,
  learningLabel,
} from "./ui";

export function LearningUnitWorkspace({ spaceId, unitId }: { spaceId: string; unitId: string }) {
  const [space, setSpace] = useState<LearningSpace | null>(null);
  const [unit, setUnit] = useState<LearningUnit | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const requestRef = useRef(0);

  const load = useCallback(async () => {
    const requestId = ++requestRef.current;
    setLoading(true);
    setError(null);
    try {
      const response = await learningService.space(spaceId);
      if (!response.success || !response.data) {
        throw new Error(responseMessage(response, "Learning unit could not be loaded"));
      }
      if (requestId !== requestRef.current) return;
      const selected = response.data.units.find((item) => item.id === unitId) ?? null;
      setSpace(response.data);
      setUnit(selected);
    } catch (loadError) {
      if (requestId !== requestRef.current) return;
      setError(loadError instanceof Error ? loadError.message : "Learning unit could not be loaded");
    } finally {
      if (requestId === requestRef.current) setLoading(false);
    }
  }, [spaceId, unitId]);

  useEffect(() => {
    void load();
    return () => {
      requestRef.current += 1;
    };
  }, [load]);

  usePageChrome(unit?.title ?? "Learning unit");

  if (loading) return <LearningState busy title="Loading learning unit…" />;
  if (error) return <LearningState description={error} onRetry={() => void load()} title="Learning unit unavailable" />;
  if (!space || !unit) {
    return <LearningState description="This unit does not exist or is no longer available." title="Learning unit not found" />;
  }

  return (
    <div className="space-y-7">
      <Link
        className="inline-flex items-center gap-2 text-sm font-medium text-[var(--text-muted)] hover:text-[var(--text-strong)]"
        params={{ spaceId }}
        to="/modules/learning/spaces/$spaceId"
      >
        <ArrowLeft className="size-4" /> {space.title}
      </Link>

      <section className="border border-[var(--border)] bg-[var(--surface)] p-5 sm:p-6">
        <div className="flex flex-wrap items-start justify-between gap-4">
          <div>
            <div className="flex flex-wrap items-center gap-2">
              <span className="font-tabular text-xs font-semibold uppercase tracking-[0.12em] text-[var(--brand-strong)]">
                Unit {unit.position}
              </span>
              <LearningStatusBadge status={unit.status} />
            </div>
            <h1 className="mt-3 text-2xl font-semibold tracking-[-0.03em] text-[var(--text-strong)]">
              {unit.title}
            </h1>
            <p className="mt-2 text-sm text-[var(--text-muted)]">
              {space.subject_name} · {space.class_group_name} · {space.academic_term_name}
            </p>
          </div>
          <p className="text-xs text-[var(--text-muted)]">Updated {formatLearningDate(unit.updated_at)}</p>
        </div>
        {unit.summary ? (
          <p className="mt-5 max-w-3xl whitespace-pre-wrap text-sm leading-6 text-[var(--text-muted)]">
            {unit.summary}
          </p>
        ) : null}
      </section>

      <div className="flex flex-wrap items-center justify-between gap-3">
        <div>
          <h2 className="text-lg font-semibold text-[var(--text-strong)]">Resources</h2>
          <p className="mt-1 text-sm text-[var(--text-muted)]">Published material available in this unit.</p>
        </div>
        <Link
          className="inline-flex min-h-10 items-center rounded-[var(--radius-md)] border border-[var(--border)] px-4 text-sm font-semibold text-[var(--text-strong)] hover:bg-[var(--surface-muted)]"
          params={{ spaceId }}
          search={{ page: 1, status: "all" }}
          to="/modules/learning/spaces/$spaceId/assignments"
        >
          Assignments
        </Link>
      </div>

      {unit.resources.length === 0 ? (
        <LearningState description="Resources added to this unit will appear here." title="No resources" />
      ) : (
        <div className="divide-y divide-[var(--border)] border border-[var(--border)] bg-[var(--surface)]">
          {unit.resources.map((resource) => (
            <article className="flex flex-wrap items-center justify-between gap-4 p-4 sm:p-5" key={resource.id}>
              <div className="min-w-0">
                <div className="flex flex-wrap items-center gap-2">
                  <FileText className="size-4 text-[var(--brand-strong)]" />
                  <h3 className="font-medium text-[var(--text-strong)]">{resource.display_title}</h3>
                  <LearningStatusBadge status={resource.status} />
                </div>
                <p className="mt-1 text-xs text-[var(--text-muted)]">
                  {resource.document?.reference ?? "Governed file unavailable"} · {learningLabel(resource.sensitivity_snapshot)}
                </p>
              </div>
              <Button
                onClick={() => void (async () => {
                  const response = await learningService.downloadResource(resource.id);
                  if (!response.success || !response.data) {
                    toast.error(responseMessage(response, "Resource could not be opened"));
                    return;
                  }
                  window.open(response.data.url, "_blank", "noopener,noreferrer");
                })()}
                size="sm"
                variant="secondary"
              >
                <Download className="size-4" /> Open
              </Button>
            </article>
          ))}
        </div>
      )}
    </div>
  );
}

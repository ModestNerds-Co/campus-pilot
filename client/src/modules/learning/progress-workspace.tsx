import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Link } from "@tanstack/react-router";
import { Search, TrendingUp } from "lucide-react";

import {
  Table, TableControlsBar, TableControlsPagination, TableEmpty, TableError,
  TableLoading, TableScroll, TableWrap, TBody, TD, TH, THead, TR,
} from "@/components/ui/data-table";
import { Input } from "@/components/ui/input";
import { usePageChrome } from "@/modules/admin/layouts/page-chrome";
import { useAuthStore } from "@/stores/auth-store";

import { learningService, responseMessage } from "./service";
import type { LearningProgressEntry, LearningSpace } from "./types";
import { formatHundredths, LearningState } from "./ui";

export interface LearningProgressSearchState { page: number; q: string }

export function LearningProgressWorkspace({ onSearchChange, search, spaceId }: {
  onSearchChange: (next: LearningProgressSearchState, options?: { replace?: boolean }) => void;
  search: LearningProgressSearchState;
  spaceId: string;
}) {
  const permissions = useAuthStore((state) => state.user?.permissions ?? []);
  const canTeach = permissions.includes("*") || permissions.includes("learning:teach");
  const [space, setSpace] = useState<LearningSpace | null>(null);
  const [progress, setProgress] = useState<LearningProgressEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const requestRef = useRef(0);

  const load = useCallback(async () => {
    const requestId = ++requestRef.current;
    setLoading(true);
    setError(null);
    try {
      const [spaceResponse, progressResponse] = await Promise.all([
        learningService.space(spaceId),
        canTeach ? learningService.progress(spaceId) : learningService.myProgress(spaceId),
      ]);
      if (!spaceResponse.success || !spaceResponse.data) throw new Error(responseMessage(spaceResponse, "Learning space could not be loaded"));
      if (!progressResponse.success || !progressResponse.data) throw new Error(responseMessage(progressResponse, "Learning progress could not be loaded"));
      if (requestId !== requestRef.current) return;
      setSpace(spaceResponse.data);
      setProgress(canTeach ? "progress" in progressResponse.data ? progressResponse.data.progress : [] : [progressResponse.data as LearningProgressEntry]);
    } catch (loadError) {
      if (requestId !== requestRef.current) return;
      setError(loadError instanceof Error ? loadError.message : "Learning progress could not be loaded");
    } finally {
      if (requestId === requestRef.current) setLoading(false);
    }
  }, [canTeach, spaceId]);

  useEffect(() => {
    void load();
    return () => { requestRef.current += 1; };
  }, [load]);
  usePageChrome(canTeach ? "Class progress" : "My progress");

  const filtered = useMemo(() => {
    const query = search.q.trim().toLowerCase();
    return query ? progress.filter((entry) => [entry.learner_name, entry.learner_number].some((value) => value.toLowerCase().includes(query))) : progress;
  }, [progress, search.q]);
  const totalPages = Math.max(1, Math.ceil(filtered.length / 25));
  const page = Math.min(search.page, totalPages);
  const visible = filtered.slice((page - 1) * 25, page * 25);

  if (loading) return <LearningState busy title="Loading learning progress…" />;
  if (error) return <LearningState description={error} onRetry={() => void load()} title="Learning progress unavailable" />;
  if (!space) return <LearningState title="Learning space not found" />;

  return <div className="space-y-6">
    <Link className="text-sm font-medium text-[var(--text-muted)] hover:text-[var(--text-strong)]" params={{ spaceId }} to="/modules/learning/spaces/$spaceId">← {space.title}</Link>
    <div><p className="font-medium text-[var(--text-strong)]">{space.subject_name} · {space.class_group_name}</p><p className="mt-1 text-sm text-[var(--text-muted)]">Assignment completion and released Learning feedback.</p></div>
    {canTeach ? <><TableControlsBar><Input aria-label="Search learner progress" className="sm:w-72" leadingIcon={<Search />} onChange={(event) => onSearchChange({ page: 1, q: event.target.value }, { replace: true })} placeholder="Search learner or number" value={search.q} />{filtered.length ? <TableControlsPagination onNext={() => onSearchChange({ ...search, page: Math.min(totalPages, page + 1) })} onPrevious={() => onSearchChange({ ...search, page: Math.max(1, page - 1) })} page={page} totalPages={totalPages} /> : null}</TableControlsBar><ProgressTable entries={visible} filtered={Boolean(search.q.trim())} /></> : progress[0] ? <PersonalProgress entry={progress[0]} /> : <LearningState description="Progress appears after assignments are published to your roster." title="No progress yet" />}
  </div>;
}

function ProgressTable({ entries, filtered }: { entries: LearningProgressEntry[]; filtered: boolean }) {
  return <TableWrap>{entries.length === 0 ? <TableEmpty description={filtered ? "Change the current search." : "Progress appears after assignments are published."} icon={<TrendingUp />} title={filtered ? "No learners match" : "No progress yet"} /> : <TableScroll><Table className="min-w-[980px]"><THead><tr><TH>Learner</TH><TH>Completion</TH><TH>Not started</TH><TH>Awaiting feedback</TH><TH>Revision requested</TH><TH>Graded</TH><TH>Learning score</TH></tr></THead><TBody>{entries.map((entry) => <TR key={entry.learner_id}><TD><p className="font-medium text-[var(--text-strong)]">{entry.learner_name}</p><p className="mt-1 font-tabular text-xs text-[var(--text-muted)]">{entry.learner_number}</p></TD><TD className="font-tabular font-semibold">{entry.completion_percent}%</TD><TD className="font-tabular text-[var(--text-muted)]">{entry.not_started}</TD><TD className="font-tabular text-[var(--text-muted)]">{entry.awaiting_feedback}</TD><TD className="font-tabular text-[var(--text-muted)]">{entry.revision_requested}</TD><TD className="font-tabular text-[var(--text-muted)]">{entry.graded}</TD><TD className="font-tabular text-[var(--text-muted)]">{formatHundredths(entry.earned_score_hundredths)} / {formatHundredths(entry.possible_score_hundredths)}</TD></TR>)}</TBody></Table></TableScroll>}</TableWrap>;
}

function PersonalProgress({ entry }: { entry: LearningProgressEntry }) {
  const facts = [["Assignments", entry.total_assignments], ["Completion", `${entry.completion_percent}%`], ["Not started", entry.not_started], ["Drafts", entry.drafts], ["Awaiting feedback", entry.awaiting_feedback], ["Revision requested", entry.revision_requested], ["Graded", entry.graded], ["Overdue", entry.overdue]];
  return <><div className="grid gap-px border border-[var(--border)] bg-[var(--border)] sm:grid-cols-2 lg:grid-cols-4">{facts.map(([label, value]) => <div className="bg-[var(--surface)] p-4" key={label}><p className="text-xs font-semibold uppercase tracking-[0.12em] text-[var(--text-muted)]">{label}</p><p className="mt-2 font-tabular text-2xl font-semibold text-[var(--text-strong)]">{value}</p></div>)}</div><section className="border border-[var(--border)] bg-[var(--surface)] p-5"><h2 className="text-sm font-semibold text-[var(--text-strong)]">Released Learning score</h2><p className="mt-2 font-tabular text-2xl font-semibold text-[var(--text-strong)]">{formatHundredths(entry.earned_score_hundredths)} / {formatHundredths(entry.possible_score_hundredths)}</p><p className="mt-2 text-xs text-[var(--text-muted)]">Formal academic results remain in Gradebook and academic reports.</p></section></>;
}

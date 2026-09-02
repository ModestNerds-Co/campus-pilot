/** Full-page Learning score-transfer proposal worklist and preparation flow. */

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Link, useNavigate } from "@tanstack/react-router";
import { ArrowRightLeft, Loader2 } from "lucide-react";
import toast from "react-hot-toast";

import { Button } from "@/components/ui/button";
import {
  Table, TableControlsBar, TableControlsPagination, TableEmpty, TableError,
  TableLoading, TableScroll, TableWrap, TBody, TD, TH, THead, TR,
} from "@/components/ui/data-table";
import { Label, Select } from "@/components/ui/input";
import { usePageChrome } from "@/modules/admin/layouts/page-chrome";
import { gradebookService } from "@/modules/gradebook/service";
import type { GradebookSheetSummary } from "@/modules/gradebook/types";
import { useAuthStore } from "@/stores/auth-store";

import { learningService, responseMessage } from "./service";
import type {
  LearningAssignment, LearningQuiz, LearningScoreTransferSourceType,
  LearningScoreTransferSummary, LearningSpaceSummary, ScoreTransfersSearch,
} from "./types";
import { formatLearningDateTime, LearningStatusBadge } from "./ui";

export function LearningScoreTransfersWorkspace({
  onSearchChange,
  search,
}: {
  onSearchChange: (next: ScoreTransfersSearch) => void;
  search: ScoreTransfersSearch;
}) {
  const navigate = useNavigate();
  const permissions = useAuthStore((state) => state.user?.permissions ?? []);
  const canTeach = permissions.includes("*") || permissions.includes("learning:teach");
  const [records, setRecords] = useState<LearningScoreTransferSummary[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [totalPages, setTotalPages] = useState(1);
  const [spaces, setSpaces] = useState<LearningSpaceSummary[]>([]);
  const [markSheets, setMarkSheets] = useState<GradebookSheetSummary[]>([]);
  const [spaceId, setSpaceId] = useState("");
  const [sourceValue, setSourceValue] = useState("");
  const [targetId, setTargetId] = useState("");
  const [assignments, setAssignments] = useState<LearningAssignment[]>([]);
  const [quizzes, setQuizzes] = useState<LearningQuiz[]>([]);
  const [optionsLoading, setOptionsLoading] = useState(canTeach);
  const [sourceLoading, setSourceLoading] = useState(false);
  const [preparing, setPreparing] = useState(false);
  const listRequest = useRef(0);
  const sourceRequest = useRef(0);

  const load = useCallback(async () => {
    const requestId = ++listRequest.current;
    setLoading(true);
    setError(null);
    try {
      const response = await learningService.scoreTransfers({
        page: search.page,
        per_page: 25,
        status: search.status === "all" ? undefined : search.status,
      });
      if (!response.success || !response.data) {
        throw new Error(responseMessage(response, "Score transfers could not be loaded"));
      }
      if (requestId !== listRequest.current) return;
      setRecords(response.data.score_transfers);
      setTotalPages(Math.max(1, response.pagination?.total_pages ?? 1));
    } catch (cause) {
      if (requestId !== listRequest.current) return;
      setError(cause instanceof Error ? cause.message : "Score transfers could not be loaded");
    } finally {
      if (requestId === listRequest.current) setLoading(false);
    }
  }, [search.page, search.status]);

  useEffect(() => {
    void load();
    return () => { listRequest.current += 1; };
  }, [load]);

  useEffect(() => {
    if (!canTeach) return;
    let active = true;
    setOptionsLoading(true);
    void Promise.all([
      learningService.spaces({ page: 1, per_page: 100 }),
      gradebookService.listMarkSheets({ page: 1, per_page: 100, status: "draft" }),
    ]).then(([spaceResponse, sheetResponse]) => {
      if (!active) return;
      if (!spaceResponse.success || !spaceResponse.data) {
        throw new Error(responseMessage(spaceResponse, "Learning spaces could not be loaded"));
      }
      if (!sheetResponse.success || !sheetResponse.data) {
        throw new Error(responseMessage(sheetResponse, "Draft mark sheets could not be loaded"));
      }
      setSpaces(spaceResponse.data.spaces);
      setMarkSheets(sheetResponse.data.mark_sheets);
    }).catch((cause) => {
      if (active) toast.error(cause instanceof Error ? cause.message : "Transfer options could not be loaded");
    }).finally(() => {
      if (active) setOptionsLoading(false);
    });
    return () => { active = false; };
  }, [canTeach]);

  useEffect(() => {
    setSourceValue("");
    setTargetId("");
    setAssignments([]);
    setQuizzes([]);
    if (!spaceId) return;
    const requestId = ++sourceRequest.current;
    setSourceLoading(true);
    void Promise.all([
      learningService.assignments(spaceId, { page: 1, per_page: 100 }),
      learningService.quizzes(spaceId, { page: 1, per_page: 100 }),
    ]).then(([assignmentResponse, quizResponse]) => {
      if (requestId !== sourceRequest.current) return;
      if (!assignmentResponse.success || !assignmentResponse.data) {
        throw new Error(responseMessage(assignmentResponse, "Assignments could not be loaded"));
      }
      if (!quizResponse.success || !quizResponse.data) {
        throw new Error(responseMessage(quizResponse, "Quizzes could not be loaded"));
      }
      setAssignments(assignmentResponse.data.assignments.filter((item) => item.status !== "draft"));
      setQuizzes(quizResponse.data.quizzes.filter((item) => item.status !== "draft"));
    }).catch((cause) => {
      if (requestId === sourceRequest.current) {
        toast.error(cause instanceof Error ? cause.message : "Learning score sources could not be loaded");
      }
    }).finally(() => {
      if (requestId === sourceRequest.current) setSourceLoading(false);
    });
    return () => { sourceRequest.current += 1; };
  }, [spaceId]);

  const selectedSpace = spaces.find((space) => space.id === spaceId);
  const targets = useMemo(
    () => selectedSpace
      ? markSheets.filter((sheet) => sheet.teaching_assignment_id === selectedSpace.teaching_assignment_id)
      : [],
    [markSheets, selectedSpace],
  );
  const prepare = async () => {
    const [sourceType, sourceId] = sourceValue.split(":") as [LearningScoreTransferSourceType, string];
    if (!sourceType || !sourceId || !targetId || preparing) return;
    setPreparing(true);
    try {
      const response = await learningService.createScoreTransfer(sourceType, sourceId, targetId);
      if (!response.success || !response.data) {
        throw new Error(responseMessage(response, "Score transfer could not be prepared"));
      }
      toast.success("Score transfer prepared for review");
      void navigate({
        to: "/modules/learning/score-transfers/$proposalId",
        params: { proposalId: response.data.id },
      });
    } catch (cause) {
      toast.error(cause instanceof Error ? cause.message : "Score transfer could not be prepared");
    } finally {
      setPreparing(false);
    }
  };

  usePageChrome("Score transfers");
  return <div className="space-y-7">
    {canTeach ? <section className="border border-[var(--border)] bg-[var(--surface)]">
      <header className="border-b border-[var(--border)] p-5 sm:p-6">
        <p className="text-xs font-semibold uppercase tracking-[0.14em] text-[var(--brand-strong)]">Prepare proposal</p>
        <h2 className="mt-2 text-lg font-semibold text-[var(--text-strong)]">Move released Learning scores into Gradebook</h2>
        <p className="mt-2 text-sm text-[var(--text-muted)]">This creates a review record. It does not change formal marks.</p>
      </header>
      <div className="grid gap-5 p-5 sm:p-6 lg:grid-cols-3">
        <div><Label htmlFor="score-transfer-space">Learning space</Label><Select className="mt-1.5" disabled={optionsLoading || preparing} id="score-transfer-space" onChange={(event) => setSpaceId(event.target.value)} value={spaceId}><option value="">Choose a space</option>{spaces.map((space) => <option key={space.id} value={space.id}>{space.title} · {space.class_group_name}</option>)}</Select></div>
        <div><Label htmlFor="score-transfer-source">Score source</Label><Select className="mt-1.5" disabled={!spaceId || sourceLoading || preparing} id="score-transfer-source" onChange={(event) => setSourceValue(event.target.value)} value={sourceValue}><option value="">Choose released work</option>{assignments.map((item) => <option key={item.id} value={`assignment:${item.id}`}>Assignment · {item.title}</option>)}{quizzes.map((item) => <option key={item.id} value={`quiz:${item.id}`}>Quiz · {item.title}</option>)}</Select></div>
        <div><Label htmlFor="score-transfer-target">Draft mark sheet</Label><Select className="mt-1.5" disabled={!spaceId || optionsLoading || preparing} id="score-transfer-target" onChange={(event) => setTargetId(event.target.value)} value={targetId}><option value="">Choose a mark sheet</option>{targets.map((sheet) => <option key={sheet.id} value={sheet.id}>{sheet.assessment_component_name} · {sheet.maximum_marks} marks</option>)}</Select></div>
      </div>
      <footer className="flex flex-wrap items-center justify-between gap-3 border-t border-[var(--border)] bg-[var(--surface-muted)] px-5 py-4 sm:px-6"><p className="text-xs text-[var(--text-muted)]">A different Academic Manager must apply the proposal.</p><Button disabled={!sourceValue || !targetId || preparing} onClick={() => void prepare()}>{preparing ? <Loader2 className="size-4 animate-spin" /> : <ArrowRightLeft className="size-4" />}{preparing ? "Preparing…" : "Prepare proposal"}</Button></footer>
    </section> : null}

    <section aria-labelledby="score-transfer-history">
      <div className="mb-4"><h2 className="text-lg font-semibold text-[var(--text-strong)]" id="score-transfer-history">Transfer history</h2><p className="mt-1 text-sm text-[var(--text-muted)]">Pending and completed review records.</p></div>
      <TableControlsBar><Select aria-label="Transfer status" className="sm:w-48" onChange={(event) => onSearchChange({ page: 1, status: event.target.value as ScoreTransfersSearch["status"] })} value={search.status}><option value="all">All statuses</option><option value="pending">Pending</option><option value="applied">Applied</option><option value="rejected">Rejected</option></Select>{!loading && records.length ? <TableControlsPagination onNext={() => onSearchChange({ ...search, page: Math.min(totalPages, search.page + 1) })} onPrevious={() => onSearchChange({ ...search, page: Math.max(1, search.page - 1) })} page={search.page} totalPages={totalPages} /> : null}</TableControlsBar>
      <TableWrap>{loading ? <TableLoading columns={7} label="Loading score transfers…" /> : error ? <TableError description={error} onRetry={() => void load()} /> : records.length === 0 ? <TableEmpty description={search.status === "all" ? "Prepare a proposal from released assignment feedback or submitted quiz attempts." : "Change the status filter."} icon={<ArrowRightLeft />} title="No score transfers" /> : <TableScroll><Table className="min-w-[980px]"><THead><tr><TH>Source</TH><TH>Class</TH><TH>Gradebook target</TH><TH>Ready</TH><TH>Exceptions</TH><TH>Status</TH><TH>Prepared</TH></tr></THead><TBody>{records.map((record) => <TR key={record.id}><TD><Link className="font-semibold text-[var(--text-strong)] hover:text-[var(--brand-strong)] hover:underline" params={{ proposalId: record.id }} to="/modules/learning/score-transfers/$proposalId">{record.source_title}</Link><p className="mt-1 text-xs text-[var(--text-muted)]">{label(record.source_type)} · {record.subject_name}</p></TD><TD>{record.class_group_name}</TD><TD>{record.target_assessment_name}<p className="mt-1 text-xs text-[var(--text-muted)]">{record.target_maximum_marks} marks</p></TD><TD className="font-tabular">{record.ready_count}</TD><TD className="font-tabular text-[var(--text-muted)]">{record.missing_source_count + record.target_already_marked_count}</TD><TD><LearningStatusBadge status={record.status} /></TD><TD className="text-[var(--text-muted)]">{record.proposed_by_name}<br /><span className="text-xs">{formatLearningDateTime(record.proposed_at)}</span></TD></TR>)}</TBody></Table></TableScroll>}</TableWrap>
    </section>
  </div>;
}

function label(value: string) {
  return value.replace(/_/g, " ").replace(/^./, (letter) => letter.toUpperCase());
}

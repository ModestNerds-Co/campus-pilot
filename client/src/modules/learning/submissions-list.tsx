import { useCallback, useEffect, useRef, useState } from "react";
import { Link } from "@tanstack/react-router";
import { Inbox } from "lucide-react";

import {
  Table, TableControlsBar, TableControlsPagination, TableEmpty, TableError,
  TableLoading, TableScroll, TableWrap, TBody, TD, TH, THead, TR,
} from "@/components/ui/data-table";
import { Select } from "@/components/ui/input";

import { learningService, responseMessage } from "./service";
import type { LearningAssignment, LearningSubmission, LearningSubmissionStatus } from "./types";
import { formatLearningDateTime, LearningStatusBadge } from "./ui";

export interface SubmissionListSearchState {
  page: number;
  status: "all" | LearningSubmissionStatus;
}

export function AssignmentSubmissions({ assignment, onSearchChange, search }: {
  assignment: LearningAssignment;
  onSearchChange: (next: SubmissionListSearchState) => void;
  search: SubmissionListSearchState;
}) {
  const [submissions, setSubmissions] = useState<LearningSubmission[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [totalPages, setTotalPages] = useState(1);
  const requestRef = useRef(0);

  const load = useCallback(async () => {
    const requestId = ++requestRef.current;
    setLoading(true);
    setError(null);
    try {
      const response = await learningService.submissions(assignment.id, {
        page: search.page,
        per_page: 25,
        status: search.status === "all" ? undefined : search.status,
      });
      if (!response.success || !response.data) throw new Error(responseMessage(response, "Submissions could not be loaded"));
      if (requestId !== requestRef.current) return;
      setSubmissions(response.data.submissions);
      setTotalPages(Math.max(1, response.pagination?.total_pages ?? 1));
    } catch (loadError) {
      if (requestId !== requestRef.current) return;
      setError(loadError instanceof Error ? loadError.message : "Submissions could not be loaded");
    } finally {
      if (requestId === requestRef.current) setLoading(false);
    }
  }, [assignment.id, search.page, search.status]);

  useEffect(() => {
    void load();
    return () => { requestRef.current += 1; };
  }, [load]);

  const notStarted = Math.max(0, assignment.recipient_count - assignment.submission_count);
  return <div className="space-y-5">
    <div className="grid gap-px border border-[var(--border)] bg-[var(--border)] sm:grid-cols-3">
      <Fact label="Recipients" value={assignment.recipient_count} />
      <Fact label="Started" value={assignment.submission_count} />
      <Fact label="Not started" value={notStarted} />
    </div>
    <TableControlsBar>
      <Select aria-label="Submission status" className="sm:w-52" onChange={(event) => onSearchChange({ page: 1, status: event.target.value as SubmissionListSearchState["status"] })} value={search.status}>
        <option value="all">All started work</option><option value="draft">Draft</option><option value="submitted">Awaiting feedback</option><option value="revision_requested">Revision requested</option><option value="graded">Graded</option>
      </Select>
      {!loading && submissions.length ? <TableControlsPagination onNext={() => onSearchChange({ ...search, page: Math.min(totalPages, search.page + 1) })} onPrevious={() => onSearchChange({ ...search, page: Math.max(1, search.page - 1) })} page={search.page} totalPages={totalPages} /> : null}
    </TableControlsBar>
    <TableWrap>
      {loading ? <TableLoading columns={4} label="Loading submissions…" /> : error ? <TableError description={error} onRetry={() => void load()} /> : submissions.length === 0 ? <TableEmpty description={search.status === "all" ? "Learner work will appear after a draft is started." : "Change the current status filter."} icon={<Inbox />} title={search.status === "all" ? "No started submissions" : "No submissions match"} /> : <TableScroll><Table className="min-w-[720px]"><THead><tr><TH>Learner</TH><TH>Status</TH><TH>Versions</TH><TH>Updated</TH></tr></THead><TBody>
        {submissions.map((submission) => <TR key={submission.id}><TD><Link className="font-semibold text-[var(--text-strong)] hover:text-[var(--brand-strong)] hover:underline" params={{ submissionId: submission.id }} search={{ version: submission.current_submission_version_id ?? "" }} to="/modules/learning/submissions/$submissionId">{submission.learner_name}</Link><p className="mt-1 font-tabular text-xs text-[var(--text-muted)]">{submission.learner_number}</p></TD><TD><LearningStatusBadge status={submission.status} /></TD><TD className="font-tabular text-[var(--text-muted)]">{submission.versions.length}</TD><TD className="whitespace-nowrap text-[var(--text-muted)]">{formatLearningDateTime(submission.updated_at)}</TD></TR>)}
      </TBody></Table></TableScroll>}
    </TableWrap>
    {notStarted > 0 ? <p className="text-sm text-[var(--text-muted)]">{notStarted} {notStarted === 1 ? "recipient has" : "recipients have"} not started work. Learner names appear after they begin work.</p> : null}
  </div>;
}

function Fact({ label, value }: { label: string; value: number }) {
  return <div className="bg-[var(--surface)] p-4"><p className="text-xs font-semibold uppercase tracking-[0.12em] text-[var(--text-muted)]">{label}</p><p className="mt-2 font-tabular text-2xl font-semibold text-[var(--text-strong)]">{value}</p></div>;
}

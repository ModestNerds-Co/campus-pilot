// Cross-engagement Internal Audit finding register.

import { useCallback, useEffect, useState } from "react";
import { Link } from "@tanstack/react-router";
import { Search, ShieldAlert } from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Table, TableControlsBar, TableControlsPagination, TableControlsSearch, TableEmpty,
  TableError, TableLoading, TableScroll, TableWrap, TBody, TD, TH, THead, TR,
} from "@/components/ui/data-table";
import { Input, Select } from "@/components/ui/input";
import { usePageChrome } from "@/modules/admin/layouts/page-chrome";

import { internalAuditService, responseMessage } from "./service";
import type { AuditFinding, FindingRating } from "./types";
import { dateTime, label, tone } from "./ui";

export function InternalAuditFindingsWorkspace() {
  const [records, setRecords] = useState<AuditFinding[]>([]);
  const [search, setSearch] = useState("");
  const [submittedSearch, setSubmittedSearch] = useState("");
  const [status, setStatus] = useState("all");
  const [rating, setRating] = useState("all");
  const [page, setPage] = useState(1);
  const [totalPages, setTotalPages] = useState(1);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const response = await internalAuditService.findings({
        page,
        per_page: 25,
        search: submittedSearch || undefined,
        status: status === "all" ? undefined : status,
        rating: rating === "all" ? undefined : rating as FindingRating,
      });
      if (!response.success || !response.data) throw new Error(responseMessage(response, "Findings could not be loaded"));
      setRecords(response.data.findings);
      setTotalPages(response.pagination?.total_pages ?? 1);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : "Findings could not be loaded");
    } finally {
      setLoading(false);
    }
  }, [page, rating, status, submittedSearch]);

  useEffect(() => { void load(); }, [load]);
  usePageChrome("Findings");

  const filtered = Boolean(submittedSearch || status !== "all" || rating !== "all");
  return <div className="space-y-6">
    <TableControlsBar>
      <TableControlsSearch onSubmit={(event) => { event.preventDefault(); setPage(1); setSubmittedSearch(search.trim()); }}>
        <Input aria-label="Search audit findings" leadingIcon={<Search />} onChange={(event) => setSearch(event.target.value)} placeholder="Search reference, title, or engagement" value={search} />
        <Button type="submit" variant="secondary">Search</Button>
      </TableControlsSearch>
      <Select aria-label="Finding rating" className="sm:w-40" onChange={(event) => { setPage(1); setRating(event.target.value); }} value={rating}>
        <option value="all">All ratings</option><option value="critical">Critical</option><option value="high">High</option><option value="moderate">Moderate</option><option value="low">Low</option>
      </Select>
      <Select aria-label="Finding status" className="sm:w-40" onChange={(event) => { setPage(1); setStatus(event.target.value); }} value={status}>
        <option value="all">All statuses</option><option value="draft">Draft</option><option value="issued">Issued</option>
      </Select>
      {!loading && records.length > 0 ? <TableControlsPagination onNext={() => setPage((value) => Math.min(totalPages, value + 1))} onPrevious={() => setPage((value) => Math.max(1, value - 1))} page={page} totalPages={totalPages} /> : null}
    </TableControlsBar>
    <TableWrap>{loading ? <TableLoading columns={5} label="Loading findings…" /> : error ? <TableError description={error} onRetry={() => void load()} /> : records.length === 0 ? <TableEmpty description={filtered ? "Change the current filters." : "Findings created during fieldwork will appear here."} icon={<ShieldAlert />} title={filtered ? "No findings match" : "No audit findings yet"} /> : <TableScroll><Table className="min-w-[900px]"><THead><tr><TH>Finding</TH><TH>Engagement</TH><TH>Rating</TH><TH>Status</TH><TH>Updated</TH></tr></THead><TBody>{records.map((record) => <TR key={record.id}>
      <TD><p className="font-semibold text-[var(--text-strong)]">{record.reference}</p><p className="mt-1 max-w-80 truncate text-sm">{record.title}</p></TD>
      <TD><Link className="font-medium text-[var(--brand-strong)] hover:underline" params={{ engagementId: record.engagement_id }} to="/modules/internal-audit/engagements/$engagementId">{record.engagement_reference}</Link><p className="mt-1 max-w-64 truncate text-xs text-[var(--text-muted)]">{record.engagement_title}</p></TD>
      <TD><Badge tone={tone(record.rating)}>{label(record.rating)}</Badge></TD><TD><Badge tone={tone(record.status)}>{label(record.status)}</Badge></TD><TD>{dateTime(record.updated_at)}</TD>
    </TR>)}</TBody></Table></TableScroll>}</TableWrap>
  </div>;
}

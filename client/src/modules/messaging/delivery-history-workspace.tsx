import { useCallback, useEffect, useState } from "react";
import { Link } from "@tanstack/react-router";
import { RadioTower } from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Table, TableControlsPagination, TableEmpty, TableError, TableLoading, TableScroll, TableWrap, TBody, TD, TH, THead, TR } from "@/components/ui/data-table";
import { usePageChrome } from "@/modules/admin/layouts/page-chrome";
import { displayValue, formatDateTime, statusTone } from "./announcements-workspace";
import { communicationService, responseMessage } from "./service";
import type { AnnouncementSummary } from "./types";

export function DeliveryHistoryWorkspace() {
  const [items, setItems] = useState<AnnouncementSummary[]>([]); const [loading, setLoading] = useState(true); const [error, setError] = useState<string | null>(null); const [page, setPage] = useState(1); const [totalPages, setTotalPages] = useState(1);
  const load = useCallback(async () => { setLoading(true); setError(null); try { const response = await communicationService.listAnnouncements({ page, per_page: 25 }); if (!response.success || !response.data) throw new Error(responseMessage(response, "Delivery history could not be loaded")); setItems(response.data.announcements.filter((item) => item.status === "published" || item.status === "cancelled")); setTotalPages(response.pagination?.total_pages ?? 1); } catch (loadError) { setError(loadError instanceof Error ? loadError.message : "Delivery history could not be loaded"); } finally { setLoading(false); } }, [page]);
  useEffect(() => { void load(); }, [load]); usePageChrome("Delivery history");
  return <div className="space-y-6"><p className="text-sm text-[var(--text-muted)]">In-app delivery and read status for published announcements.</p>{!loading && items.length > 0 ? <div className="flex justify-end"><TableControlsPagination onNext={() => setPage((value) => Math.min(totalPages, value + 1))} onPrevious={() => setPage((value) => Math.max(1, value - 1))} page={page} totalPages={totalPages} /></div> : null}<TableWrap>{loading ? <TableLoading columns={5} label="Loading delivery history…" /> : error ? <TableError description={error} onRetry={() => void load()} /> : items.length === 0 ? <TableEmpty description="Published announcements will appear here." icon={<RadioTower />} title="No delivery history" /> : <TableScroll><Table><THead><tr><TH>Announcement</TH><TH>Status</TH><TH>Recipients</TH><TH>Read</TH><TH>Published</TH></tr></THead><TBody>{items.map((item) => <TR key={item.id}><TD><Link className="font-medium text-[var(--text-strong)] hover:text-[var(--brand-strong)] hover:underline" params={{ announcementId: item.id }} search={{ filter: "all", page: 1, q: "", status: "all" }} to="/modules/messaging/announcements/$announcementId">{item.title}</Link></TD><TD><Badge tone={statusTone(item.status)}>{displayValue(item.status)}</Badge></TD><TD className="font-tabular">{item.recipient_count}</TD><TD className="font-tabular">{item.read_count} / {item.recipient_count}</TD><TD>{item.published_at ? formatDateTime(item.published_at) : "—"}</TD></TR>)}</TBody></Table></TableScroll>}</TableWrap></div>;
}

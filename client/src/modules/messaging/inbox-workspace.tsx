import { useCallback, useEffect, useState } from "react";
import { Inbox, Loader2 } from "lucide-react";
import toast from "react-hot-toast";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Table, TableControlsBar, TableControlsPagination, TableEmpty, TableError, TableLoading, TableScroll, TableWrap, TBody, TD, TH, THead, TR } from "@/components/ui/data-table";
import { DialogBody, DialogFooter, DialogHeader, DialogShell } from "@/components/ui/dialog";
import { Select } from "@/components/ui/input";
import { usePageChrome } from "@/modules/admin/layouts/page-chrome";

import { displayValue, formatDateTime, priorityTone } from "./announcements-workspace";
import { communicationService, responseMessage } from "./service";
import type { InboxItem } from "./types";

export function CommunicationInboxWorkspace() {
  const [messages, setMessages] = useState<InboxItem[]>([]); const [loading, setLoading] = useState(true); const [error, setError] = useState<string | null>(null);
  const [page, setPage] = useState(1); const [totalPages, setTotalPages] = useState(1); const [filter, setFilter] = useState<"all" | "unread">("all"); const [selected, setSelected] = useState<InboxItem | null>(null);
  const load = useCallback(async () => { setLoading(true); setError(null); try { const response = await communicationService.inbox({ page, per_page: 25, unread_only: filter === "unread" || undefined }); if (!response.success || !response.data) throw new Error(responseMessage(response, "Inbox could not be loaded")); setMessages(response.data.messages); setTotalPages(response.pagination?.total_pages ?? 1); } catch (loadError) { setError(loadError instanceof Error ? loadError.message : "Inbox could not be loaded"); } finally { setLoading(false); } }, [filter, page]);
  useEffect(() => { void load(); }, [load]); usePageChrome("Inbox");
  const openMessage = async (message: InboxItem) => { setSelected(message); if (message.read_at) return; try { const response = await communicationService.markRead(message.delivery_id); if (!response.success || !response.data) throw new Error(responseMessage(response, "Message could not be marked as read")); setSelected(response.data); setMessages((current) => current.map((item) => item.delivery_id === response.data?.delivery_id ? response.data : item)); } catch (readError) { toast.error(readError instanceof Error ? readError.message : "Message could not be marked as read"); } };

  return <div className="space-y-6"><p className="text-sm text-[var(--text-muted)]">Announcements delivered to your account.</p>
    <TableControlsBar><Select aria-label="Inbox filter" className="sm:w-44" onChange={(event) => { setPage(1); setFilter(event.target.value as "all" | "unread"); }} value={filter}><option value="all">All messages</option><option value="unread">Unread</option></Select>{!loading && messages.length > 0 ? <TableControlsPagination onNext={() => setPage((value) => Math.min(totalPages, value + 1))} onPrevious={() => setPage((value) => Math.max(1, value - 1))} page={page} totalPages={totalPages} /> : null}</TableControlsBar>
    <TableWrap>{loading ? <TableLoading columns={4} label="Loading inbox…" /> : error ? <TableError description={error} onRetry={() => void load()} /> : messages.length === 0 ? <TableEmpty description={filter === "unread" ? "There are no unread messages." : "Published announcements will appear here."} icon={<Inbox />} title={filter === "unread" ? "No unread messages" : "Inbox is empty"} /> : <TableScroll><Table><THead><tr><TH>Message</TH><TH>Priority</TH><TH>From</TH><TH>Published</TH></tr></THead><TBody>{messages.map((message) => <TR className="cursor-pointer" key={message.delivery_id} onClick={() => void openMessage(message)}><TD><button className={`text-left text-sm text-[var(--text-strong)] hover:text-[var(--brand-strong)] ${message.read_at ? "font-medium" : "font-bold"}`} type="button">{message.title}</button>{!message.read_at ? <span className="ml-2 inline-block size-2 rounded-full bg-[var(--brand-strong)]" title="Unread" /> : null}</TD><TD><Badge tone={priorityTone(message.priority)}>{displayValue(message.priority)}</Badge></TD><TD className="text-[var(--text-muted)]">{message.sender_name}</TD><TD className="whitespace-nowrap text-[var(--text-muted)]">{formatDateTime(message.published_at)}</TD></TR>)}</TBody></Table></TableScroll>}</TableWrap>
    <MessageDrawer message={selected} onClose={() => setSelected(null)} />
  </div>;
}

function MessageDrawer({ message, onClose }: { message: InboxItem | null; onClose: () => void }) { return <DialogShell onClose={onClose} open={Boolean(message)} panelClassName="sm:max-w-[680px]"><DialogHeader onClose={onClose} title={message?.title ?? "Message"} /><DialogBody>{message ? <article><div className="flex flex-wrap items-center gap-2"><Badge tone={priorityTone(message.priority)}>{displayValue(message.priority)}</Badge><span className="text-xs text-[var(--text-muted)]">{message.sender_name} · {formatDateTime(message.published_at)}</span></div><p className="mt-6 whitespace-pre-wrap text-sm leading-7 text-[var(--text-strong)]">{message.body}</p></article> : <div className="flex justify-center py-12"><Loader2 className="size-5 animate-spin" /></div>}</DialogBody><DialogFooter><Button onClick={onClose} type="button">Close</Button></DialogFooter></DialogShell>; }

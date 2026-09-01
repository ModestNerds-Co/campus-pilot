// Personal Communication inbox with URL-owned filtering and cancellation state.
import { useCallback, useEffect, useRef, useState } from "react";
import { Inbox, Loader2 } from "lucide-react";
import toast from "react-hot-toast";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Table,
  TableControlsBar,
  TableControlsPagination,
  TableEmpty,
  TableError,
  TableLoading,
  TableScroll,
  TableWrap,
  TBody,
  TD,
  TH,
  THead,
  TR,
} from "@/components/ui/data-table";
import { DialogBody, DialogFooter, DialogHeader, DialogShell } from "@/components/ui/dialog";
import { Select } from "@/components/ui/input";
import { usePageChrome } from "@/modules/admin/layouts/page-chrome";

import {
  displayValue,
  formatDateTime,
  priorityTone,
  statusTone,
} from "./announcements-workspace";
import { communicationService, responseMessage } from "./service";
import type { InboxItem, InboxListFilter } from "./types";

interface InboxFilters {
  filter: InboxListFilter;
  page: number;
}

export function CommunicationInboxWorkspace({
  filter,
  onFiltersChange,
  page,
}: {
  filter: InboxListFilter;
  onFiltersChange: (next: InboxFilters, options?: { replace?: boolean }) => void;
  page: number;
}) {
  const [messages, setMessages] = useState<InboxItem[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [totalPages, setTotalPages] = useState(1);
  const [selected, setSelected] = useState<InboxItem | null>(null);
  const listRequestRef = useRef(0);
  const messageRequestRef = useRef(0);

  const load = useCallback(async () => {
    const requestId = ++listRequestRef.current;
    setLoading(true);
    setError(null);
    try {
      const response = await communicationService.inbox({
        page,
        per_page: 25,
        unread_only: filter === "unread" || undefined,
      });
      if (!response.success || !response.data) {
        throw new Error(responseMessage(response, "Inbox could not be loaded"));
      }
      if (requestId !== listRequestRef.current) return;
      setMessages(response.data.messages);
      setTotalPages(Math.max(1, response.pagination?.total_pages ?? 1));
    } catch (loadError) {
      if (requestId !== listRequestRef.current) return;
      setError(loadError instanceof Error ? loadError.message : "Inbox could not be loaded");
    } finally {
      if (requestId === listRequestRef.current) setLoading(false);
    }
  }, [filter, page]);

  useEffect(() => {
    void load();
  }, [load]);
  usePageChrome("Inbox");

  const openMessage = async (message: InboxItem) => {
    const requestId = ++messageRequestRef.current;
    setSelected(message);
    if (message.read_at) return;
    try {
      const response = await communicationService.markRead(message.delivery_id);
      if (!response.success || !response.data) {
        throw new Error(responseMessage(response, "Message could not be marked as read"));
      }
      setMessages((current) =>
        current.map((item) =>
          item.delivery_id === response.data?.delivery_id ? response.data : item,
        ),
      );
      if (requestId === messageRequestRef.current) setSelected(response.data);
    } catch (readError) {
      if (requestId !== messageRequestRef.current) return;
      toast.error(
        readError instanceof Error ? readError.message : "Message could not be marked as read",
      );
    }
  };

  const closeMessage = () => {
    messageRequestRef.current += 1;
    setSelected(null);
  };

  return (
    <div className="space-y-6">
      <p className="text-sm text-[var(--text-muted)]">Announcements delivered to your account.</p>
      <TableControlsBar>
        <Select
          aria-label="Inbox filter"
          className="sm:w-44"
          onChange={(event) =>
            onFiltersChange({ filter: event.target.value as InboxListFilter, page: 1 })
          }
          value={filter}
        >
          <option value="all">All messages</option>
          <option value="unread">Unread</option>
        </Select>
        {!loading && messages.length > 0 ? (
          <TableControlsPagination
            onNext={() =>
              onFiltersChange({ filter, page: Math.min(totalPages, page + 1) })
            }
            onPrevious={() => onFiltersChange({ filter, page: Math.max(1, page - 1) })}
            page={page}
            totalPages={totalPages}
          />
        ) : null}
      </TableControlsBar>
      <TableWrap>
        {loading ? (
          <TableLoading columns={4} label="Loading inbox…" />
        ) : error ? (
          <TableError description={error} onRetry={() => void load()} />
        ) : messages.length === 0 ? (
          <TableEmpty
            description={
              filter === "unread"
                ? "There are no unread messages."
                : "Published announcements will appear here."
            }
            icon={<Inbox />}
            title={filter === "unread" ? "No unread messages" : "Inbox is empty"}
          />
        ) : (
          <TableScroll>
            <Table>
              <THead>
                <tr>
                  <TH>Message</TH>
                  <TH>Priority</TH>
                  <TH>From</TH>
                  <TH>Published</TH>
                </tr>
              </THead>
              <TBody>
                {messages.map((message) => (
                  <TR
                    className="cursor-pointer"
                    key={message.delivery_id}
                    onClick={() => void openMessage(message)}
                  >
                    <TD>
                      <div className="flex flex-wrap items-center gap-2">
                        <button
                          className={`text-left text-sm text-[var(--text-strong)] hover:text-[var(--brand-strong)] ${message.read_at ? "font-medium" : "font-bold"}`}
                          type="button"
                        >
                          {message.title}
                        </button>
                        {!message.read_at ? (
                          <span
                            aria-label="Unread"
                            className="inline-block size-2 rounded-full bg-[var(--brand-strong)]"
                            role="img"
                          />
                        ) : null}
                        {message.announcement_status === "cancelled" ? (
                          <Badge tone="danger">Cancelled</Badge>
                        ) : null}
                      </div>
                    </TD>
                    <TD>
                      <Badge tone={priorityTone(message.priority)}>
                        {displayValue(message.priority)}
                      </Badge>
                    </TD>
                    <TD className="text-[var(--text-muted)]">{message.sender_name}</TD>
                    <TD className="whitespace-nowrap text-[var(--text-muted)]">
                      {formatDateTime(message.published_at)}
                    </TD>
                  </TR>
                ))}
              </TBody>
            </Table>
          </TableScroll>
        )}
      </TableWrap>
      <MessageDrawer message={selected} onClose={closeMessage} />
    </div>
  );
}

function MessageDrawer({ message, onClose }: { message: InboxItem | null; onClose: () => void }) {
  return (
    <DialogShell onClose={onClose} open={Boolean(message)} panelClassName="sm:max-w-[680px]">
      <DialogHeader onClose={onClose} title={message?.title ?? "Message"} />
      <DialogBody>
        {message ? (
          <article>
            <div className="flex flex-wrap items-center gap-2">
              <Badge tone={priorityTone(message.priority)}>{displayValue(message.priority)}</Badge>
              {message.announcement_status === "cancelled" ? (
                <Badge tone={statusTone(message.announcement_status)}>Cancelled</Badge>
              ) : null}
              <span className="text-xs text-[var(--text-muted)]">
                {message.sender_name} · {formatDateTime(message.published_at)}
              </span>
            </div>
            {message.announcement_status === "cancelled" ? (
              <div className="mt-5 border-l-4 border-[var(--tone-danger)] bg-[var(--tone-danger-bg)] p-4">
                <p className="text-xs font-semibold uppercase tracking-[0.12em] text-[var(--tone-danger)]">
                  Cancelled
                </p>
                <p className="mt-2 text-sm leading-6 text-[var(--text-strong)]">
                  {message.cancellation_reason || "This announcement was cancelled."}
                </p>
              </div>
            ) : null}
            <p className="mt-6 whitespace-pre-wrap text-sm leading-7 text-[var(--text-strong)]">
              {message.body}
            </p>
          </article>
        ) : (
          <div className="flex justify-center py-12">
            <Loader2 className="size-5 animate-spin" />
          </div>
        )}
      </DialogBody>
      <DialogFooter>
        <Button onClick={onClose} type="button">
          Close
        </Button>
      </DialogFooter>
    </DialogShell>
  );
}

// Permission-aware Communication landing page and URL-owned announcement list.
import { useCallback, useEffect, useRef, useState } from "react";
import { Link } from "@tanstack/react-router";
import { MessageSquareText, Plus, Search } from "lucide-react";

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
import { Input, Select } from "@/components/ui/input";
import { usePageChrome } from "@/modules/admin/layouts/page-chrome";
import { useAuthStore } from "@/stores/auth-store";

import { AnnouncementDrawer } from "./announcement-drawer";
import { CommunicationInboxWorkspace } from "./inbox-workspace";
import { communicationService, responseMessage } from "./service";
import type {
  AnnouncementDetail,
  AnnouncementStatus,
  AnnouncementSummary,
  CommunicationReferenceData,
  MessagingListSearch,
} from "./types";

export type MessagingSearchChange = (
  next: MessagingListSearch,
  options?: { replace?: boolean },
) => void;

export function CommunicationHome({
  onSearchChange,
  search,
}: {
  onSearchChange: MessagingSearchChange;
  search: MessagingListSearch;
}) {
  const permissions = useAuthStore((state) => state.user?.permissions ?? []);
  const canCompose = permissions.includes("*") || permissions.includes("messaging:create");
  return canCompose ? (
    <AnnouncementsWorkspace onSearchChange={onSearchChange} search={search} />
  ) : (
    <CommunicationInboxWorkspace
      filter={search.filter}
      onFiltersChange={(next, options) =>
        onSearchChange({ ...search, ...next }, options)
      }
      page={search.page}
    />
  );
}

export function AnnouncementsWorkspace({
  onSearchChange,
  search,
}: {
  onSearchChange: MessagingSearchChange;
  search: MessagingListSearch;
}) {
  const permissions = useAuthStore((state) => state.user?.permissions ?? []);
  const canCreate = permissions.includes("*") || permissions.includes("messaging:create");
  const [items, setItems] = useState<AnnouncementSummary[]>([]);
  const [references, setReferences] = useState<CommunicationReferenceData | null>(null);
  const [referencesLoading, setReferencesLoading] = useState(false);
  const [referencesError, setReferencesError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [totalPages, setTotalPages] = useState(1);
  const [drawerOpen, setDrawerOpen] = useState(false);
  const listRequestRef = useRef(0);
  const referenceRequestRef = useRef(0);

  const load = useCallback(async () => {
    const requestId = ++listRequestRef.current;
    setLoading(true);
    setError(null);
    try {
      const response = await communicationService.listAnnouncements({
        page: search.page,
        per_page: 25,
        status: search.status === "all" ? undefined : search.status,
        search: search.q.trim() || undefined,
      });
      if (!response.success || !response.data) {
        throw new Error(responseMessage(response, "Announcements could not be loaded"));
      }
      if (requestId !== listRequestRef.current) return;
      setItems(response.data.announcements);
      setTotalPages(Math.max(1, response.pagination?.total_pages ?? 1));
    } catch (loadError) {
      if (requestId !== listRequestRef.current) return;
      setError(
        loadError instanceof Error ? loadError.message : "Announcements could not be loaded",
      );
    } finally {
      if (requestId === listRequestRef.current) setLoading(false);
    }
  }, [search.page, search.q, search.status]);

  const loadReferences = useCallback(async () => {
    if (!canCreate) return;
    const requestId = ++referenceRequestRef.current;
    setReferencesLoading(true);
    setReferencesError(null);
    try {
      const response = await communicationService.references();
      if (!response.success || !response.data) {
        throw new Error(responseMessage(response, "Audiences could not be loaded"));
      }
      if (requestId === referenceRequestRef.current) setReferences(response.data);
    } catch (referenceError) {
      if (requestId !== referenceRequestRef.current) return;
      setReferencesError(
        referenceError instanceof Error
          ? referenceError.message
          : "Audiences could not be loaded",
      );
    } finally {
      if (requestId === referenceRequestRef.current) setReferencesLoading(false);
    }
  }, [canCreate]);

  useEffect(() => {
    void load();
  }, [load]);
  useEffect(() => {
    void loadReferences();
  }, [loadReferences]);

  usePageChrome(
    "Announcements",
    canCreate ? (
      <Button onClick={() => setDrawerOpen(true)}>
        <Plus className="size-4" /> New announcement
      </Button>
    ) : null,
  );
  const filtered = Boolean(search.q.trim() || search.status !== "all");

  return (
    <div className="space-y-6">
      <p className="text-sm text-[var(--text-muted)]">
        Prepare, review, and publish school announcements.
      </p>
      <TableControlsBar>
        <Input
          aria-label="Search announcements"
          className="sm:w-72"
          leadingIcon={<Search />}
          onChange={(event) =>
            onSearchChange(
              { ...search, page: 1, q: event.target.value },
              { replace: true },
            )
          }
          placeholder="Search title or message"
          value={search.q}
        />
        <Select
          aria-label="Status filter"
          className="sm:w-44"
          onChange={(event) =>
            onSearchChange({
              ...search,
              page: 1,
              status: event.target.value as "all" | AnnouncementStatus,
            })
          }
          value={search.status}
        >
          <option value="all">All statuses</option>
          <option value="draft">Draft</option>
          <option value="submitted">Submitted</option>
          <option value="published">Published</option>
          <option value="cancelled">Cancelled</option>
        </Select>
        {!loading && items.length > 0 ? (
          <TableControlsPagination
            onNext={() =>
              onSearchChange({ ...search, page: Math.min(totalPages, search.page + 1) })
            }
            onPrevious={() =>
              onSearchChange({ ...search, page: Math.max(1, search.page - 1) })
            }
            page={search.page}
            totalPages={totalPages}
          />
        ) : null}
      </TableControlsBar>
      <TableWrap>
        {loading ? (
          <TableLoading columns={6} label="Loading announcements…" />
        ) : error ? (
          <TableError description={error} onRetry={() => void load()} />
        ) : items.length === 0 ? (
          <TableEmpty
            description={
              filtered
                ? "Change the current filters."
                : canCreate
                  ? "Create the first announcement draft."
                  : "No announcements are available."
            }
            icon={<MessageSquareText />}
            title={filtered ? "No announcements match" : "No announcements yet"}
          />
        ) : (
          <TableScroll>
            <Table className="min-w-[820px]">
              <THead>
                <tr>
                  <TH>Announcement</TH>
                  <TH>Priority</TH>
                  <TH>Status</TH>
                  <TH>Recipients</TH>
                  <TH>Read</TH>
                  <TH>Updated</TH>
                </tr>
              </THead>
              <TBody>
                {items.map((item) => (
                  <TR key={item.id}>
                    <TD>
                      <Link
                        className="font-medium text-[var(--text-strong)] hover:text-[var(--brand-strong)] hover:underline"
                        params={{ announcementId: item.id }}
                        search={search}
                        to="/modules/messaging/announcements/$announcementId"
                      >
                        {item.title}
                      </Link>
                      <p className="mt-1 text-xs text-[var(--text-muted)]">
                        {item.creator_name}
                      </p>
                    </TD>
                    <TD>
                      <Badge tone={priorityTone(item.priority)}>
                        {displayValue(item.priority)}
                      </Badge>
                    </TD>
                    <TD>
                      <Badge tone={statusTone(item.status)}>{displayValue(item.status)}</Badge>
                    </TD>
                    <TD className="font-tabular text-[var(--text-muted)]">
                      {item.recipient_count || "—"}
                    </TD>
                    <TD className="font-tabular text-[var(--text-muted)]">
                      {item.recipient_count
                        ? `${item.read_count} / ${item.recipient_count}`
                        : "—"}
                    </TD>
                    <TD className="whitespace-nowrap text-[var(--text-muted)]">
                      {formatDateTime(item.updated_at)}
                    </TD>
                  </TR>
                ))}
              </TBody>
            </Table>
          </TableScroll>
        )}
      </TableWrap>
      <AnnouncementDrawer
        announcement={null}
        onClose={() => setDrawerOpen(false)}
        onRetryReferences={() => void loadReferences()}
        onSaved={() => {
          setDrawerOpen(false);
          void load();
        }}
        open={drawerOpen}
        references={references}
        referencesError={referencesError}
        referencesLoading={referencesLoading}
      />
    </div>
  );
}

export function displayValue(value: string) {
  return value.replace(/_/g, " ").replace(/^./, (letter) => letter.toUpperCase());
}

export function formatDateTime(value: string) {
  return new Intl.DateTimeFormat(undefined, {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(new Date(value));
}

export function priorityTone(
  priority: AnnouncementSummary["priority"],
): "neutral" | "warning" | "danger" {
  return priority === "urgent" ? "danger" : priority === "important" ? "warning" : "neutral";
}

export function statusTone(
  status: AnnouncementSummary["status"],
): "neutral" | "warning" | "success" | "danger" {
  return status === "published"
    ? "success"
    : status === "cancelled"
      ? "danger"
      : status === "submitted"
        ? "warning"
        : "neutral";
}

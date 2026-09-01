import { useCallback, useEffect, useState } from "react";
import { Clock3, Loader2, Plus, Search } from "lucide-react";
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
import {
  DialogBody,
  DialogFooter,
  DialogHeader,
  DialogShell,
} from "@/components/ui/dialog";
import { Input, Label, Select, Textarea } from "@/components/ui/input";
import { usePageChrome } from "@/modules/admin/layouts/page-chrome";
import { useAuthStore } from "@/stores/auth-store";

import { libraryService, responseMessage } from "./service";
import { libraryAccessProfile } from "./access";
import type { CopyRecord, Hold, Membership, TitleSummary } from "./types";
import { displayValue, formatDateTime, statusTone } from "./ui";

export function LibraryHoldsWorkspace() {
  const permissions = useAuthStore((state) => state.user?.permissions ?? []);
  const { canBorrow, canCirculate } = libraryAccessProfile(permissions);
  const [holds, setHolds] = useState<Hold[]>([]);
  const [members, setMembers] = useState<Membership[]>([]);
  const [titles, setTitles] = useState<TitleSummary[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [search, setSearch] = useState("");
  const [status, setStatus] = useState("all");
  const [page, setPage] = useState(1);
  const [totalPages, setTotalPages] = useState(1);
  const [placeOpen, setPlaceOpen] = useState(false);
  const [selected, setSelected] = useState<Hold | null>(null);
  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const response = await libraryService.holds({
        page,
        per_page: 25,
        search: search.trim() || undefined,
        status: status === "all" ? undefined : status,
      });
      if (!response.success || !response.data)
        throw new Error(responseMessage(response, "Holds could not be loaded"));
      setHolds(response.data.holds);
      setTotalPages(response.pagination?.total_pages ?? 1);
    } catch (loadError) {
      setError(
        loadError instanceof Error
          ? loadError.message
          : "Holds could not be loaded",
      );
    } finally {
      setLoading(false);
    }
  }, [page, search, status]);
  useEffect(() => {
    void load();
  }, [load]);
  useEffect(() => {
    if (!canBorrow) return;
    void Promise.all([
      libraryService.members({ per_page: 100, status: "active" }),
      libraryService.titles({ per_page: 100, status: "active" }),
    ]).then(([memberResponse, titleResponse]) => {
      if (memberResponse.success)
        setMembers(memberResponse.data?.memberships ?? []);
      if (titleResponse.success) setTitles(titleResponse.data?.titles ?? []);
    });
  }, [canBorrow]);
  usePageChrome(
    canCirculate ? "Reservations" : "My holds",
    canBorrow ? (
      <Button onClick={() => setPlaceOpen(true)}>
        <Plus className="size-4" />
        Place hold
      </Button>
    ) : null,
  );
  const filtered = Boolean(search.trim() || status !== "all");
  return (
    <div className="space-y-6">
      <p className="text-sm text-[var(--text-muted)]">
        {canCirculate
          ? "Reserve titles and manage the pickup queue."
          : "Place and review your title reservations."}
      </p>
      <TableControlsBar>
        <Input
          aria-label="Search holds"
          className="sm:w-72"
          leadingIcon={<Search />}
          onChange={(event) => {
            setPage(1);
            setSearch(event.target.value);
          }}
          placeholder="Search member or title"
          value={search}
        />
        <Select
          aria-label="Hold status"
          className="sm:w-44"
          onChange={(event) => {
            setPage(1);
            setStatus(event.target.value);
          }}
          value={status}
        >
          <option value="all">All statuses</option>
          {["waiting", "ready", "fulfilled", "cancelled", "expired"].map(
            (value) => (
              <option key={value} value={value}>
                {displayValue(value)}
              </option>
            ),
          )}
        </Select>
        {!loading && holds.length > 0 ? (
          <TableControlsPagination
            onNext={() => setPage((value) => Math.min(totalPages, value + 1))}
            onPrevious={() => setPage((value) => Math.max(1, value - 1))}
            page={page}
            totalPages={totalPages}
          />
        ) : null}
      </TableControlsBar>
      <TableWrap>
        {loading ? (
          <TableLoading columns={6} label="Loading holds…" />
        ) : error ? (
          <TableError description={error} onRetry={() => void load()} />
        ) : holds.length === 0 ? (
          <TableEmpty
            description={
              filtered
                ? "Change the current filters."
                : "No title reservations are in this scope."
            }
            icon={<Clock3 />}
            title={filtered ? "No holds match" : "No holds yet"}
          />
        ) : (
          <TableScroll>
            <Table className="min-w-[820px]">
              <THead>
                <tr>
                  <TH>Title</TH>
                  <TH>Member</TH>
                  <TH>Queue</TH>
                  <TH>Copy</TH>
                  <TH>Expires</TH>
                  <TH>Status</TH>
                </tr>
              </THead>
              <TBody>
                {holds.map((hold) => (
                  <TR
                    className="cursor-pointer"
                    key={hold.id}
                    onClick={() => setSelected(hold)}
                  >
                    <TD>
                      <span className="font-medium text-[var(--text-strong)]">
                        {hold.title}
                      </span>
                    </TD>
                    <TD>
                      <span className="text-[var(--text-strong)]">
                        {hold.borrower_name}
                      </span>
                      <p className="mt-1 text-xs text-[var(--text-muted)]">
                        {hold.borrower_number}
                      </p>
                    </TD>
                    <TD className="font-tabular text-[var(--text-muted)]">
                      {hold.queue_position}
                    </TD>
                    <TD className="font-tabular text-[var(--text-muted)]">
                      {hold.accession_number || "—"}
                    </TD>
                    <TD className="whitespace-nowrap text-[var(--text-muted)]">
                      {hold.expires_at ? formatDateTime(hold.expires_at) : "—"}
                    </TD>
                    <TD>
                      <Badge tone={statusTone(hold.status)}>
                        {displayValue(hold.status)}
                      </Badge>
                    </TD>
                  </TR>
                ))}
              </TBody>
            </Table>
          </TableScroll>
        )}
      </TableWrap>
      <PlaceHoldDrawer
        members={members}
        onClose={() => setPlaceOpen(false)}
        onSaved={() => {
          setPlaceOpen(false);
          void load();
        }}
        open={placeOpen}
        titles={titles}
      />
      <HoldDrawer
        canBorrow={canBorrow}
        canCirculate={canCirculate}
        hold={selected}
        onClose={() => setSelected(null)}
        onSaved={(value) => {
          if (value)
            setHolds((current) =>
              current.map((item) => (item.id === value.id ? value : item)),
            );
          setSelected(value);
          void load();
        }}
      />
    </div>
  );
}

function PlaceHoldDrawer({
  members,
  onClose,
  onSaved,
  open,
  titles,
}: {
  members: Membership[];
  onClose: () => void;
  onSaved: () => void;
  open: boolean;
  titles: TitleSummary[];
}) {
  const [titleId, setTitleId] = useState("");
  const [memberId, setMemberId] = useState("");
  const [saving, setSaving] = useState(false);
  useEffect(() => {
    if (open) {
      setTitleId("");
      setMemberId(members.length === 1 ? members[0].id : "");
    }
  }, [members, open]);
  const submit = async (event: React.FormEvent) => {
    event.preventDefault();
    setSaving(true);
    try {
      const response = await libraryService.placeHold(titleId, memberId);
      if (!response.success || !response.data)
        throw new Error(responseMessage(response, "Hold could not be placed"));
      toast.success("Hold placed");
      onSaved();
    } catch (error) {
      toast.error(
        error instanceof Error ? error.message : "Hold could not be placed",
      );
    } finally {
      setSaving(false);
    }
  };
  return (
    <DialogShell onClose={onClose} open={open}>
      <DialogHeader onClose={saving ? undefined : onClose} title="Place hold" />
      <form onSubmit={submit}>
        <DialogBody className="space-y-5">
          <div>
            <Label>Title</Label>
            <Select
              className="mt-1.5"
              data-autofocus="true"
              onChange={(event) => setTitleId(event.target.value)}
              required
              value={titleId}
            >
              <option value="">Choose a title</option>
              {titles.map((title) => (
                <option key={title.id} value={title.id}>
                  {title.title}
                </option>
              ))}
            </Select>
          </div>
          <div>
            <Label>Member</Label>
            <Select
              className="mt-1.5"
              onChange={(event) => setMemberId(event.target.value)}
              required
              value={memberId}
            >
              <option value="">Choose a member</option>
              {members.map((member) => (
                <option key={member.id} value={member.id}>
                  {member.borrower_name} · {member.borrower_number}
                </option>
              ))}
            </Select>
          </div>
        </DialogBody>
        <DialogFooter>
          <Button onClick={onClose} type="button" variant="secondary">
            Cancel
          </Button>
          <Button disabled={saving || !titleId || !memberId} type="submit">
            {saving ? (
              <>
                <Loader2 className="size-4 animate-spin" />
                Placing…
              </>
            ) : (
              "Place hold"
            )}
          </Button>
        </DialogFooter>
      </form>
    </DialogShell>
  );
}

function HoldDrawer({
  canBorrow,
  canCirculate,
  hold,
  onClose,
  onSaved,
}: {
  canBorrow: boolean;
  canCirculate: boolean;
  hold: Hold | null;
  onClose: () => void;
  onSaved: (hold: Hold | null) => void;
}) {
  const [action, setAction] = useState<
    "ready" | "cancel" | "expire" | "checkout" | null
  >(null);
  const [copies, setCopies] = useState<CopyRecord[]>([]);
  const [copyId, setCopyId] = useState("");
  const [expiresAt, setExpiresAt] = useState("");
  const [reason, setReason] = useState("");
  const [saving, setSaving] = useState(false);
  useEffect(() => {
    setAction(null);
    setCopies([]);
    setCopyId(hold?.copy_id ?? "");
    setExpiresAt(defaultExpiry());
    setReason("");
  }, [hold]);
  useEffect(() => {
    if (action !== "ready" || !hold) return;
    void libraryService
      .copies(hold.title_id, { per_page: 100, status: "available" })
      .then((response) => {
        const values = response.success ? (response.data?.copies ?? []) : [];
        setCopies(values);
        setCopyId(values[0]?.id ?? "");
      });
  }, [action, hold]);
  if (!hold) return null;
  const perform = async () => {
    if (!action || saving) return;
    setSaving(true);
    try {
      if (action === "checkout") {
        if (!hold.copy_id)
          throw new Error("The ready hold has no assigned copy");
        const response = await libraryService.checkout({
          copy_id: hold.copy_id,
          membership_id: hold.membership_id,
          fulfilled_hold_id: hold.id,
          checked_out_on: today(),
          notes: null,
        });
        if (!response.success)
          throw new Error(
            responseMessage(response, "Reserved copy could not be checked out"),
          );
        toast.success("Reserved copy checked out");
        onSaved(null);
        onClose();
        return;
      }
      const response =
        action === "ready"
          ? await libraryService.readyHold(
              hold,
              copyId,
              new Date(expiresAt).toISOString(),
            )
          : action === "cancel"
            ? await libraryService.cancelHold(hold, reason)
            : await libraryService.expireHold(hold, reason);
      if (!response.success || !response.data)
        throw new Error(responseMessage(response, "Hold could not be updated"));
      toast.success(
        action === "ready"
          ? "Hold is ready for pickup"
          : action === "cancel"
            ? "Hold cancelled"
            : "Hold expired",
      );
      onSaved(response.data);
      setAction(null);
    } catch (error) {
      toast.error(
        error instanceof Error ? error.message : "Hold could not be updated",
      );
    } finally {
      setSaving(false);
    }
  };
  return (
    <DialogShell onClose={onClose} open>
      <DialogHeader
        onClose={saving ? undefined : onClose}
        title="Library hold"
      />
      <DialogBody className="space-y-6">
        <div className="border border-[var(--border)] bg-[var(--surface-muted)] p-4">
          <div className="flex items-start justify-between gap-4">
            <div>
              <p className="font-semibold text-[var(--text-strong)]">
                {hold.title}
              </p>
              <p className="mt-1 text-sm text-[var(--text-muted)]">
                {hold.borrower_name} · queue {hold.queue_position}
              </p>
            </div>
            <Badge tone={statusTone(hold.status)}>
              {displayValue(hold.status)}
            </Badge>
          </div>
          {hold.accession_number ? (
            <p className="mt-4 font-tabular text-sm text-[var(--text-body)]">
              Copy {hold.accession_number}
            </p>
          ) : null}
        </div>
        {action === "ready" ? (
          <div className="space-y-4">
            <div>
              <Label>Available copy</Label>
              <Select
                className="mt-1.5"
                onChange={(event) => setCopyId(event.target.value)}
                required
                value={copyId}
              >
                <option value="">Choose a copy</option>
                {copies.map((copy) => (
                  <option key={copy.id} value={copy.id}>
                    {copy.accession_number}
                    {copy.location ? ` · ${copy.location}` : ""}
                  </option>
                ))}
              </Select>
            </div>
            <div>
              <Label>Pickup expires</Label>
              <Input
                className="mt-1.5"
                min={new Date().toISOString().slice(0, 16)}
                onChange={(event) => setExpiresAt(event.target.value)}
                required
                type="datetime-local"
                value={expiresAt}
              />
            </div>
          </div>
        ) : action === "cancel" || action === "expire" ? (
          <div>
            <Label>Reason</Label>
            <Textarea
              className="mt-1.5"
              data-autofocus="true"
              onChange={(event) => setReason(event.target.value)}
              required
              value={reason}
            />
          </div>
        ) : action === "checkout" ? (
          <p className="text-sm leading-6 text-[var(--text-body)]">
            Check out copy {hold.accession_number} to {hold.borrower_name}{" "}
            today?
          </p>
        ) : null}
      </DialogBody>
      <DialogFooter>
        {action ? (
          <>
            <Button
              onClick={() => setAction(null)}
              type="button"
              variant="secondary"
            >
              Back
            </Button>
            <Button
              disabled={
                saving ||
                (action === "ready" && (!copyId || !expiresAt)) ||
                (["cancel", "expire"].includes(action) && !reason.trim())
              }
              onClick={() => void perform()}
              type="button"
              variant={action === "expire" ? "destructive" : "default"}
            >
              {saving ? (
                <>
                  <Loader2 className="size-4 animate-spin" />
                  Saving…
                </>
              ) : action === "ready" ? (
                "Mark ready"
              ) : (
                displayValue(action)
              )}
            </Button>
          </>
        ) : (
          <>
            <Button onClick={onClose} type="button" variant="secondary">
              Close
            </Button>
            {hold.status === "waiting" && canBorrow ? (
              <Button
                onClick={() => setAction("cancel")}
                type="button"
                variant="outline"
              >
                Cancel hold
              </Button>
            ) : null}
            {hold.status === "waiting" && canCirculate ? (
              <Button onClick={() => setAction("ready")} type="button">
                Mark ready
              </Button>
            ) : null}
            {hold.status === "ready" && canCirculate ? (
              <>
                <Button
                  onClick={() => setAction("expire")}
                  type="button"
                  variant="destructive"
                >
                  Expire
                </Button>
                <Button onClick={() => setAction("checkout")} type="button">
                  Check out
                </Button>
              </>
            ) : null}
          </>
        )}
      </DialogFooter>
    </DialogShell>
  );
}

function today() {
  return new Date().toISOString().slice(0, 10);
}
function defaultExpiry() {
  const value = new Date(Date.now() + 48 * 60 * 60 * 1000);
  value.setSeconds(0, 0);
  return value.toISOString().slice(0, 16);
}

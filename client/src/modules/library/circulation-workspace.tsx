import { useCallback, useEffect, useMemo, useState } from "react";
import { BookOpenCheck, Loader2, Plus, Search } from "lucide-react";
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
import type {
  CopyCondition,
  CopyRecord,
  Loan,
  Membership,
  TitleSummary,
} from "./types";
import { displayValue, formatDate, optional, statusTone } from "./ui";

export function LibraryCirculationWorkspace() {
  const permissions = useAuthStore((state) => state.user?.permissions ?? []);
  const canBorrow = allowed(permissions, "library:borrow");
  const canCirculate = allowed(permissions, "library:circulate");
  const canManage = allowed(permissions, "library:manage");
  const [loans, setLoans] = useState<Loan[]>([]);
  const [members, setMembers] = useState<Membership[]>([]);
  const [titles, setTitles] = useState<TitleSummary[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [search, setSearch] = useState("");
  const [status, setStatus] = useState("active");
  const [overdue, setOverdue] = useState(false);
  const [page, setPage] = useState(1);
  const [totalPages, setTotalPages] = useState(1);
  const [checkoutOpen, setCheckoutOpen] = useState(false);
  const [selected, setSelected] = useState<Loan | null>(null);
  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const response = await libraryService.loans({
        page,
        per_page: 25,
        search: search.trim() || undefined,
        status: status === "all" ? undefined : status,
        overdue_only: overdue || undefined,
      });
      if (!response.success || !response.data)
        throw new Error(responseMessage(response, "Loans could not be loaded"));
      setLoans(response.data.loans);
      setTotalPages(response.pagination?.total_pages ?? 1);
    } catch (loadError) {
      setError(
        loadError instanceof Error
          ? loadError.message
          : "Loans could not be loaded",
      );
    } finally {
      setLoading(false);
    }
  }, [overdue, page, search, status]);
  useEffect(() => {
    void load();
  }, [load]);
  useEffect(() => {
    if (!canCirculate) return;
    void Promise.all([
      libraryService.members({ per_page: 100, status: "active" }),
      libraryService.titles({ per_page: 100, status: "active" }),
    ]).then(([memberResponse, titleResponse]) => {
      if (memberResponse.success)
        setMembers(memberResponse.data?.memberships ?? []);
      if (titleResponse.success) setTitles(titleResponse.data?.titles ?? []);
    });
  }, [canCirculate]);
  usePageChrome(
    "Circulation",
    canCirculate ? (
      <Button onClick={() => setCheckoutOpen(true)}>
        <Plus className="size-4" />
        Check out copy
      </Button>
    ) : null,
  );
  const filtered = Boolean(search.trim() || status !== "active" || overdue);
  return (
    <div className="space-y-6">
      <p className="text-sm text-[var(--text-muted)]">
        Check out, renew, return, and record lost copies.
      </p>
      <TableControlsBar>
        <Input
          aria-label="Search loans"
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
          aria-label="Loan status"
          className="sm:w-44"
          onChange={(event) => {
            setPage(1);
            setStatus(event.target.value);
          }}
          value={status}
        >
          <option value="all">All statuses</option>
          <option value="active">Active</option>
          <option value="returned">Returned</option>
          <option value="lost">Lost</option>
        </Select>
        <label className="flex min-h-10 items-center gap-2 text-sm text-[var(--text-body)]">
          <input
            checked={overdue}
            onChange={(event) => {
              setPage(1);
              setOverdue(event.target.checked);
            }}
            type="checkbox"
          />
          Overdue only
        </label>
        {!loading && loans.length > 0 ? (
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
          <TableLoading columns={6} label="Loading loans…" />
        ) : error ? (
          <TableError description={error} onRetry={() => void load()} />
        ) : loans.length === 0 ? (
          <TableEmpty
            description={
              filtered
                ? "Change the current filters."
                : canCirculate
                  ? "Check out an available copy to an active member."
                  : "No loans are available for this account."
            }
            icon={<BookOpenCheck />}
            title={filtered ? "No loans match" : "No loans yet"}
          />
        ) : (
          <TableScroll>
            <Table className="min-w-[840px]">
              <THead>
                <tr>
                  <TH>Title</TH>
                  <TH>Member</TH>
                  <TH>Accession</TH>
                  <TH>Checked out</TH>
                  <TH>Due</TH>
                  <TH>Status</TH>
                </tr>
              </THead>
              <TBody>
                {loans.map((loan) => (
                  <TR
                    className="cursor-pointer"
                    key={loan.id}
                    onClick={() => setSelected(loan)}
                  >
                    <TD>
                      <span className="font-medium text-[var(--text-strong)]">
                        {loan.title}
                      </span>
                    </TD>
                    <TD>
                      <span className="text-[var(--text-strong)]">
                        {loan.borrower_name}
                      </span>
                      <p className="mt-1 text-xs text-[var(--text-muted)]">
                        {loan.borrower_number}
                      </p>
                    </TD>
                    <TD className="font-tabular text-[var(--text-muted)]">
                      {loan.accession_number}
                    </TD>
                    <TD className="whitespace-nowrap text-[var(--text-muted)]">
                      {formatDate(loan.checked_out_on)}
                    </TD>
                    <TD
                      className={
                        loan.overdue
                          ? "whitespace-nowrap font-medium text-[var(--tone-danger)]"
                          : "whitespace-nowrap text-[var(--text-muted)]"
                      }
                    >
                      {formatDate(loan.due_on)}
                      {loan.overdue ? ` · ${loan.overdue_days}d overdue` : ""}
                    </TD>
                    <TD>
                      <Badge tone={statusTone(loan.status)}>
                        {displayValue(loan.status)}
                      </Badge>
                    </TD>
                  </TR>
                ))}
              </TBody>
            </Table>
          </TableScroll>
        )}
      </TableWrap>
      <CheckoutDrawer
        members={members}
        onClose={() => setCheckoutOpen(false)}
        onSaved={() => {
          setCheckoutOpen(false);
          void load();
        }}
        open={checkoutOpen}
        titles={titles}
      />
      <LoanDrawer
        canBorrow={canBorrow}
        canCirculate={canCirculate}
        canManage={canManage}
        loan={selected}
        onClose={() => setSelected(null)}
        onSaved={(value) => {
          setSelected(value);
          setLoans((current) =>
            current.map((item) => (item.id === value.id ? value : item)),
          );
        }}
      />
    </div>
  );
}

function CheckoutDrawer({
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
  const [copies, setCopies] = useState<CopyRecord[]>([]);
  const [copyId, setCopyId] = useState("");
  const [memberId, setMemberId] = useState("");
  const [date, setDate] = useState(today());
  const [notes, setNotes] = useState("");
  const [saving, setSaving] = useState(false);
  const [loadingCopies, setLoadingCopies] = useState(false);
  useEffect(() => {
    if (!open) return;
    setTitleId("");
    setCopies([]);
    setCopyId("");
    setMemberId("");
    setDate(today());
    setNotes("");
  }, [open]);
  useEffect(() => {
    if (!titleId) {
      setCopies([]);
      setCopyId("");
      return;
    }
    setLoadingCopies(true);
    void libraryService
      .copies(titleId, { per_page: 100, status: "available" })
      .then((response) => {
        const values = response.success ? (response.data?.copies ?? []) : [];
        setCopies(values);
        setCopyId(values[0]?.id ?? "");
      })
      .finally(() => setLoadingCopies(false));
  }, [titleId]);
  const submit = async (event: React.FormEvent) => {
    event.preventDefault();
    if (!copyId || !memberId || saving) return;
    setSaving(true);
    try {
      const response = await libraryService.checkout({
        copy_id: copyId,
        membership_id: memberId,
        fulfilled_hold_id: null,
        checked_out_on: date,
        notes: optional(notes),
      });
      if (!response.success || !response.data)
        throw new Error(
          responseMessage(response, "Copy could not be checked out"),
        );
      toast.success("Copy checked out");
      onSaved();
    } catch (error) {
      toast.error(
        error instanceof Error
          ? error.message
          : "Copy could not be checked out",
      );
    } finally {
      setSaving(false);
    }
  };
  return (
    <DialogShell onClose={onClose} open={open}>
      <DialogHeader
        onClose={saving ? undefined : onClose}
        title="Check out copy"
      />
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
              {titles
                .filter((title) => title.available_copy_count > 0)
                .map((title) => (
                  <option key={title.id} value={title.id}>
                    {title.title} · {title.available_copy_count} available
                  </option>
                ))}
            </Select>
          </div>
          <div>
            <Label>Copy</Label>
            <Select
              className="mt-1.5"
              disabled={!titleId || loadingCopies}
              onChange={(event) => setCopyId(event.target.value)}
              required
              value={copyId}
            >
              <option value="">
                {loadingCopies ? "Loading copies…" : "Choose a copy"}
              </option>
              {copies.map((copy) => (
                <option key={copy.id} value={copy.id}>
                  {copy.accession_number}
                  {copy.location ? ` · ${copy.location}` : ""}
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
                  {member.borrower_name} · {member.borrower_number} (
                  {member.active_loan_count}/{member.loan_limit})
                </option>
              ))}
            </Select>
          </div>
          <div>
            <Label>Checkout date</Label>
            <Input
              className="mt-1.5"
              max={today()}
              onChange={(event) => setDate(event.target.value)}
              required
              type="date"
              value={date}
            />
          </div>
          <div>
            <Label>Notes</Label>
            <Textarea
              className="mt-1.5"
              onChange={(event) => setNotes(event.target.value)}
              value={notes}
            />
          </div>
        </DialogBody>
        <DialogFooter>
          <Button
            disabled={saving}
            onClick={onClose}
            type="button"
            variant="secondary"
          >
            Cancel
          </Button>
          <Button disabled={saving || !copyId || !memberId} type="submit">
            {saving ? (
              <>
                <Loader2 className="size-4 animate-spin" />
                Checking out…
              </>
            ) : (
              "Check out"
            )}
          </Button>
        </DialogFooter>
      </form>
    </DialogShell>
  );
}

function LoanDrawer({
  canBorrow,
  canCirculate,
  canManage,
  loan,
  onClose,
  onSaved,
}: {
  canBorrow: boolean;
  canCirculate: boolean;
  canManage: boolean;
  loan: Loan | null;
  onClose: () => void;
  onSaved: (loan: Loan) => void;
}) {
  const [action, setAction] = useState<
    "renew" | "return" | "lost" | "fine" | null
  >(null);
  const [dueOn, setDueOn] = useState("");
  const [returnDate, setReturnDate] = useState(today());
  const [condition, setCondition] = useState<CopyCondition>("good");
  const [notes, setNotes] = useState("");
  const [reason, setReason] = useState("");
  const [fineKind, setFineKind] = useState<"overdue" | "replacement">(
    "overdue",
  );
  const [saving, setSaving] = useState(false);
  useEffect(() => {
    setAction(null);
    setDueOn("");
    setReturnDate(today());
    setCondition("good");
    setNotes("");
    setReason("");
    setFineKind(loan?.status === "lost" ? "replacement" : "overdue");
  }, [loan]);
  if (!loan) return null;
  const perform = async () => {
    if (!action || saving) return;
    setSaving(true);
    try {
      if (action === "fine") {
        const response = await libraryService.assessFine(loan.id, fineKind);
        if (!response.success)
          throw new Error(
            responseMessage(response, "Fine could not be assessed"),
          );
        toast.success("Fine assessed");
        onClose();
        return;
      }
      const response =
        action === "renew"
          ? await libraryService.renewLoan(loan, dueOn || null)
          : action === "return"
            ? await libraryService.returnLoan(
                loan,
                returnDate,
                condition,
                optional(notes),
              )
            : await libraryService.markLost(loan, reason);
      if (!response.success || !response.data)
        throw new Error(
          responseMessage(
            response,
            `Loan could not be ${action === "renew" ? "renewed" : action === "return" ? "returned" : "marked lost"}`,
          ),
        );
      toast.success(
        action === "renew"
          ? "Loan renewed"
          : action === "return"
            ? "Copy returned"
            : "Copy marked lost",
      );
      onSaved(response.data);
      setAction(null);
    } catch (error) {
      toast.error(
        error instanceof Error ? error.message : "Loan could not be updated",
      );
    } finally {
      setSaving(false);
    }
  };
  return (
    <DialogShell onClose={onClose} open>
      <DialogHeader
        onClose={saving ? undefined : onClose}
        title="Library loan"
      />
      <DialogBody className="space-y-6">
        <div className="border border-[var(--border)] bg-[var(--surface-muted)] p-4">
          <div className="flex items-start justify-between gap-4">
            <div>
              <p className="font-semibold text-[var(--text-strong)]">
                {loan.title}
              </p>
              <p className="mt-1 text-sm text-[var(--text-muted)]">
                {loan.accession_number}
              </p>
            </div>
            <Badge tone={statusTone(loan.status)}>
              {displayValue(loan.status)}
            </Badge>
          </div>
          <dl className="mt-5 grid grid-cols-2 gap-4 text-sm">
            <div>
              <dt className="text-[var(--text-muted)]">Member</dt>
              <dd className="mt-1 font-medium text-[var(--text-strong)]">
                {loan.borrower_name}
              </dd>
            </div>
            <div>
              <dt className="text-[var(--text-muted)]">Due</dt>
              <dd
                className={
                  loan.overdue
                    ? "mt-1 font-medium text-[var(--tone-danger)]"
                    : "mt-1 font-medium text-[var(--text-strong)]"
                }
              >
                {formatDate(loan.due_on)}
              </dd>
            </div>
          </dl>
        </div>
        {action === "renew" ? (
          <div>
            <Label>New due date (optional)</Label>
            <Input
              className="mt-1.5"
              min={loan.due_on}
              onChange={(event) => setDueOn(event.target.value)}
              type="date"
              value={dueOn}
            />
            <p className="mt-2 text-xs text-[var(--text-muted)]">
              Leave blank to apply the configured borrower loan period.
            </p>
          </div>
        ) : action === "return" ? (
          <div className="space-y-4">
            <div>
              <Label>Return date</Label>
              <Input
                className="mt-1.5"
                max={today()}
                min={loan.checked_out_on}
                onChange={(event) => setReturnDate(event.target.value)}
                type="date"
                value={returnDate}
              />
            </div>
            <div>
              <Label>Copy condition</Label>
              <Select
                className="mt-1.5"
                onChange={(event) =>
                  setCondition(event.target.value as CopyCondition)
                }
                value={condition}
              >
                <option value="new">New</option>
                <option value="good">Good</option>
                <option value="worn">Worn</option>
                <option value="damaged">Damaged</option>
              </Select>
            </div>
            <div>
              <Label>Notes</Label>
              <Textarea
                className="mt-1.5"
                onChange={(event) => setNotes(event.target.value)}
                value={notes}
              />
            </div>
          </div>
        ) : action === "lost" ? (
          <div>
            <Label>Reason</Label>
            <Textarea
              className="mt-1.5"
              onChange={(event) => setReason(event.target.value)}
              required
              value={reason}
            />
          </div>
        ) : action === "fine" ? (
          <div>
            <Label>Fine type</Label>
            <Select
              className="mt-1.5"
              onChange={(event) =>
                setFineKind(event.target.value as "overdue" | "replacement")
              }
              value={fineKind}
            >
              <option value="overdue">Overdue</option>
              <option value="replacement">Replacement</option>
            </Select>
          </div>
        ) : null}
      </DialogBody>
      <DialogFooter>
        {action ? (
          <>
            <Button
              disabled={saving}
              onClick={() => setAction(null)}
              type="button"
              variant="secondary"
            >
              Back
            </Button>
            <Button
              disabled={saving || (action === "lost" && !reason.trim())}
              onClick={() => void perform()}
              type="button"
              variant={action === "lost" ? "destructive" : "default"}
            >
              {saving ? (
                <>
                  <Loader2 className="size-4 animate-spin" />
                  Saving…
                </>
              ) : action === "fine" ? (
                "Assess fine"
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
            {loan.status === "active" ? (
              <>
                {canManage && loan.overdue ? (
                  <Button
                    onClick={() => setAction("fine")}
                    type="button"
                    variant="outline"
                  >
                    Assess overdue fine
                  </Button>
                ) : null}
                {canBorrow ? (
                  <Button
                    onClick={() => setAction("renew")}
                    type="button"
                    variant="outline"
                  >
                    Renew
                  </Button>
                ) : null}
                {canCirculate ? (
                  <>
                    <Button onClick={() => setAction("return")} type="button">
                      Return copy
                    </Button>
                    <Button
                      onClick={() => setAction("lost")}
                      type="button"
                      variant="destructive"
                    >
                      Mark lost
                    </Button>
                  </>
                ) : null}
              </>
            ) : canManage && loan.status === "lost" ? (
              <Button onClick={() => setAction("fine")} type="button">
                Assess replacement fine
              </Button>
            ) : null}
          </>
        )}
      </DialogFooter>
    </DialogShell>
  );
}

function allowed(permissions: string[], permission: string) {
  return permissions.includes("*") || permissions.includes(permission);
}
function today() {
  return new Date().toISOString().slice(0, 10);
}

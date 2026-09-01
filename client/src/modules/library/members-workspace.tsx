import { useCallback, useEffect, useMemo, useState } from "react";
import { Loader2, Plus, Search, UsersRound } from "lucide-react";
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
import { Input, Label, Select } from "@/components/ui/input";
import { usePageChrome } from "@/modules/admin/layouts/page-chrome";
import { useAuthStore } from "@/stores/auth-store";

import { libraryService, responseMessage } from "./service";
import type {
  BorrowerKind,
  LibraryReferenceData,
  Membership,
  MembershipStatus,
} from "./types";
import { displayValue, statusTone } from "./ui";

export function LibraryMembersWorkspace() {
  const permissions = useAuthStore((state) => state.user?.permissions ?? []);
  const canManage =
    permissions.includes("*") || permissions.includes("library:manage");
  const [members, setMembers] = useState<Membership[]>([]);
  const [references, setReferences] = useState<LibraryReferenceData | null>(
    null,
  );
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [search, setSearch] = useState("");
  const [status, setStatus] = useState("all");
  const [page, setPage] = useState(1);
  const [totalPages, setTotalPages] = useState(1);
  const [drawerOpen, setDrawerOpen] = useState(false);
  const [selected, setSelected] = useState<Membership | null>(null);
  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const response = await libraryService.members({
        page,
        per_page: 25,
        search: search.trim() || undefined,
        status: status === "all" ? undefined : status,
      });
      if (!response.success || !response.data)
        throw new Error(
          responseMessage(response, "Library members could not be loaded"),
        );
      setMembers(response.data.memberships);
      setTotalPages(response.pagination?.total_pages ?? 1);
    } catch (loadError) {
      setError(
        loadError instanceof Error
          ? loadError.message
          : "Library members could not be loaded",
      );
    } finally {
      setLoading(false);
    }
  }, [page, search, status]);
  useEffect(() => {
    void load();
  }, [load]);
  useEffect(() => {
    if (!canManage) return;
    void libraryService.references().then((response) => {
      if (response.success) setReferences(response.data ?? null);
    });
  }, [canManage]);
  usePageChrome(
    "Members",
    canManage ? (
      <Button
        onClick={() => {
          setSelected(null);
          setDrawerOpen(true);
        }}
      >
        <Plus className="size-4" />
        Add member
      </Button>
    ) : null,
  );
  const filtered = Boolean(search.trim() || status !== "all");
  return (
    <div className="space-y-6">
      <p className="text-sm text-[var(--text-muted)]">
        Library memberships reference existing learner and employee records.
      </p>
      <TableControlsBar>
        <Input
          aria-label="Search members"
          className="sm:w-72"
          leadingIcon={<Search />}
          onChange={(event) => {
            setPage(1);
            setSearch(event.target.value);
          }}
          placeholder="Search name, number, or card"
          value={search}
        />
        <Select
          aria-label="Membership status"
          className="sm:w-44"
          onChange={(event) => {
            setPage(1);
            setStatus(event.target.value);
          }}
          value={status}
        >
          <option value="all">All statuses</option>
          <option value="active">Active</option>
          <option value="suspended">Suspended</option>
          <option value="closed">Closed</option>
        </Select>
        {!loading && members.length > 0 ? (
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
          <TableLoading columns={6} label="Loading members…" />
        ) : error ? (
          <TableError description={error} onRetry={() => void load()} />
        ) : members.length === 0 ? (
          <TableEmpty
            description={
              filtered
                ? "Change the current filters."
                : canManage
                  ? "Add a learner or employee from the campus directory."
                  : "No Library membership is available for this account."
            }
            icon={<UsersRound />}
            title={filtered ? "No members match" : "No Library members yet"}
          />
        ) : (
          <TableScroll>
            <Table className="min-w-[780px]">
              <THead>
                <tr>
                  <TH>Member</TH>
                  <TH>Card</TH>
                  <TH>Type</TH>
                  <TH>Status</TH>
                  <TH>Loans</TH>
                  <TH>Holds</TH>
                </tr>
              </THead>
              <TBody>
                {members.map((member) => (
                  <TR
                    className={canManage ? "cursor-pointer" : undefined}
                    key={member.id}
                    onClick={() => {
                      if (canManage) {
                        setSelected(member);
                        setDrawerOpen(true);
                      }
                    }}
                  >
                    <TD>
                      <span className="font-medium text-[var(--text-strong)]">
                        {member.borrower_name}
                      </span>
                      <p className="mt-1 text-xs text-[var(--text-muted)]">
                        {member.borrower_number}
                      </p>
                    </TD>
                    <TD className="font-tabular text-[var(--text-muted)]">
                      {member.card_number}
                    </TD>
                    <TD className="text-[var(--text-muted)]">
                      {displayValue(member.borrower_kind)}
                    </TD>
                    <TD>
                      <Badge tone={statusTone(member.status)}>
                        {displayValue(member.status)}
                      </Badge>
                    </TD>
                    <TD className="font-tabular text-[var(--text-muted)]">
                      {member.active_loan_count} / {member.loan_limit}
                    </TD>
                    <TD className="font-tabular text-[var(--text-muted)]">
                      {member.active_hold_count}
                    </TD>
                  </TR>
                ))}
              </TBody>
            </Table>
          </TableScroll>
        )}
      </TableWrap>
      <MemberDrawer
        member={selected}
        onClose={() => setDrawerOpen(false)}
        onSaved={() => {
          setDrawerOpen(false);
          void load();
          void libraryService.references().then((response) => {
            if (response.success) setReferences(response.data ?? null);
          });
        }}
        open={drawerOpen}
        references={references}
      />
    </div>
  );
}

function MemberDrawer({
  member,
  onClose,
  onSaved,
  open,
  references,
}: {
  member: Membership | null;
  onClose: () => void;
  onSaved: () => void;
  open: boolean;
  references: LibraryReferenceData | null;
}) {
  const [kind, setKind] = useState<BorrowerKind>("learner");
  const [borrowerId, setBorrowerId] = useState("");
  const [status, setStatus] = useState<MembershipStatus>("active");
  const [loanLimit, setLoanLimit] = useState("5");
  const [saving, setSaving] = useState(false);
  useEffect(() => {
    if (!open) return;
    setKind(member?.borrower_kind ?? "learner");
    setBorrowerId(member?.borrower_id ?? "");
    setStatus(member?.status ?? "active");
    setLoanLimit(String(member?.loan_limit ?? 5));
  }, [member, open]);
  const candidates = useMemo(
    () =>
      (kind === "learner"
        ? references?.learners
        : references?.employees
      )?.filter(
        (candidate) => !candidate.already_member || candidate.id === borrowerId,
      ) ?? [],
    [borrowerId, kind, references],
  );
  const submit = async (event: React.FormEvent) => {
    event.preventDefault();
    if (saving || (!member && !borrowerId)) return;
    setSaving(true);
    try {
      const response = member
        ? await libraryService.updateMember(
            member.id,
            member.version,
            status,
            Number(loanLimit),
          )
        : await libraryService.createMember({
            borrower_kind: kind,
            borrower_id: borrowerId,
            loan_limit: loanLimit ? Number(loanLimit) : null,
          });
      if (!response.success || !response.data)
        throw new Error(
          responseMessage(response, "Library membership could not be saved"),
        );
      toast.success(member ? "Membership updated" : "Member added");
      onSaved();
    } catch (error) {
      toast.error(
        error instanceof Error
          ? error.message
          : "Library membership could not be saved",
      );
    } finally {
      setSaving(false);
    }
  };
  return (
    <DialogShell onClose={onClose} open={open}>
      <DialogHeader
        onClose={saving ? undefined : onClose}
        title={member ? "Library member" : "Add Library member"}
      />
      <form onSubmit={submit}>
        <DialogBody className="space-y-5">
          {member ? (
            <div className="border border-[var(--border)] bg-[var(--surface-muted)] p-4">
              <p className="font-medium text-[var(--text-strong)]">
                {member.borrower_name}
              </p>
              <p className="mt-1 text-sm text-[var(--text-muted)]">
                {member.borrower_number} · {member.card_number}
              </p>
            </div>
          ) : (
            <>
              <div>
                <Label>Directory</Label>
                <Select
                  className="mt-1.5"
                  data-autofocus="true"
                  onChange={(event) => {
                    setKind(event.target.value as BorrowerKind);
                    setBorrowerId("");
                  }}
                  value={kind}
                >
                  <option value="learner">Learners</option>
                  <option value="employee">Employees</option>
                </Select>
              </div>
              <div>
                <Label>Person</Label>
                <Select
                  className="mt-1.5"
                  onChange={(event) => setBorrowerId(event.target.value)}
                  required
                  value={borrowerId}
                >
                  <option value="">Choose a person</option>
                  {candidates.map((candidate) => (
                    <option key={candidate.id} value={candidate.id}>
                      {candidate.display_name} · {candidate.number}
                    </option>
                  ))}
                </Select>
                {candidates.length === 0 ? (
                  <p className="mt-2 text-xs text-[var(--text-muted)]">
                    Everyone in this directory already has a membership.
                  </p>
                ) : null}
              </div>
            </>
          )}
          <div>
            <Label>Loan limit</Label>
            <Input
              className="mt-1.5"
              max="100"
              min="1"
              onChange={(event) => setLoanLimit(event.target.value)}
              required
              type="number"
              value={loanLimit}
            />
          </div>
          {member ? (
            <div>
              <Label>Status</Label>
              <Select
                className="mt-1.5"
                onChange={(event) =>
                  setStatus(event.target.value as MembershipStatus)
                }
                value={status}
              >
                <option value="active">Active</option>
                <option value="suspended">Suspended</option>
                <option value="closed">Closed</option>
              </Select>
            </div>
          ) : null}
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
          <Button disabled={saving || (!member && !borrowerId)} type="submit">
            {saving ? (
              <>
                <Loader2 className="size-4 animate-spin" />
                Saving…
              </>
            ) : member ? (
              "Save membership"
            ) : (
              "Add member"
            )}
          </Button>
        </DialogFooter>
      </form>
    </DialogShell>
  );
}

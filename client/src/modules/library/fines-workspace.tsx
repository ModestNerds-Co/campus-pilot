import { useCallback, useEffect, useMemo, useState } from "react";
import { CircleDollarSign, Loader2, Search } from "lucide-react";
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
import type { Fine, LibraryReferenceData } from "./types";
import { displayValue, formatDateTime, formatMinor, statusTone } from "./ui";

export function LibraryFinesWorkspace() {
  const permissions = useAuthStore((state) => state.user?.permissions ?? []);
  const canManage =
    permissions.includes("*") || permissions.includes("library:manage");
  const [fines, setFines] = useState<Fine[]>([]);
  const [references, setReferences] = useState<LibraryReferenceData | null>(
    null,
  );
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [search, setSearch] = useState("");
  const [status, setStatus] = useState("all");
  const [page, setPage] = useState(1);
  const [totalPages, setTotalPages] = useState(1);
  const [selected, setSelected] = useState<Fine | null>(null);
  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const response = await libraryService.fines({
        page,
        per_page: 25,
        search: search.trim() || undefined,
        status: status === "all" ? undefined : status,
      });
      if (!response.success || !response.data)
        throw new Error(responseMessage(response, "Fines could not be loaded"));
      setFines(response.data.fines);
      setTotalPages(response.pagination?.total_pages ?? 1);
    } catch (loadError) {
      setError(
        loadError instanceof Error
          ? loadError.message
          : "Fines could not be loaded",
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
  usePageChrome("Fines");
  const filtered = Boolean(search.trim() || status !== "all");
  return (
    <div className="space-y-6">
      <p className="text-sm text-[var(--text-muted)]">
        Review assessed fines and submit learner charges to Fees.
      </p>
      <TableControlsBar>
        <Input
          aria-label="Search fines"
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
          aria-label="Fine status"
          className="sm:w-48"
          onChange={(event) => {
            setPage(1);
            setStatus(event.target.value);
          }}
          value={status}
        >
          <option value="all">All statuses</option>
          <option value="assessed">Assessed</option>
          <option value="submitted_to_fees">Submitted to Fees</option>
          <option value="waived">Waived</option>
        </Select>
        {!loading && fines.length > 0 ? (
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
          <TableLoading columns={6} label="Loading fines…" />
        ) : error ? (
          <TableError description={error} onRetry={() => void load()} />
        ) : fines.length === 0 ? (
          <TableEmpty
            description={
              filtered
                ? "Change the current filters."
                : "No fines are in this scope."
            }
            icon={<CircleDollarSign />}
            title={filtered ? "No fines match" : "No fines yet"}
          />
        ) : (
          <TableScroll>
            <Table className="min-w-[820px]">
              <THead>
                <tr>
                  <TH>Member</TH>
                  <TH>Title</TH>
                  <TH>Type</TH>
                  <TH>Amount</TH>
                  <TH>Status</TH>
                  <TH>Assessed</TH>
                </tr>
              </THead>
              <TBody>
                {fines.map((fine) => (
                  <TR
                    className="cursor-pointer"
                    key={fine.id}
                    onClick={() => setSelected(fine)}
                  >
                    <TD>
                      <span className="font-medium text-[var(--text-strong)]">
                        {fine.borrower_name}
                      </span>
                      <p className="mt-1 text-xs text-[var(--text-muted)]">
                        {fine.borrower_number}
                      </p>
                    </TD>
                    <TD className="text-[var(--text-body)]">{fine.title}</TD>
                    <TD className="text-[var(--text-muted)]">
                      {displayValue(fine.kind)}
                    </TD>
                    <TD className="font-tabular font-medium text-[var(--text-strong)]">
                      {formatMinor(
                        fine.amount_minor,
                        fine.currency_code,
                        fine.currency_minor_units,
                      )}
                    </TD>
                    <TD>
                      <Badge tone={statusTone(fine.status)}>
                        {displayValue(fine.status)}
                      </Badge>
                    </TD>
                    <TD className="whitespace-nowrap text-[var(--text-muted)]">
                      {formatDateTime(fine.created_at)}
                    </TD>
                  </TR>
                ))}
              </TBody>
            </Table>
          </TableScroll>
        )}
      </TableWrap>
      <FineDrawer
        canManage={canManage}
        fine={selected}
        onClose={() => setSelected(null)}
        onSaved={(value) => {
          setSelected(value);
          setFines((current) =>
            current.map((item) => (item.id === value.id ? value : item)),
          );
        }}
        references={references}
      />
    </div>
  );
}

function FineDrawer({
  canManage,
  fine,
  onClose,
  onSaved,
  references,
}: {
  canManage: boolean;
  fine: Fine | null;
  onClose: () => void;
  onSaved: (fine: Fine) => void;
  references: LibraryReferenceData | null;
}) {
  const [action, setAction] = useState<"submit" | "waive" | null>(null);
  const [billingAccountId, setBillingAccountId] = useState("");
  const [reason, setReason] = useState("");
  const [saving, setSaving] = useState(false);
  const accounts = useMemo(
    () =>
      references?.billing_accounts.filter(
        (account) => account.learner_number === fine?.borrower_number,
      ) ?? [],
    [fine, references],
  );
  useEffect(() => {
    setAction(null);
    setBillingAccountId(accounts[0]?.id ?? "");
    setReason("");
  }, [accounts, fine]);
  if (!fine) return null;
  const perform = async () => {
    if (!action || saving) return;
    setSaving(true);
    try {
      const response =
        action === "submit"
          ? await libraryService.submitFine(fine, billingAccountId)
          : await libraryService.waiveFine(fine, reason);
      if (!response.success || !response.data)
        throw new Error(responseMessage(response, "Fine could not be updated"));
      toast.success(
        action === "submit" ? "Fine submitted to Fees" : "Fine waived",
      );
      onSaved(response.data);
      setAction(null);
    } catch (error) {
      toast.error(
        error instanceof Error ? error.message : "Fine could not be updated",
      );
    } finally {
      setSaving(false);
    }
  };
  return (
    <DialogShell onClose={onClose} open>
      <DialogHeader
        onClose={saving ? undefined : onClose}
        title="Library fine"
      />
      <DialogBody className="space-y-6">
        <div className="border border-[var(--border)] bg-[var(--surface-muted)] p-4">
          <div className="flex items-start justify-between gap-4">
            <div>
              <p className="font-semibold text-[var(--text-strong)]">
                {formatMinor(
                  fine.amount_minor,
                  fine.currency_code,
                  fine.currency_minor_units,
                )}
              </p>
              <p className="mt-1 text-sm text-[var(--text-muted)]">
                {displayValue(fine.kind)} · {fine.title}
              </p>
            </div>
            <Badge tone={statusTone(fine.status)}>
              {displayValue(fine.status)}
            </Badge>
          </div>
          <p className="mt-4 text-sm text-[var(--text-body)]">
            {fine.borrower_name} · {fine.borrower_number}
          </p>
          {fine.assessed_days ? (
            <p className="mt-1 text-xs text-[var(--text-muted)]">
              {fine.assessed_days} overdue days assessed
            </p>
          ) : null}
          {fine.fees_charge_status ? (
            <p className="mt-3 text-xs text-[var(--text-muted)]">
              Fees request: {displayValue(fine.fees_charge_status)}
            </p>
          ) : null}
          {fine.waiver_reason ? (
            <p className="mt-3 text-sm text-[var(--text-body)]">
              {fine.waiver_reason}
            </p>
          ) : null}
        </div>
        {action === "submit" ? (
          <div>
            <Label>Active learner billing account</Label>
            <Select
              className="mt-1.5"
              data-autofocus="true"
              onChange={(event) => setBillingAccountId(event.target.value)}
              required
              value={billingAccountId}
            >
              <option value="">Choose an account</option>
              {accounts.map((account) => (
                <option key={account.id} value={account.id}>
                  {account.account_number} · {account.learner_name}
                </option>
              ))}
            </Select>
            {accounts.length === 0 ? (
              <p className="mt-2 text-xs text-[var(--tone-danger)]">
                This learner does not have an active Fees billing account.
              </p>
            ) : null}
          </div>
        ) : action === "waive" ? (
          <div>
            <Label>Waiver reason</Label>
            <Textarea
              className="mt-1.5"
              data-autofocus="true"
              onChange={(event) => setReason(event.target.value)}
              required
              value={reason}
            />
          </div>
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
                (action === "submit" ? !billingAccountId : !reason.trim())
              }
              onClick={() => void perform()}
              type="button"
            >
              {saving ? (
                <>
                  <Loader2 className="size-4 animate-spin" />
                  Saving…
                </>
              ) : action === "submit" ? (
                "Submit to Fees"
              ) : (
                "Waive fine"
              )}
            </Button>
          </>
        ) : (
          <>
            <Button onClick={onClose} type="button" variant="secondary">
              Close
            </Button>
            {canManage && fine.status === "assessed" ? (
              <>
                <Button
                  onClick={() => setAction("waive")}
                  type="button"
                  variant="outline"
                >
                  Waive
                </Button>
                {fine.borrower_kind === "learner" ? (
                  <Button
                    disabled={accounts.length === 0}
                    onClick={() => setAction("submit")}
                    type="button"
                  >
                    Submit to Fees
                  </Button>
                ) : null}
              </>
            ) : null}
          </>
        )}
      </DialogFooter>
    </DialogShell>
  );
}

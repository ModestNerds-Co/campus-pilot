import { useCallback, useEffect, useState } from "react";
import {
  CalendarClock,
  Edit,
  Loader2,
  MoreVertical,
  Plus,
  Search,
  Trash2,
} from "lucide-react";
import toast from "react-hot-toast";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { ConfirmDrawer } from "@/components/ui/confirm-drawer";
import {
  Table,
  TableControlsBar,
  TableControlsPagination,
  TableControlsSearch,
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

import { hrPayrollService, hrResponseMessage } from "./service";
import type {
  AvailabilityKind,
  AvailabilityStatus,
  Employee,
  EmployeeAvailability,
  EmployeeAvailabilityInput,
} from "./types";

export function EmployeeAvailabilityList() {
  const [records, setRecords] = useState<EmployeeAvailability[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [search, setSearch] = useState("");
  const [submittedSearch, setSubmittedSearch] = useState("");
  const [status, setStatus] = useState<"all" | AvailabilityStatus>("all");
  const [kind, setKind] = useState<"all" | AvailabilityKind>("all");
  const [page, setPage] = useState(1);
  const [totalPages, setTotalPages] = useState(1);
  const [drawerRecord, setDrawerRecord] = useState<
    EmployeeAvailability | null | undefined
  >(undefined);
  const [deleteRecord, setDeleteRecord] = useState<EmployeeAvailability | null>(
    null,
  );
  const [menuId, setMenuId] = useState<string | null>(null);
  const [deleting, setDeleting] = useState(false);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const response = await hrPayrollService.listEmployeeAvailability({
        page,
        per_page: 20,
        search: submittedSearch || undefined,
        status: status === "all" ? undefined : status,
        kind: kind === "all" ? undefined : kind,
      });
      if (!response.success || !response.data)
        throw new Error(
          hrResponseMessage(
            response,
            "Employee availability could not be loaded",
          ),
        );
      setRecords(response.data.availability_periods);
      setTotalPages(response.pagination?.total_pages ?? 1);
    } catch (loadError) {
      setError(
        loadError instanceof Error
          ? loadError.message
          : "Employee availability could not be loaded",
      );
    } finally {
      setLoading(false);
    }
  }, [kind, page, status, submittedSearch]);

  useEffect(() => {
    void load();
  }, [load]);
  const remove = async () => {
    if (!deleteRecord || deleting) return;
    setDeleting(true);
    const response = await hrPayrollService.deleteEmployeeAvailability(
      deleteRecord.id,
    );
    setDeleting(false);
    if (!response.success)
      return toast.error(
        hrResponseMessage(response, "Availability period could not be removed"),
      );
    toast.success("Availability period removed");
    setDeleteRecord(null);
    void load();
  };

  usePageChrome(
    "Availability",
    <Button onClick={() => setDrawerRecord(null)}>
      <Plus className="size-4" />
      Add period
    </Button>,
  );
  const filtered = submittedSearch || status !== "all" || kind !== "all";

  return (
    <div className="space-y-6">
      <p className="text-sm text-[var(--text-muted)]">
        Approved periods are the workforce scheduling constraint for
        Timetabling, Fleet, and other operational modules.
      </p>
      <TableControlsBar>
        <TableControlsSearch
          onSubmit={(event) => {
            event.preventDefault();
            setPage(1);
            setSubmittedSearch(search.trim());
          }}
        >
          <Input
            aria-label="Search employee availability"
            leadingIcon={<Search />}
            onChange={(event) => setSearch(event.target.value)}
            placeholder="Search employee…"
            value={search}
          />
          <Button type="submit" variant="secondary">
            Search
          </Button>
        </TableControlsSearch>
        <Select
          aria-label="Availability type"
          className="sm:w-40"
          onChange={(event) => {
            setPage(1);
            setKind(event.target.value as typeof kind);
          }}
          value={kind}
        >
          <option value="all">All types</option>
          {availabilityKinds.map((item) => (
            <option key={item} value={item}>
              {displayStatus(item)}
            </option>
          ))}
        </Select>
        <Select
          aria-label="Availability status"
          className="sm:w-44"
          onChange={(event) => {
            setPage(1);
            setStatus(event.target.value as typeof status);
          }}
          value={status}
        >
          <option value="all">All statuses</option>
          {availabilityStatuses.map((item) => (
            <option key={item} value={item}>
              {displayStatus(item)}
            </option>
          ))}
        </Select>
        {!loading && records.length > 0 ? (
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
          <TableLoading columns={6} label="Loading availability…" />
        ) : error ? (
          <TableError description={error} onRetry={() => void load()} />
        ) : records.length === 0 ? (
          <TableEmpty
            description={
              filtered
                ? "Change the current filters."
                : "Add the first availability period."
            }
            icon={<CalendarClock />}
            title={
              filtered
                ? "No periods match these filters"
                : "No availability periods yet"
            }
          />
        ) : (
          <TableScroll>
            <Table>
              <THead>
                <tr>
                  <TH>Employee</TH>
                  <TH>Type</TH>
                  <TH>Starts</TH>
                  <TH>Ends</TH>
                  <TH>Status</TH>
                  <TH className="text-right">Actions</TH>
                </tr>
              </THead>
              <TBody>
                {records.map((record) => (
                  <TR key={record.id}>
                    <TD>
                      <div className="font-medium text-[var(--text-strong)]">
                        {record.employee_name}
                      </div>
                      <div className="font-tabular text-xs text-[var(--text-muted)]">
                        {record.employee_number}
                      </div>
                    </TD>
                    <TD className="text-[var(--text-muted)]">
                      {displayStatus(record.kind)}
                    </TD>
                    <TD className="font-tabular text-sm">
                      {formatDateTime(record.starts_at)}
                    </TD>
                    <TD className="font-tabular text-sm">
                      {formatDateTime(record.ends_at)}
                    </TD>
                    <TD>
                      <Badge tone={availabilityTone(record.status)}>
                        {displayStatus(record.status)}
                      </Badge>
                      {record.decided_by_name ? (
                        <div className="mt-1 text-xs text-[var(--text-muted)]">
                          by {record.decided_by_name}
                        </div>
                      ) : null}
                    </TD>
                    <TD className="text-right">
                      {record.status === "rejected" ||
                      record.status === "cancelled" ? (
                        <span className="text-[var(--text-subtle)]">—</span>
                      ) : (
                        <div className="relative inline-flex">
                          <button
                            aria-label="Availability actions"
                            className="inline-flex size-8 items-center justify-center rounded-[var(--radius-md)] hover:bg-[var(--surface-muted)]"
                            onClick={() =>
                              setMenuId(menuId === record.id ? null : record.id)
                            }
                            type="button"
                          >
                            <MoreVertical className="size-4" />
                          </button>
                          {menuId === record.id ? (
                            <div className="absolute right-0 top-9 z-10 w-40 rounded-[var(--radius-lg)] border border-[var(--border)] bg-[var(--surface)] py-1 shadow-[var(--shadow-popover)]">
                              <button
                                className="flex w-full items-center gap-2 px-4 py-2 text-sm hover:bg-[var(--surface-muted)]"
                                onClick={() => {
                                  setDrawerRecord(record);
                                  setMenuId(null);
                                }}
                                type="button"
                              >
                                <Edit className="size-4" />
                                Edit
                              </button>
                              {record.status === "draft" ? (
                                <button
                                  className="flex w-full items-center gap-2 px-4 py-2 text-sm text-[var(--tone-danger)] hover:bg-[var(--tone-danger-bg)]"
                                  onClick={() => {
                                    setDeleteRecord(record);
                                    setMenuId(null);
                                  }}
                                  type="button"
                                >
                                  <Trash2 className="size-4" />
                                  Remove
                                </button>
                              ) : null}
                            </div>
                          ) : null}
                        </div>
                      )}
                    </TD>
                  </TR>
                ))}
              </TBody>
            </Table>
          </TableScroll>
        )}
      </TableWrap>
      <AvailabilityDrawer
        onClose={() => setDrawerRecord(undefined)}
        onSaved={() => {
          setDrawerRecord(undefined);
          void load();
        }}
        open={drawerRecord !== undefined}
        record={drawerRecord ?? null}
      />
      <ConfirmDrawer
        confirmLabel="Remove period"
        description={`Remove the draft ${deleteRecord ? displayStatus(deleteRecord.kind).toLowerCase() : "availability"} period?`}
        isPending={deleting}
        onClose={() => setDeleteRecord(null)}
        onConfirm={() => void remove()}
        open={deleteRecord !== null}
        title="Remove availability period?"
      />
    </div>
  );
}

function AvailabilityDrawer({
  onClose,
  onSaved,
  open,
  record,
}: {
  onClose: () => void;
  onSaved: () => void;
  open: boolean;
  record: EmployeeAvailability | null;
}) {
  const [employees, setEmployees] = useState<Employee[]>([]);
  const [employeeId, setEmployeeId] = useState("");
  const [kind, setKind] = useState<AvailabilityKind>("leave");
  const [startsAt, setStartsAt] = useState("");
  const [endsAt, setEndsAt] = useState("");
  const [status, setStatus] = useState<AvailabilityStatus>("draft");
  const [notes, setNotes] = useState("");
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  useEffect(() => {
    if (!open) return;
    setEmployeeId(record?.employee_id ?? "");
    setKind(record?.kind ?? "leave");
    setStartsAt(toLocalInput(record?.starts_at));
    setEndsAt(toLocalInput(record?.ends_at));
    setStatus(record?.status ?? "draft");
    setNotes(record?.notes ?? "");
    setLoading(true);
    void hrPayrollService
      .listEmployees({ per_page: 100 })
      .then((response) => {
        if (response.success && response.data)
          setEmployees(response.data.employees);
      })
      .finally(() => setLoading(false));
  }, [open, record]);
  const submit = async (event: React.FormEvent) => {
    event.preventDefault();
    setSaving(true);
    const data: EmployeeAvailabilityInput = {
      kind,
      starts_at: new Date(startsAt).toISOString(),
      ends_at: new Date(endsAt).toISOString(),
      status,
      notes: notes.trim() || null,
    };
    const response = record
      ? await hrPayrollService.updateEmployeeAvailability(record.id, data)
      : await hrPayrollService.createEmployeeAvailability({
          ...data,
          employee_id: employeeId,
        });
    setSaving(false);
    if (!response.success)
      return toast.error(
        hrResponseMessage(response, "Availability period could not be saved"),
      );
    toast.success("Availability period saved");
    onSaved();
  };
  const locked = record?.status === "approved";
  const statusChoices = record
    ? availabilityTransitions[record.status]
    : (["draft", "submitted"] as AvailabilityStatus[]);
  return (
    <DialogShell onClose={onClose} open={open}>
      <DialogHeader
        onClose={onClose}
        title={`${record ? "Edit" : "Add"} availability`}
      />
      <form onSubmit={submit}>
        <DialogBody className="space-y-5">
          <div>
            <Label>Employee</Label>
            <Select
              className="mt-1.5"
              data-autofocus="true"
              disabled={loading || record !== null}
              onChange={(event) => setEmployeeId(event.target.value)}
              required
              value={employeeId}
            >
              <option value="">Choose an employee</option>
              {employees.map((employee) => (
                <option key={employee.id} value={employee.id}>
                  {employee.display_name} · {employee.employee_number}
                </option>
              ))}
            </Select>
          </div>
          <div className="grid gap-4 sm:grid-cols-2">
            <div>
              <Label>Type</Label>
              <Select
                className="mt-1.5"
                disabled={locked}
                onChange={(event) =>
                  setKind(event.target.value as AvailabilityKind)
                }
                value={kind}
              >
                {availabilityKinds.map((item) => (
                  <option key={item} value={item}>
                    {displayStatus(item)}
                  </option>
                ))}
              </Select>
            </div>
            <div>
              <Label>Status</Label>
              <Select
                className="mt-1.5"
                onChange={(event) =>
                  setStatus(event.target.value as AvailabilityStatus)
                }
                value={status}
              >
                {statusChoices.map((item) => (
                  <option key={item} value={item}>
                    {displayStatus(item)}
                  </option>
                ))}
              </Select>
            </div>
          </div>
          <div>
            <Label>Starts</Label>
            <Input
              className="mt-1.5"
              disabled={locked}
              onChange={(event) => setStartsAt(event.target.value)}
              required
              type="datetime-local"
              value={startsAt}
            />
          </div>
          <div>
            <Label>Ends</Label>
            <Input
              className="mt-1.5"
              disabled={locked}
              onChange={(event) => setEndsAt(event.target.value)}
              required
              type="datetime-local"
              value={endsAt}
            />
          </div>
          {locked ? (
            <p className="rounded-[var(--radius-lg)] bg-[var(--surface-muted)] p-4 text-sm text-[var(--text-muted)]">
              Approved dates cannot be rewritten. Cancel this period and add a
              replacement if the schedule changes.
            </p>
          ) : null}
          <div>
            <Label>Notes</Label>
            <Textarea
              className="mt-1.5 min-h-28"
              disabled={locked}
              maxLength={4000}
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
            variant="ghost"
          >
            Cancel
          </Button>
          <Button disabled={saving || loading} type="submit">
            {saving ? (
              <>
                <Loader2 className="size-4 animate-spin" />
                Saving…
              </>
            ) : (
              "Save availability"
            )}
          </Button>
        </DialogFooter>
      </form>
    </DialogShell>
  );
}

const availabilityKinds: AvailabilityKind[] = [
  "leave",
  "training",
  "medical",
  "personal",
  "other",
];
const availabilityStatuses: AvailabilityStatus[] = [
  "draft",
  "submitted",
  "approved",
  "rejected",
  "cancelled",
];
const availabilityTransitions: Record<
  AvailabilityStatus,
  AvailabilityStatus[]
> = {
  draft: ["draft", "submitted", "cancelled"],
  submitted: ["submitted", "approved", "rejected", "cancelled"],
  approved: ["approved", "cancelled"],
  rejected: ["rejected"],
  cancelled: ["cancelled"],
};
function displayStatus(value: string) {
  return value
    .replace(/_/g, " ")
    .replace(/^./, (letter) => letter.toUpperCase());
}
function availabilityTone(
  status: AvailabilityStatus,
): "neutral" | "success" | "danger" | "warning" | "info" {
  if (status === "approved") return "success";
  if (status === "rejected" || status === "cancelled") return "danger";
  if (status === "submitted") return "info";
  return "warning";
}
function formatDateTime(value: string) {
  return new Intl.DateTimeFormat(undefined, {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(new Date(value));
}
function toLocalInput(value?: string | null) {
  if (!value) return "";
  const date = new Date(value);
  const offset = date.getTimezoneOffset() * 60_000;
  return new Date(date.getTime() - offset).toISOString().slice(0, 16);
}

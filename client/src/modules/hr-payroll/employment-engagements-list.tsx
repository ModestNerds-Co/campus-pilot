import { useCallback, useEffect, useState } from "react";
import {
  BriefcaseBusiness,
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
import { hasPermission } from "@/modules/users/access-control";
import { useAuthStore } from "@/stores/auth-store";

import { hrPayrollService, hrResponseMessage } from "./service";
import type {
  Department,
  Employee,
  EmploymentEngagement,
  EmploymentEngagementInput,
  EmploymentType,
  EngagementStatus,
  Position,
} from "./types";

export function EmploymentEngagementsList() {
  const permissions = useAuthStore((state) => state.user?.permissions);
  const canCreate = hasPermission(permissions, "hr_payroll:create");
  const canEdit = hasPermission(permissions, "hr_payroll:edit");
  const canDelete = hasPermission(permissions, "hr_payroll:delete");
  const hasActions = canEdit || canDelete;
  const [records, setRecords] = useState<EmploymentEngagement[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [search, setSearch] = useState("");
  const [submittedSearch, setSubmittedSearch] = useState("");
  const [status, setStatus] = useState<"all" | EngagementStatus>("all");
  const [page, setPage] = useState(1);
  const [totalPages, setTotalPages] = useState(1);
  const [drawerRecord, setDrawerRecord] = useState<
    EmploymentEngagement | null | undefined
  >(undefined);
  const [deleteRecord, setDeleteRecord] = useState<EmploymentEngagement | null>(
    null,
  );
  const [menuId, setMenuId] = useState<string | null>(null);
  const [deleting, setDeleting] = useState(false);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const response = await hrPayrollService.listEmploymentEngagements({
        page,
        per_page: 20,
        search: submittedSearch || undefined,
        status: status === "all" ? undefined : status,
      });
      if (!response.success || !response.data)
        throw new Error(
          hrResponseMessage(response, "Employment history could not be loaded"),
        );
      setRecords(response.data.employment_engagements);
      setTotalPages(response.pagination?.total_pages ?? 1);
    } catch (loadError) {
      setError(
        loadError instanceof Error
          ? loadError.message
          : "Employment history could not be loaded",
      );
    } finally {
      setLoading(false);
    }
  }, [page, status, submittedSearch]);

  useEffect(() => {
    void load();
  }, [load]);
  const remove = async () => {
    if (!canDelete || !deleteRecord || deleting) return;
    setDeleting(true);
    const response = await hrPayrollService.deleteEmploymentEngagement(
      deleteRecord.id,
    );
    setDeleting(false);
    if (!response.success)
      return toast.error(
        hrResponseMessage(
          response,
          "Employment engagement could not be removed",
        ),
      );
    toast.success("Employment engagement removed");
    setDeleteRecord(null);
    void load();
  };

  usePageChrome(
    "Employment",
    canCreate ? <Button onClick={() => setDrawerRecord(null)}>
      <Plus className="size-4" />
      Add engagement
    </Button> : undefined,
  );
  const filtered = submittedSearch || status !== "all";

  return (
    <div className="space-y-6">
      <p className="text-sm text-[var(--text-muted)]">
        Dated engagements hold each employee&apos;s contract and assignment
        history. The active engagement updates the workforce directory.
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
            aria-label="Search employment history"
            leadingIcon={<Search />}
            onChange={(event) => setSearch(event.target.value)}
            placeholder="Search employee or reference…"
            value={search}
          />
          <Button type="submit" variant="secondary">
            Search
          </Button>
        </TableControlsSearch>
        <Select
          aria-label="Engagement status"
          className="sm:w-44"
          onChange={(event) => {
            setPage(1);
            setStatus(event.target.value as typeof status);
          }}
          value={status}
        >
          <option value="all">All statuses</option>
          {engagementStatuses.map((item) => (
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
          <TableLoading columns={hasActions ? 6 : 5} label="Loading employment history…" />
        ) : error ? (
          <TableError description={error} onRetry={() => void load()} />
        ) : records.length === 0 ? (
          <TableEmpty
            description={
              filtered
                ? "Change the current filters."
                : canCreate
                  ? "Add the first employment engagement."
                  : "No employment history is available."
            }
            icon={<BriefcaseBusiness />}
            title={
              filtered
                ? "No engagements match these filters"
                : "No employment history yet"
            }
          />
        ) : (
          <TableScroll>
            <Table>
              <THead>
                <tr>
                  <TH>Employee</TH>
                  <TH>Engagement</TH>
                  <TH>Assignment</TH>
                  <TH>Dates</TH>
                  <TH>Status</TH>
                  {hasActions ? <TH className="text-right">Actions</TH> : null}
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
                    <TD>
                      <div className="text-[var(--text-strong)]">
                        {displayStatus(record.employment_type)}
                      </div>
                      <div className="font-tabular text-xs text-[var(--text-muted)]">
                        {record.reference ||
                          `${record.workload_basis_points / 100}% workload`}
                      </div>
                    </TD>
                    <TD>
                      <div className="text-[var(--text-strong)]">
                        {record.position_title || "No position"}
                      </div>
                      <div className="text-xs text-[var(--text-muted)]">
                        {record.department_name || "No department"}
                      </div>
                    </TD>
                    <TD>
                      <div className="font-tabular text-sm">
                        {formatDate(record.start_date)}
                      </div>
                      <div className="font-tabular text-xs text-[var(--text-muted)]">
                        {record.end_date
                          ? `to ${formatDate(record.end_date)}`
                          : "No end date"}
                      </div>
                    </TD>
                    <TD>
                      <Badge tone={engagementTone(record.status)}>
                        {displayStatus(record.status)}
                      </Badge>
                    </TD>
                    {hasActions ? <TD className="text-right">
                      {(record.status === "ended" || record.status === "cancelled" || !canEdit) &&
                      !(record.status === "draft" && canDelete) ? (
                        <span className="text-[var(--text-subtle)]">—</span>
                      ) : (
                        <div className="relative inline-flex">
                          <button
                            aria-label="Employment engagement actions"
                            className="inline-flex size-8 items-center justify-center rounded-[var(--radius-md)] hover:bg-[var(--surface-muted)]"
                            onClick={() =>
                              setMenuId(menuId === record.id ? null : record.id)
                            }
                            type="button"
                          >
                            <MoreVertical className="size-4" />
                          </button>
                          {menuId === record.id ? (
                            <div className="absolute right-0 top-9 z-10 w-44 rounded-[var(--radius-lg)] border border-[var(--border)] bg-[var(--surface)] py-1 shadow-[var(--shadow-popover)]">
                              {canEdit ? <button
                                className="flex w-full items-center gap-2 px-4 py-2 text-sm hover:bg-[var(--surface-muted)]"
                                onClick={() => {
                                  setDrawerRecord(record);
                                  setMenuId(null);
                                }}
                                type="button"
                              >
                                <Edit className="size-4" />
                                Edit
                              </button> : null}
                              {record.status === "draft" && canDelete ? (
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
                    </TD> : null}
                  </TR>
                ))}
              </TBody>
            </Table>
          </TableScroll>
        )}
      </TableWrap>
      <EmploymentDrawer
        onClose={() => setDrawerRecord(undefined)}
        onSaved={() => {
          setDrawerRecord(undefined);
          void load();
        }}
        open={(drawerRecord === null && canCreate) || (drawerRecord !== null && drawerRecord !== undefined && canEdit)}
        record={drawerRecord ?? null}
      />
      <ConfirmDrawer
        confirmLabel="Remove engagement"
        description={`Remove ${deleteRecord?.reference || "this employment engagement"}?`}
        isPending={deleting}
        onClose={() => setDeleteRecord(null)}
        onConfirm={() => void remove()}
        open={canDelete && deleteRecord !== null}
        title="Remove employment engagement?"
      />
    </div>
  );
}

function EmploymentDrawer({
  onClose,
  onSaved,
  open,
  record,
}: {
  onClose: () => void;
  onSaved: () => void;
  open: boolean;
  record: EmploymentEngagement | null;
}) {
  const [employees, setEmployees] = useState<Employee[]>([]);
  const [departments, setDepartments] = useState<Department[]>([]);
  const [positions, setPositions] = useState<Position[]>([]);
  const [employeeId, setEmployeeId] = useState("");
  const [reference, setReference] = useState("");
  const [employmentType, setEmploymentType] =
    useState<EmploymentType>("permanent");
  const [departmentId, setDepartmentId] = useState("");
  const [positionId, setPositionId] = useState("");
  const [status, setStatus] = useState<EngagementStatus>("draft");
  const [startDate, setStartDate] = useState("");
  const [endDate, setEndDate] = useState("");
  const [workload, setWorkload] = useState("100");
  const [notes, setNotes] = useState("");
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    if (!open) return;
    setEmployeeId(record?.employee_id ?? "");
    setReference(record?.reference ?? "");
    setEmploymentType(record?.employment_type ?? "permanent");
    setDepartmentId(record?.department_id ?? "");
    setPositionId(record?.position_id ?? "");
    setStatus(record?.status ?? "draft");
    setStartDate(record?.start_date ?? "");
    setEndDate(record?.end_date ?? "");
    setWorkload(String((record?.workload_basis_points ?? 10_000) / 100));
    setNotes(record?.notes ?? "");
    setLoading(true);
    void Promise.all([
      hrPayrollService.listEmployees({ per_page: 100 }),
      hrPayrollService.listDepartments({ per_page: 100, status: "active" }),
      hrPayrollService.listPositions({ per_page: 100, status: "active" }),
    ])
      .then(([employeeResponse, departmentResponse, positionResponse]) => {
        if (employeeResponse.success && employeeResponse.data)
          setEmployees(employeeResponse.data.employees);
        if (departmentResponse.success && departmentResponse.data)
          setDepartments(departmentResponse.data.departments);
        if (positionResponse.success && positionResponse.data)
          setPositions(positionResponse.data.positions);
      })
      .finally(() => setLoading(false));
  }, [open, record]);

  const submit = async (event: React.FormEvent) => {
    event.preventDefault();
    setSaving(true);
    const data: EmploymentEngagementInput = {
      reference: reference.trim() || null,
      employment_type: employmentType,
      department_id: departmentId || null,
      position_id: positionId || null,
      status,
      start_date: startDate,
      end_date: endDate || null,
      workload_basis_points: Math.round(Number(workload) * 100),
      notes: notes.trim() || null,
    };
    const response = record
      ? await hrPayrollService.updateEmploymentEngagement(record.id, data)
      : await hrPayrollService.createEmploymentEngagement({
          ...data,
          employee_id: employeeId,
        });
    setSaving(false);
    if (!response.success)
      return toast.error(
        hrResponseMessage(response, "Employment engagement could not be saved"),
      );
    toast.success("Employment engagement saved");
    onSaved();
  };
  const availablePositions = positions.filter(
    (item) =>
      !departmentId ||
      !item.department_id ||
      item.department_id === departmentId,
  );
  const statusChoices = record
    ? engagementTransitions[record.status]
    : (["draft", "active"] as EngagementStatus[]);

  return (
    <DialogShell onClose={onClose} open={open}>
      <DialogHeader
        onClose={onClose}
        title={`${record ? "Edit" : "Add"} employment engagement`}
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
              <Label>Employment type</Label>
              <Select
                className="mt-1.5"
                onChange={(event) =>
                  setEmploymentType(event.target.value as EmploymentType)
                }
                value={employmentType}
              >
                {employmentTypes.map((item) => (
                  <option key={item} value={item}>
                    {displayStatus(item)}
                  </option>
                ))}
              </Select>
            </div>
            <div>
              <Label>Reference</Label>
              <Input
                className="mt-1.5"
                maxLength={80}
                onChange={(event) => setReference(event.target.value)}
                placeholder="Optional contract reference"
                value={reference}
              />
            </div>
          </div>
          <div className="grid gap-4 sm:grid-cols-2">
            <div>
              <Label>Department</Label>
              <Select
                className="mt-1.5"
                onChange={(event) => {
                  setDepartmentId(event.target.value);
                  setPositionId("");
                }}
                value={departmentId}
              >
                <option value="">No department</option>
                {departments.map((department) => (
                  <option key={department.id} value={department.id}>
                    {department.name}
                  </option>
                ))}
              </Select>
            </div>
            <div>
              <Label>Position</Label>
              <Select
                className="mt-1.5"
                onChange={(event) => setPositionId(event.target.value)}
                value={positionId}
              >
                <option value="">No position</option>
                {availablePositions.map((position) => (
                  <option key={position.id} value={position.id}>
                    {position.title}
                  </option>
                ))}
              </Select>
            </div>
          </div>
          <div className="grid gap-4 sm:grid-cols-2">
            <div>
              <Label>Start date</Label>
              <Input
                className="mt-1.5"
                onChange={(event) => setStartDate(event.target.value)}
                required
                type="date"
                value={startDate}
              />
            </div>
            <div>
              <Label>End date</Label>
              <Input
                className="mt-1.5"
                onChange={(event) => setEndDate(event.target.value)}
                required={employmentType === "fixed_term" || status === "ended"}
                type="date"
                value={endDate}
              />
            </div>
          </div>
          <div className="grid gap-4 sm:grid-cols-2">
            <div>
              <Label>Workload (%)</Label>
              <Input
                className="mt-1.5"
                max="100"
                min="0.01"
                onChange={(event) => setWorkload(event.target.value)}
                required
                step="0.01"
                type="number"
                value={workload}
              />
            </div>
            <div>
              <Label>Status</Label>
              <Select
                className="mt-1.5"
                onChange={(event) =>
                  setStatus(event.target.value as EngagementStatus)
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
            <Label>Notes</Label>
            <Textarea
              className="mt-1.5 min-h-28"
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
              "Save engagement"
            )}
          </Button>
        </DialogFooter>
      </form>
    </DialogShell>
  );
}

const employmentTypes: EmploymentType[] = [
  "permanent",
  "fixed_term",
  "temporary",
  "casual",
  "contractor",
  "intern",
];
const engagementStatuses: EngagementStatus[] = [
  "draft",
  "active",
  "ended",
  "cancelled",
];
const engagementTransitions: Record<EngagementStatus, EngagementStatus[]> = {
  draft: ["draft", "active", "cancelled"],
  active: ["active", "ended"],
  ended: ["ended"],
  cancelled: ["cancelled"],
};
function displayStatus(value: string) {
  return value
    .replace(/_/g, " ")
    .replace(/^./, (letter) => letter.toUpperCase());
}
function engagementTone(
  status: EngagementStatus,
): "neutral" | "success" | "danger" | "warning" {
  if (status === "active") return "success";
  if (status === "cancelled") return "danger";
  if (status === "draft") return "warning";
  return "neutral";
}
function formatDate(value: string | null) {
  if (!value) return "Unknown";
  return new Intl.DateTimeFormat(undefined, {
    day: "numeric",
    month: "short",
    year: "numeric",
    timeZone: "UTC",
  }).format(new Date(`${value}T00:00:00Z`));
}

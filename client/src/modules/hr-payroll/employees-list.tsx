import { useCallback, useEffect, useState } from "react";
import { BriefcaseBusiness, Edit, KeyRound, Loader2, MoreVertical, Plus, Search, Trash2, UserRound } from "lucide-react";
import toast from "react-hot-toast";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { ConfirmDrawer } from "@/components/ui/confirm-drawer";
import { Table, TableControlsBar, TableControlsPagination, TableControlsSearch, TableEmpty, TableError, TableLoading, TableScroll, TableWrap, TBody, TD, TH, THead, TR } from "@/components/ui/data-table";
import { DialogBody, DialogFooter, DialogHeader, DialogShell } from "@/components/ui/dialog";
import { Input, Label, Select } from "@/components/ui/input";
import { usePageChrome } from "@/modules/admin/layouts/page-chrome";
import { usersService } from "@/modules/users/services/users-service";
import type { User } from "@/modules/users/types";

import { hrPayrollService } from "./service";
import type { Department, Employee, EmployeeInput, EmploymentStatus, Position } from "./types";

export function EmployeesList() {
  const [employees, setEmployees] = useState<Employee[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [search, setSearch] = useState("");
  const [submittedSearch, setSubmittedSearch] = useState("");
  const [status, setStatus] = useState<"all" | EmploymentStatus>("all");
  const [page, setPage] = useState(1);
  const [totalPages, setTotalPages] = useState(1);
  const [drawerEmployee, setDrawerEmployee] = useState<Employee | null | undefined>(undefined);
  const [accountEmployee, setAccountEmployee] = useState<Employee | null>(null);
  const [deleteEmployee, setDeleteEmployee] = useState<Employee | null>(null);
  const [menuId, setMenuId] = useState<string | null>(null);

  const load = useCallback(async () => {
    setLoading(true); setError(null);
    try {
      const response = await hrPayrollService.listEmployees({ page, per_page: 20, search: submittedSearch || undefined, status: status === "all" ? undefined : status });
      if (!response.success || !response.data) throw new Error(response.message || "Employees could not be loaded");
      setEmployees(response.data.employees); setTotalPages(response.pagination?.total_pages ?? 1);
    } catch (loadError) { setError(loadError instanceof Error ? loadError.message : "Employees could not be loaded"); }
    finally { setLoading(false); }
  }, [page, status, submittedSearch]);

  useEffect(() => { void load(); }, [load]);
  const remove = async () => {
    if (!deleteEmployee) return;
    const response = await hrPayrollService.deleteEmployee(deleteEmployee.id);
    if (response.success) { toast.success("Employee removed"); setDeleteEmployee(null); void load(); }
    else toast.error(response.message || "Employee could not be removed");
  };

  usePageChrome("Employees", <Button onClick={() => setDrawerEmployee(null)}><Plus className="size-4" />Add employee</Button>);
  return <div className="space-y-6">
    <p className="text-sm text-[var(--text-muted)]">Employees are the shared workforce records used by HR, Fleet, and other campus modules.</p>
    <TableControlsBar><TableControlsSearch onSubmit={(event) => { event.preventDefault(); setPage(1); setSubmittedSearch(search.trim()); }}><Input aria-label="Search employees" leadingIcon={<Search />} onChange={(event) => setSearch(event.target.value)} placeholder="Search name, number, or work email…" value={search} /><Button type="submit" variant="secondary">Search</Button></TableControlsSearch>
      <Select aria-label="Employment status" className="sm:w-44" onChange={(event) => { setPage(1); setStatus(event.target.value as typeof status); }} value={status}><option value="all">All statuses</option><option value="active">Active</option><option value="inactive">Inactive</option><option value="suspended">Suspended</option><option value="terminated">Terminated</option></Select>
      {!loading && employees.length ? <TableControlsPagination onNext={() => setPage((value) => Math.min(totalPages, value + 1))} onPrevious={() => setPage((value) => Math.max(1, value - 1))} page={page} totalPages={totalPages} /> : null}
    </TableControlsBar>
    <TableWrap>{loading ? <TableLoading columns={5} label="Loading employees…" /> : error ? <TableError description={error} onRetry={() => void load()} /> : employees.length === 0 ? <TableEmpty description={submittedSearch || status !== "all" ? "Change the current filters." : "Add the first campus employee."} icon={<UserRound />} title={submittedSearch || status !== "all" ? "No employees match these filters" : "No employees yet"} /> : <TableScroll><Table><THead><tr><TH>Employee</TH><TH>Assignment</TH><TH>System account</TH><TH>Status</TH><TH className="text-right">Actions</TH></tr></THead><TBody>
      {employees.map((employee) => <TR key={employee.id}><TD><div className="font-medium text-[var(--text-strong)]">{employee.display_name}</div><div className="font-tabular text-xs text-[var(--text-muted)]">{employee.employee_number}{employee.work_email ? ` · ${employee.work_email}` : ""}</div></TD><TD><div className="text-[var(--text-strong)]">{employee.position_title || "—"}</div><div className="text-xs text-[var(--text-muted)]">{employee.department_name || "No department"}</div></TD><TD>{employee.account_email ? <span className="text-[var(--text-strong)]">{employee.account_email}</span> : <span className="text-[var(--text-muted)]">Not linked</span>}</TD><TD><Badge tone={employee.employment_status === "active" ? "success" : employee.employment_status === "suspended" ? "warning" : "neutral"}>{employee.employment_status}</Badge></TD><TD className="text-right"><div className="relative inline-flex"><button aria-label="Employee actions" className="inline-flex size-8 items-center justify-center rounded-[var(--radius-md)] hover:bg-[var(--surface-muted)]" onClick={() => setMenuId(menuId === employee.id ? null : employee.id)} type="button"><MoreVertical className="size-4" /></button>{menuId === employee.id ? <div className="absolute right-0 top-9 z-10 w-48 rounded-[var(--radius-lg)] border border-[var(--border)] bg-[var(--surface)] py-1 shadow-[var(--shadow-popover)]"><button className="flex w-full items-center gap-2 px-4 py-2 text-sm hover:bg-[var(--surface-muted)]" onClick={() => { setDrawerEmployee(employee); setMenuId(null); }}><Edit className="size-4" />Edit employee</button><button className="flex w-full items-center gap-2 px-4 py-2 text-sm hover:bg-[var(--surface-muted)]" onClick={() => { setAccountEmployee(employee); setMenuId(null); }}><KeyRound className="size-4" />{employee.account_id ? "Change account" : "Link account"}</button><button className="flex w-full items-center gap-2 px-4 py-2 text-sm text-[var(--tone-danger)] hover:bg-[var(--tone-danger-bg)]" onClick={() => { setDeleteEmployee(employee); setMenuId(null); }}><Trash2 className="size-4" />Remove</button></div> : null}</div></TD></TR>)}
    </TBody></Table></TableScroll>}</TableWrap>
    <EmployeeDrawer employee={drawerEmployee ?? null} onClose={() => setDrawerEmployee(undefined)} onSaved={() => { setDrawerEmployee(undefined); void load(); }} open={drawerEmployee !== undefined} />
    <AccountDrawer employee={accountEmployee} onClose={() => setAccountEmployee(null)} onSaved={() => { setAccountEmployee(null); void load(); }} />
    <ConfirmDrawer confirmLabel="Remove employee" description={`Remove ${deleteEmployee?.display_name || "this employee"}? Active module profiles, such as a Fleet driver profile, must be removed first.`} onClose={() => setDeleteEmployee(null)} onConfirm={() => void remove()} open={deleteEmployee !== null} title="Remove employee?" />
  </div>;
}

function EmployeeDrawer({ employee, onClose, onSaved, open }: { employee: Employee | null; onClose: () => void; onSaved: () => void; open: boolean }) {
  const [departments, setDepartments] = useState<Department[]>([]); const [positions, setPositions] = useState<Position[]>([]);
  const [form, setForm] = useState<EmployeeInput>({ employee_number: "", display_name: "", employment_status: "active" }); const [saving, setSaving] = useState(false);
  useEffect(() => {
    if (!open) return;
    setForm(employee ? { employee_number: employee.employee_number, display_name: employee.display_name, first_names: employee.first_names, surname: employee.surname, work_email: employee.work_email, phone: employee.phone, department_id: employee.department_id, position_id: employee.position_id, employment_status: employee.employment_status, hire_date: employee.hire_date, end_date: employee.end_date } : { employee_number: "", display_name: "", employment_status: "active" });
    void Promise.all([hrPayrollService.listDepartments({ per_page: 100, status: "active" }), hrPayrollService.listPositions({ per_page: 100, status: "active" })]).then(([departmentResponse, positionResponse]) => { if (departmentResponse.success && departmentResponse.data) setDepartments(departmentResponse.data.departments); if (positionResponse.success && positionResponse.data) setPositions(positionResponse.data.positions); });
  }, [employee, open]);
  const field = <K extends keyof EmployeeInput>(key: K, value: EmployeeInput[K]) => setForm((current) => ({ ...current, [key]: value }));
  const submit = async (event: React.FormEvent) => { event.preventDefault(); setSaving(true); try { const response = employee ? await hrPayrollService.updateEmployee(employee.id, form) : await hrPayrollService.createEmployee(form); if (!response.success) throw new Error(response.message || "Employee could not be saved"); toast.success("Employee saved"); onSaved(); } catch (error) { toast.error(error instanceof Error ? error.message : "Employee could not be saved"); } finally { setSaving(false); } };
  const availablePositions = positions.filter((position) => !form.department_id || !position.department_id || position.department_id === form.department_id);
  return <DialogShell onClose={onClose} open={open}><DialogHeader onClose={onClose} title={employee ? "Edit employee" : "Add employee"} /><form onSubmit={submit}><DialogBody className="space-y-5">
    <div className="grid gap-4 sm:grid-cols-2"><div><Label>Employee number</Label><Input className="mt-1.5" onChange={(event) => field("employee_number", event.target.value)} required value={form.employee_number} /></div><div><Label>Display name</Label><Input className="mt-1.5" onChange={(event) => field("display_name", event.target.value)} required value={form.display_name} /></div></div>
    <div className="grid gap-4 sm:grid-cols-2"><div><Label>First names</Label><Input className="mt-1.5" onChange={(event) => field("first_names", event.target.value || null)} value={form.first_names ?? ""} /></div><div><Label>Surname</Label><Input className="mt-1.5" onChange={(event) => field("surname", event.target.value || null)} value={form.surname ?? ""} /></div></div>
    <div className="grid gap-4 sm:grid-cols-2"><div><Label>Work email</Label><Input className="mt-1.5" onChange={(event) => field("work_email", event.target.value || null)} type="email" value={form.work_email ?? ""} /></div><div><Label>Phone</Label><Input className="mt-1.5" onChange={(event) => field("phone", event.target.value || null)} type="tel" value={form.phone ?? ""} /></div></div>
    <div className="grid gap-4 sm:grid-cols-2"><div><Label>Department</Label><Select className="mt-1.5" onChange={(event) => { field("department_id", event.target.value || null); field("position_id", null); }} value={form.department_id ?? ""}><option value="">No department</option>{departments.map((department) => <option key={department.id} value={department.id}>{department.name}</option>)}</Select></div><div><Label>Position</Label><Select className="mt-1.5" onChange={(event) => field("position_id", event.target.value || null)} value={form.position_id ?? ""}><option value="">No position</option>{availablePositions.map((position) => <option key={position.id} value={position.id}>{position.title}</option>)}</Select></div></div>
    <div className="grid gap-4 sm:grid-cols-3"><div><Label>Status</Label><Select className="mt-1.5" onChange={(event) => field("employment_status", event.target.value as EmploymentStatus)} value={form.employment_status}><option value="active">Active</option><option value="inactive">Inactive</option><option value="suspended">Suspended</option><option value="terminated">Terminated</option></Select></div><div><Label>Hire date</Label><Input className="mt-1.5" onChange={(event) => field("hire_date", event.target.value || null)} type="date" value={form.hire_date ?? ""} /></div><div><Label>End date</Label><Input className="mt-1.5" onChange={(event) => field("end_date", event.target.value || null)} type="date" value={form.end_date ?? ""} /></div></div>
  </DialogBody><DialogFooter><Button disabled={saving} onClick={onClose} type="button" variant="ghost">Cancel</Button><Button disabled={saving} type="submit">{saving ? <><Loader2 className="size-4 animate-spin" />Saving…</> : "Save employee"}</Button></DialogFooter></form></DialogShell>;
}

function AccountDrawer({ employee, onClose, onSaved }: { employee: Employee | null; onClose: () => void; onSaved: () => void }) {
  const [users, setUsers] = useState<User[]>([]); const [accountId, setAccountId] = useState(""); const [saving, setSaving] = useState(false);
  useEffect(() => { if (!employee) return; setAccountId(employee.account_id ?? ""); void usersService.listUsers({ per_page: 100, status: "active" }).then((response) => { if (response.success && response.data) setUsers(response.data.users); }); }, [employee]);
  const submit = async (event: React.FormEvent) => { event.preventDefault(); if (!employee) return; setSaving(true); const response = await hrPayrollService.linkEmployeeAccount(employee.id, accountId || null); setSaving(false); if (response.success) { toast.success(accountId ? "System account linked" : "System account unlinked"); onSaved(); } else toast.error(response.message || "Account link could not be changed"); };
  return <DialogShell onClose={onClose} open={employee !== null}><DialogHeader onClose={onClose} title="System account" /><form onSubmit={submit}><DialogBody className="space-y-5"><div className="flex items-start gap-3 rounded-[var(--radius-lg)] bg-[var(--surface-muted)] p-4"><BriefcaseBusiness className="mt-0.5 size-5 text-[var(--brand-strong)]" /><div><p className="font-medium text-[var(--text-strong)]">{employee?.display_name}</p><p className="mt-1 text-sm text-[var(--text-muted)]">Linking an account lets this employee sign in. Their employee record remains the source used by operational modules.</p></div></div><div><Label>System user</Label><Select className="mt-1.5" onChange={(event) => setAccountId(event.target.value)} value={accountId}><option value="">No linked account</option>{users.map((user) => <option key={user.id} value={user.id}>{user.full_name} · {user.email}</option>)}</Select></div></DialogBody><DialogFooter><Button disabled={saving} onClick={onClose} type="button" variant="ghost">Cancel</Button><Button disabled={saving} type="submit">{saving ? <Loader2 className="size-4 animate-spin" /> : <KeyRound className="size-4" />}Save account link</Button></DialogFooter></form></DialogShell>;
}

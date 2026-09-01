/**
 * Client-side boundary for HR administration workspaces.
 *
 * `hr_payroll:view` retains the employee self-service/read-only surfaces. The
 * API remains authoritative for every read and mutation.
 */
export const HR_ADMINISTRATION_PERMISSIONS = [
  "hr_payroll:create",
  "hr_payroll:edit",
  "hr_payroll:delete",
] as const;

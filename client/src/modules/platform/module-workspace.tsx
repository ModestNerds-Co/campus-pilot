import React, { useEffect, useState } from "react";
import { Link } from "@tanstack/react-router";
import { ArrowRight, CheckCircle2, CircleDashed } from "lucide-react";

import { ProtectedRoute } from "@/components/protected-route";
import { usePageChrome } from "@/modules/admin/layouts/page-chrome";

import { accessService } from "./access-service";
import {
  defaultModuleVisual,
  moduleVisuals,
  stageLabel,
} from "./module-registry";
import type { ModuleDefinition } from "./types";
import { TimetableWorkspace } from "@/modules/timetabling";
import { useAuthStore } from "@/stores/auth-store";
import { LibraryCatalogueWorkspace } from "@/modules/library";
import { HealthPatientsWorkspace } from "@/modules/health";
import { HostelResidencesWorkspace } from "@/modules/hostel";
import { DocumentRegistryDocumentsWorkspace } from "@/modules/document-registry";
import { InternalAuditEngagementsWorkspace } from "@/modules/internal-audit";

export const ModuleWorkspace: React.FC<{ moduleKey: string }> = ({
  moduleKey,
}) => {
  const normalizedKey = moduleKey.replace(/-/g, "_");
  const [module, setModule] = useState<ModuleDefinition | null>(null);
  const [isLoading, setIsLoading] = useState(true);

  useEffect(() => {
    let active = true;
    void accessService
      .getCatalog()
      .then((response) => {
        if (active && response.success && response.data) {
          setModule(
            response.data.modules.find((item) => item.key === normalizedKey) ??
              null,
          );
        }
      })
      .finally(() => {
        if (active) setIsLoading(false);
      });
    return () => {
      active = false;
    };
  }, [normalizedKey]);

  if (isLoading)
    return (
      <div className="h-64 animate-pulse rounded-[var(--radius-xl)] bg-[var(--surface-sunken)]" />
    );
  if (!module) return <UnknownModule />;

  return (
    <ProtectedRoute
      requiredModule={module.key}
      requiredPermission={`${module.permission_namespace}:view`}
    >
      {module.key === "timetabling" ? (
        <TimetableWorkspace module={module} />
      ) : module.key === "library" ? (
        <LibraryCatalogueWorkspace />
      ) : module.key === "health" ? (
        <HealthPatientsWorkspace />
      ) : module.key === "hostel" ? (
        <HostelResidencesWorkspace />
      ) : module.key === "document_registry" ? (
        <DocumentRegistryDocumentsWorkspace />
      ) : module.key === "internal_audit" ? (
        <InternalAuditEngagementsWorkspace />
      ) : (
        <ModuleFoundation module={module} />
      )}
    </ProtectedRoute>
  );
};

const ModuleFoundation: React.FC<{ module: ModuleDefinition }> = ({
  module,
}) => {
  const permissions = useAuthStore((state) => state.user?.permissions ?? []);
  const enabledModules = useAuthStore((state) => state.user?.modules ?? []);
  const visual = moduleVisuals[module.key] ?? defaultModuleVisual;
  const Icon = visual.icon;
  usePageChrome("Overview");

  if (module.key === "fleet") {
    return (
      <div className="space-y-8">
        <ModuleIntroduction module={module} />
        <section aria-labelledby="fleet-workspaces">
          <p className="text-[11px] font-semibold uppercase tracking-[0.16em] text-[var(--brand-strong)]">
            Working areas
          </p>
          <h2
            className="mt-1 text-xl font-semibold tracking-[-0.025em] text-[var(--text-strong)]"
            id="fleet-workspaces"
          >
            Run daily fleet operations
          </h2>
          <div className="mt-4 grid gap-4 md:grid-cols-3">
            <FleetLink
              description="Register and maintain the campus vehicle record."
              label="Vehicles"
              to="/modules/fleet/vehicles"
            />
            <FleetLink
              description="Manage authorized drivers and licence details."
              label="Drivers"
              to="/modules/fleet/drivers"
            />
            <FleetLink
              description="Record journeys, mileage, fuel, and daily checks."
              label="Daily vehicle log"
              to="/modules/fleet/daily-log"
            />
          </div>
        </section>
      </div>
    );
  }

  if (module.key === "hr_payroll") {
    return (
      <div className="space-y-8">
        <ModuleIntroduction module={module} />
        <section aria-labelledby="hr-workspaces">
          <p className="text-[11px] font-semibold uppercase tracking-[0.16em] text-[var(--brand-strong)]">
            Working areas
          </p>
          <h2
            className="mt-1 text-xl font-semibold tracking-[-0.025em] text-[var(--text-strong)]"
            id="hr-workspaces"
          >
            Manage the workforce directory
          </h2>
          <div className="mt-4 grid gap-4 md:grid-cols-3">
            <HrLink
              description="Maintain the canonical record used across campus modules."
              label="Employees"
              to="/modules/hr-payroll/employees"
            />
            <HrLink
              description="Map and validate existing employee records before importing them."
              label="Employee imports"
              to="/modules/hr-payroll/imports"
            />
            <HrLink
              description="Keep dated contract and assignment history."
              label="Employment"
              to="/modules/hr-payroll/employment"
            />
            <HrLink
              description="Record reviewed workforce scheduling constraints."
              label="Availability"
              to="/modules/hr-payroll/availability"
            />
            <HrLink
              description="Organize employees by operational area."
              label="Departments"
              to="/modules/hr-payroll/departments"
            />
            <HrLink
              description="Maintain the positions employees may hold."
              label="Positions"
              to="/modules/hr-payroll/positions"
            />
          </div>
        </section>
      </div>
    );
  }

  if (module.key === "academics") {
    return (
      <div className="space-y-8">
        <ModuleIntroduction module={module} />
        <section aria-labelledby="academics-workspaces">
          <p className="text-[11px] font-semibold uppercase tracking-[0.16em] text-[var(--brand-strong)]">
            Working areas
          </p>
          <h2
            className="mt-1 text-xl font-semibold tracking-[-0.025em] text-[var(--text-strong)]"
            id="academics-workspaces"
          >
            Manage the teaching structure
          </h2>
          <div className="mt-4 grid gap-4 md:grid-cols-2 xl:grid-cols-3">
            <AcademicLink
              description="Define the campus academic cycles."
              label="Academic years"
              to="/modules/academics/academic-years"
            />
            <AcademicLink
              description="Set the dated teaching periods within each academic year."
              label="Academic terms"
              to="/modules/academics/terms"
            />
            <AcademicLink
              description="Maintain the grade references used across campus records."
              label="Grade levels"
              to="/modules/academics/grade-levels"
            />
            <AcademicLink
              description="Maintain the subjects taught across classes."
              label="Subjects"
              to="/modules/academics/subjects"
            />
            <AcademicLink
              description="Attach teacher profiles to HR employees."
              label="Teachers"
              to="/modules/academics/teachers"
            />
            <AcademicLink
              description="Organize classes within an academic year."
              label="Classes"
              to="/modules/academics/classes"
            />
            <AcademicLink
              description="Connect each class, subject, and teacher for timetabling."
              label="Teaching assignments"
              to="/modules/academics/teaching-assignments"
            />
            <AcademicLink
              description="Define term assessment cycles and weighted components."
              label="Assessments"
              to="/modules/academics/assessments"
            />
            <AcademicLink
              description="Capture, submit, and publish learner marks."
              label="Gradebook"
              to="/modules/academics/gradebook"
            />
            <AcademicLink
              description="Prepare report cards and published transcripts."
              label="Progress & reporting"
              to="/modules/academics/reporting"
            />
          </div>
        </section>
      </div>
    );
  }

  if (module.key === "finance") {
    return (
      <div className="space-y-8">
        <ModuleIntroduction module={module} />
        <section aria-labelledby="finance-workspaces">
          <p className="text-[11px] font-semibold uppercase tracking-[0.16em] text-[var(--brand-strong)]">
            Working areas
          </p>
          <h2
            className="mt-1 text-xl font-semibold tracking-[-0.025em] text-[var(--text-strong)]"
            id="finance-workspaces"
          >
            Set the accounting foundation
          </h2>
          <div className="mt-4 grid gap-4 md:grid-cols-2 xl:grid-cols-3">
            <FinanceLink
              description="Review operational requests before creating journal drafts."
              label="Posting requests"
              to="/modules/finance/posting-requests"
            />
            <FinanceLink
              description="Prepare, approve, post, and reverse balanced ledger entries."
              label="Journals"
              to="/modules/finance/journals"
            />
            <FinanceLink
              description="Set the reporting currency and currencies used in transactions."
              label="Currencies"
              to="/modules/finance/currencies"
            />
            <FinanceLink
              description="Maintain summary and posting accounts for the campus ledger."
              label="Chart of accounts"
              to="/modules/finance/chart-of-accounts"
            />
            <FinanceLink
              description="Open and close the dated periods used for journal posting."
              label="Fiscal years and periods"
              to="/modules/finance/accounting-periods"
            />
          </div>
        </section>
      </div>
    );
  }

  if (module.key === "attendance") {
    return (
      <div className="space-y-8">
        <ModuleIntroduction module={module} />
        <section aria-labelledby="attendance-workspaces">
          <p className="text-[11px] font-semibold uppercase tracking-[0.16em] text-[var(--brand-strong)]">
            Working areas
          </p>
          <h2
            className="mt-1 text-xl font-semibold tracking-[-0.025em] text-[var(--text-strong)]"
            id="attendance-workspaces"
          >
            Record daily attendance
          </h2>
          <div className="mt-4 grid gap-4 md:grid-cols-2 xl:grid-cols-3">
            <Link
              className="group border border-[var(--border)] bg-[var(--surface)] p-5 hover:border-[var(--border-strong)] hover:shadow-[var(--shadow-hover)]"
              to="/modules/attendance/registers"
            >
              <span className="flex items-center justify-between gap-4">
                <span className="font-semibold text-[var(--text-strong)]">
                  Registers
                </span>
                <ArrowRight className="size-4 text-[var(--text-subtle)] transition-transform group-hover:translate-x-1 group-hover:text-[var(--brand-strong)]" />
              </span>
              <span className="mt-2 block text-sm leading-5 text-[var(--text-muted)]">
                Create class registers, record learner marks, and submit
                completed attendance.
              </span>
            </Link>
          </div>
        </section>
      </div>
    );
  }

  if (module.key === "fees") {
    return (
      <div className="space-y-8">
        <ModuleIntroduction module={module} />
        <section aria-labelledby="fees-workspaces">
          <p className="text-[11px] font-semibold uppercase tracking-[0.16em] text-[var(--brand-strong)]">
            Working areas
          </p>
          <h2
            className="mt-1 text-xl font-semibold tracking-[-0.025em] text-[var(--text-strong)]"
            id="fees-workspaces"
          >
            Configure learner billing
          </h2>
          <div className="mt-4 grid gap-4 md:grid-cols-2 xl:grid-cols-3">
            <FeesLink
              description="Create learner invoices and track their Finance request state."
              label="Invoices"
              to="/modules/fees/invoices"
            />
            <FeesLink
              description="Open and maintain the Fees record linked to each learner."
              label="Billing accounts"
              to="/modules/fees/billing-accounts"
            />
            <FeesLink
              description="Define amounts, academic scope, currency, and Finance posting accounts."
              label="Fee structures"
              to="/modules/fees/fee-structures"
            />
            {permissions.includes("*") ||
            permissions.includes("fees:create") ? (
              <FeesLink
                description="Import existing learner billing accounts from CSV or XLSX files."
                label="Data imports"
                to="/modules/fees/imports"
              />
            ) : null}
          </div>
        </section>
      </div>
    );
  }

  if (module.key === "procurement") {
    return (
      <div className="space-y-8">
        <ModuleIntroduction module={module} />
        <section aria-labelledby="procurement-workspaces">
          <p className="text-[11px] font-semibold uppercase tracking-[0.16em] text-[var(--brand-strong)]">
            Working areas
          </p>
          <h2
            className="mt-1 text-xl font-semibold tracking-[-0.025em] text-[var(--text-strong)]"
            id="procurement-workspaces"
          >
            Manage purchasing
          </h2>
          <div className="mt-4 grid gap-4 md:grid-cols-2 xl:grid-cols-3">
            <ProcurementLink
              description="Prepare requests and record approval decisions before ordering."
              label="Requisitions"
              to="/modules/procurement/requisitions"
            />
            <ProcurementLink
              description="Issue approved requests to suppliers and track remaining quantities."
              label="Purchase orders"
              to="/modules/procurement/purchase-orders"
            />
            <ProcurementLink
              description="Record and post deliveries against issued purchase orders."
              label="Goods receipts"
              to="/modules/procurement/goods-receipts"
            />
            <ProcurementLink
              description="Maintain the suppliers available to purchasing requests."
              label="Suppliers"
              to="/modules/procurement/suppliers"
            />
          </div>
        </section>
      </div>
    );
  }

  if (module.key === "sis") {
    return (
      <div className="space-y-8">
        <ModuleIntroduction module={module} />
        <section aria-labelledby="sis-workspaces">
          <p className="text-[11px] font-semibold uppercase tracking-[0.16em] text-[var(--brand-strong)]">
            Working areas
          </p>
          <h2
            className="mt-1 text-xl font-semibold tracking-[-0.025em] text-[var(--text-strong)]"
            id="sis-workspaces"
          >
            Manage people and admissions
          </h2>
          <div className="mt-4 grid gap-4 md:grid-cols-2 xl:grid-cols-3">
            <SisLink
              description="Maintain learner records used across admissions and enrolment."
              label="Learners"
              to="/modules/sis/learners"
            />
            <SisLink
              description="Maintain the people responsible for learners."
              label="Guardians"
              to="/modules/sis/guardians"
            />
            <SisLink
              description="Connect learners to guardians and their responsibilities."
              label="Guardian relationships"
              to="/modules/sis/guardian-relationships"
            />
            <SisLink
              description="Track applications against an academic year and target grade."
              label="Applications"
              to="/modules/sis/applications"
            />
            <SisLink
              description="Place learners in an Academics class for an academic year."
              label="Enrolments"
              to="/modules/sis/enrolments"
            />
            <SisLink
              description="Map and validate existing learner or guardian records before importing them."
              label="Data imports"
              to="/modules/sis/imports"
            />
            <SisLink
              description="Set the prefix, padding, and next sequence for new learner numbers."
              label="Settings"
              to="/modules/sis/settings"
            />
          </div>
        </section>
      </div>
    );
  }

  if (module.key === "assets_inventory") {
    return (
      <div className="space-y-8">
        <ModuleIntroduction module={module} />
        <section aria-labelledby="assets-inventory-workspaces">
          <p className="text-[11px] font-semibold uppercase tracking-[0.16em] text-[var(--brand-strong)]">
            Working areas
          </p>
          <h2
            className="mt-1 text-xl font-semibold tracking-[-0.025em] text-[var(--text-strong)]"
            id="assets-inventory-workspaces"
          >
            Manage stock and records
          </h2>
          <div className="mt-4 grid gap-4 md:grid-cols-2 xl:grid-cols-3">
            <AssetsInventoryLink
              description="Review exact on-hand quantities by item and store."
              label="Stock"
              to="/modules/assets-inventory/stock"
            />
            {permissions.includes("*") ||
            permissions.includes("assets_inventory:view") ? (
              <AssetsInventoryLink
                description="Request stock for a department and track approval and issue status."
                label="Requests"
                to="/modules/assets-inventory/requests"
              />
            ) : null}
            <AssetsInventoryLink
              description="Open the immutable register of posted stock changes."
              label="Movements"
              to="/modules/assets-inventory/movements"
            />
            {enabledModules.includes("procurement") &&
            (permissions.includes("*") ||
              permissions.includes("assets_inventory:receive")) ? (
              <AssetsInventoryLink
                description="Allocate posted Procurement receipts to inventory items and stores."
                label="Procurement receipts"
                to="/modules/assets-inventory/procurement-receipts"
              />
            ) : null}
            <AssetsInventoryLink
              description="Maintain item definitions, quantity precision, and reorder levels."
              label="Items"
              to="/modules/assets-inventory/items"
            />
            <AssetsInventoryLink
              description="Maintain the stores used to hold inventory."
              label="Stores"
              to="/modules/assets-inventory/stores"
            />
          </div>
        </section>
      </div>
    );
  }

  return (
    <div className="space-y-8">
      <ModuleIntroduction module={module} />
      <section aria-labelledby="module-scope">
        <div className="border-b border-[var(--border)] pb-3">
          <p className="text-[11px] font-semibold uppercase tracking-[0.16em] text-[var(--brand-strong)]">
            Module scope
          </p>
          <h2
            className="mt-1 text-xl font-semibold tracking-[-0.025em] text-[var(--text-strong)]"
            id="module-scope"
          >
            Planned areas
          </h2>
        </div>
        <div className="grid gap-x-8 md:grid-cols-2 xl:grid-cols-3">
          {visual.highlights.map((highlight, index) => (
            <div
              className="flex items-start gap-4 border-b border-[var(--border-subtle)] py-5"
              key={highlight}
            >
              <span className="font-tabular text-xs font-semibold text-[var(--text-subtle)]">
                {String(index + 1).padStart(2, "0")}
              </span>
              <span className="text-sm font-medium text-[var(--text-body)]">
                {highlight}
              </span>
            </div>
          ))}
        </div>
      </section>
      <section className="flex items-start gap-4 bg-[var(--surface-muted)] p-5">
        <CircleDashed className="mt-0.5 size-5 shrink-0 text-[var(--brand-strong)]" />
        <div>
          <h2 className="text-sm font-semibold text-[var(--text-strong)]">
            This workspace is not available yet
          </h2>
          <p className="mt-1 max-w-[55em] text-sm leading-6 text-[var(--text-muted)]">
            Return to All modules and choose an available workspace.
          </p>
        </div>
      </section>
    </div>
  );
};

const ModuleIntroduction: React.FC<{ module: ModuleDefinition }> = ({
  module,
}) => {
  const visual = moduleVisuals[module.key] ?? defaultModuleVisual;
  const Icon = visual.icon;
  return (
    <section className="relative overflow-hidden bg-[var(--sidebar)] px-6 py-8 text-[var(--sidebar-foreground)] sm:px-8 sm:py-10">
      <div
        aria-hidden="true"
        className="campus-grid-pattern absolute inset-0 opacity-40"
      />
      <div className="relative flex max-w-3xl items-start gap-4">
        <span className="flex size-11 shrink-0 items-center justify-center rounded-[10px] bg-[var(--brand-highlight)] text-[var(--sidebar-active-fg)]">
          <Icon className="size-5" />
        </span>
        <div>
          <div className="flex items-center gap-2 text-[11px] font-semibold uppercase tracking-[0.16em] text-[var(--brand-highlight)]">
            <CheckCircle2 className="size-3.5" />
            {stageLabel(module.stage)}
          </div>
          <h1 className="mt-3 text-2xl font-semibold tracking-[-0.035em] sm:text-3xl">
            {module.label}
          </h1>
          <p className="mt-3 max-w-2xl text-sm leading-6 text-[var(--sidebar-muted)]">
            {module.description}
          </p>
        </div>
      </div>
    </section>
  );
};

const FleetLink: React.FC<{
  description: string;
  label: string;
  to:
    | "/modules/fleet/vehicles"
    | "/modules/fleet/drivers"
    | "/modules/fleet/daily-log";
}> = ({ description, label, to }) => (
  <Link
    className="group border border-[var(--border)] bg-[var(--surface)] p-5 hover:border-[var(--border-strong)] hover:shadow-[var(--shadow-hover)]"
    to={to}
  >
    <span className="flex items-center justify-between gap-4">
      <span className="font-semibold text-[var(--text-strong)]">{label}</span>
      <ArrowRight className="size-4 text-[var(--text-subtle)] transition-transform group-hover:translate-x-1 group-hover:text-[var(--brand-strong)]" />
    </span>
    <span className="mt-2 block text-sm leading-5 text-[var(--text-muted)]">
      {description}
    </span>
  </Link>
);

const HrLink: React.FC<{
  description: string;
  label: string;
  to:
    | "/modules/hr-payroll/employees"
    | "/modules/hr-payroll/imports"
    | "/modules/hr-payroll/employment"
    | "/modules/hr-payroll/availability"
    | "/modules/hr-payroll/departments"
    | "/modules/hr-payroll/positions";
}> = ({ description, label, to }) => (
  <Link
    className="group border border-[var(--border)] bg-[var(--surface)] p-5 hover:border-[var(--border-strong)] hover:shadow-[var(--shadow-hover)]"
    to={to}
  >
    <span className="flex items-center justify-between gap-4">
      <span className="font-semibold text-[var(--text-strong)]">{label}</span>
      <ArrowRight className="size-4 text-[var(--text-subtle)] transition-transform group-hover:translate-x-1 group-hover:text-[var(--brand-strong)]" />
    </span>
    <span className="mt-2 block text-sm leading-5 text-[var(--text-muted)]">
      {description}
    </span>
  </Link>
);

const AcademicLink: React.FC<{
  description: string;
  label: string;
  to:
    | "/modules/academics/academic-years"
    | "/modules/academics/terms"
    | "/modules/academics/grade-levels"
    | "/modules/academics/subjects"
    | "/modules/academics/teachers"
    | "/modules/academics/classes"
    | "/modules/academics/teaching-assignments"
    | "/modules/academics/assessments"
    | "/modules/academics/gradebook"
    | "/modules/academics/reporting";
}> = ({ description, label, to }) => (
  <Link
    className="group border border-[var(--border)] bg-[var(--surface)] p-5 hover:border-[var(--border-strong)] hover:shadow-[var(--shadow-hover)]"
    to={to}
  >
    <span className="flex items-center justify-between gap-4">
      <span className="font-semibold text-[var(--text-strong)]">{label}</span>
      <ArrowRight className="size-4 text-[var(--text-subtle)] transition-transform group-hover:translate-x-1 group-hover:text-[var(--brand-strong)]" />
    </span>
    <span className="mt-2 block text-sm leading-5 text-[var(--text-muted)]">
      {description}
    </span>
  </Link>
);

const FinanceLink: React.FC<{
  description: string;
  label: string;
  to:
    | "/modules/finance/posting-requests"
    | "/modules/finance/journals"
    | "/modules/finance/currencies"
    | "/modules/finance/chart-of-accounts"
    | "/modules/finance/accounting-periods";
}> = ({ description, label, to }) => (
  <Link
    className="group border border-[var(--border)] bg-[var(--surface)] p-5 hover:border-[var(--border-strong)] hover:shadow-[var(--shadow-hover)]"
    to={to}
  >
    <span className="flex items-center justify-between gap-4">
      <span className="font-semibold text-[var(--text-strong)]">{label}</span>
      <ArrowRight className="size-4 text-[var(--text-subtle)] transition-transform group-hover:translate-x-1 group-hover:text-[var(--brand-strong)]" />
    </span>
    <span className="mt-2 block text-sm leading-5 text-[var(--text-muted)]">
      {description}
    </span>
  </Link>
);

const FeesLink: React.FC<{
  description: string;
  label: string;
  to:
    | "/modules/fees/invoices"
    | "/modules/fees/billing-accounts"
    | "/modules/fees/fee-structures"
    | "/modules/fees/imports";
}> = ({ description, label, to }) => (
  <Link
    className="group border border-[var(--border)] bg-[var(--surface)] p-5 hover:border-[var(--border-strong)] hover:shadow-[var(--shadow-hover)]"
    to={to}
  >
    <span className="flex items-center justify-between gap-4">
      <span className="font-semibold text-[var(--text-strong)]">{label}</span>
      <ArrowRight className="size-4 text-[var(--text-subtle)] transition-transform group-hover:translate-x-1 group-hover:text-[var(--brand-strong)]" />
    </span>
    <span className="mt-2 block text-sm leading-5 text-[var(--text-muted)]">
      {description}
    </span>
  </Link>
);

const ProcurementLink: React.FC<{
  description: string;
  label: string;
  to:
    | "/modules/procurement/requisitions"
    | "/modules/procurement/purchase-orders"
    | "/modules/procurement/goods-receipts"
    | "/modules/procurement/suppliers";
}> = ({ description, label, to }) => (
  <Link
    className="group border border-[var(--border)] bg-[var(--surface)] p-5 hover:border-[var(--border-strong)] hover:shadow-[var(--shadow-hover)]"
    to={to}
  >
    <span className="flex items-center justify-between gap-4">
      <span className="font-semibold text-[var(--text-strong)]">{label}</span>
      <ArrowRight className="size-4 text-[var(--text-subtle)] transition-transform group-hover:translate-x-1 group-hover:text-[var(--brand-strong)]" />
    </span>
    <span className="mt-2 block text-sm leading-5 text-[var(--text-muted)]">
      {description}
    </span>
  </Link>
);

const AssetsInventoryLink: React.FC<{
  description: string;
  label: string;
  to:
    | "/modules/assets-inventory/stock"
    | "/modules/assets-inventory/requests"
    | "/modules/assets-inventory/movements"
    | "/modules/assets-inventory/procurement-receipts"
    | "/modules/assets-inventory/items"
    | "/modules/assets-inventory/stores";
}> = ({ description, label, to }) => (
  <Link
    className="group border border-[var(--border)] bg-[var(--surface)] p-5 hover:border-[var(--border-strong)] hover:shadow-[var(--shadow-hover)]"
    to={to}
  >
    <span className="flex items-center justify-between gap-4">
      <span className="font-semibold text-[var(--text-strong)]">{label}</span>
      <ArrowRight className="size-4 text-[var(--text-subtle)] transition-transform group-hover:translate-x-1 group-hover:text-[var(--brand-strong)]" />
    </span>
    <span className="mt-2 block text-sm leading-5 text-[var(--text-muted)]">
      {description}
    </span>
  </Link>
);

const SisLink: React.FC<{
  description: string;
  label: string;
  to:
    | "/modules/sis/learners"
    | "/modules/sis/guardians"
    | "/modules/sis/guardian-relationships"
    | "/modules/sis/applications"
    | "/modules/sis/enrolments"
    | "/modules/sis/imports"
    | "/modules/sis/settings";
}> = ({ description, label, to }) => (
  <Link
    className="group border border-[var(--border)] bg-[var(--surface)] p-5 hover:border-[var(--border-strong)] hover:shadow-[var(--shadow-hover)]"
    to={to}
  >
    <span className="flex items-center justify-between gap-4">
      <span className="font-semibold text-[var(--text-strong)]">{label}</span>
      <ArrowRight className="size-4 text-[var(--text-subtle)] transition-transform group-hover:translate-x-1 group-hover:text-[var(--brand-strong)]" />
    </span>
    <span className="mt-2 block text-sm leading-5 text-[var(--text-muted)]">
      {description}
    </span>
  </Link>
);

const UnknownModule = () => {
  usePageChrome("Module not found");
  return (
    <div className="py-16 text-center">
      <CircleDashed className="mx-auto size-9 text-[var(--text-subtle)]" />
      <h1 className="mt-4 text-xl font-semibold text-[var(--text-strong)]">
        This module is not in the Campus Pilot catalog
      </h1>
      <p className="mt-2 text-sm text-[var(--text-muted)]">
        Return to All modules and choose an available workspace.
      </p>
      <Link
        className="mt-5 inline-flex items-center gap-2 text-sm font-semibold text-[var(--brand-strong)]"
        to="/home"
      >
        All modules <ArrowRight className="size-4" />
      </Link>
    </div>
  );
};

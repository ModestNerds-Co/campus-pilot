import React, { useEffect, useState } from "react";
import { Link, useLocation, useNavigate } from "@tanstack/react-router";
import {
  ChevronLeft,
  ChevronRight,
  ArrowLeftRight,
  BookOpen,
  BookOpenCheck,
  Boxes,
  BriefcaseBusiness,
  Building2,
  CalendarClock,
  CalendarDays,
  CalendarRange,
  CalendarCheck2,
  BarChart3,
  BedDouble,
  ClipboardList,
  ArchiveRestore,
  FileArchive,
  Coins,
  CircleDollarSign,
  DoorOpen,
  GraduationCap,
  HeartHandshake,
  FileUp,
  FileCheck2,
  FileInput,
  LayoutDashboard,
  ListOrdered,
  Landmark,
  Inbox,
  LogOut,
  Menu,
  MessageSquareText,
  PackageSearch,
  PackageCheck,
  Pill,
  ReceiptText,
  RadioTower,
  School,
  Settings2,
  ShieldAlert,
  Clock3,
  ShoppingCart,
  Truck,
  Trophy,
  UserRoundCheck,
  UsersRound,
  Warehouse,
  Wrench,
  MapPin,
  X,
} from "lucide-react";
import toast from "react-hot-toast";

import { useNavigationDrawer } from "@/hooks/use-navigation-drawer";
import { ThemeToggle } from "@/lib/theme";
import { bootstrapService } from "@/modules/configs";
import type { SchoolConfiguration } from "@/modules/configs/types";
import { AgentWidget } from "@/modules/agent";
import { ACADEMIC_ADMINISTRATION_PERMISSIONS } from "@/modules/academics/access";
import { HR_ADMINISTRATION_PERMISSIONS } from "@/modules/hr-payroll/access";
import { libraryAccessProfile } from "@/modules/library/access";
import { hostelAccessProfile } from "@/modules/hostel/access";
import {
  SIS_ADMINISTRATION_PERMISSIONS,
  SIS_IMPORT_ACCESS_PERMISSIONS,
} from "@/modules/sis/access";
import { useAuthStore } from "@/stores/auth-store";

import {
  defaultModuleVisual,
  moduleRouteKey,
  moduleVisuals,
} from "./module-registry";
import {
  PageChromeProvider,
  usePageChromeContext,
} from "@/modules/admin/layouts/page-chrome";

interface ModuleLayoutProps {
  children: React.ReactNode;
}

type LocalNavItem = {
  label: string;
  path: string;
  icon: React.ComponentType<{ className?: string }>;
  permission?: string;
  anyPermissions?: string[];
  requiredModule?: string;
};

const moduleLabels: Record<string, string> = {
  agent: "Agent",
  sis: "People and admissions",
  academics: "Academics",
  attendance: "Attendance",
  activities: "Activities",
  learning: "E-learning",
  student_support: "Student support",
  timetabling: "Timetabling",
  messaging: "Communication",
  finance: "Finance",
  fees: "Fees and billing",
  library: "Library",
  hr_payroll: "HR and payroll",
  procurement: "Procurement",
  fleet: "Fleet",
  facilities: "Facilities",
  transport: "Transport",
  hostel: "Hostel",
  health: "Health services",
  assets_inventory: "Assets and inventory",
  document_registry: "Document registry",
  internal_audit: "Internal audit",
};

const fleetNavigation: LocalNavItem[] = [
  { label: "Vehicles", path: "/modules/fleet/vehicles", icon: Truck },
  { label: "Drivers", path: "/modules/fleet/drivers", icon: ClipboardList },
  {
    label: "Daily vehicle log",
    path: "/modules/fleet/daily-log",
    icon: ReceiptText,
  },
];

const facilitiesNavigation: LocalNavItem[] = [
  { label: "Work orders", path: "/modules/facilities/work-orders", icon: Wrench, permission: "facilities:operate" },
  { label: "Locations", path: "/modules/facilities/locations", icon: MapPin, permission: "facilities:manage" },
];

const activitiesNavigation: LocalNavItem[] = [
  { label: "Groups", path: "/modules/activities/groups", icon: UsersRound, permission: "activities:view" },
  { label: "Sessions", path: "/modules/activities/sessions", icon: CalendarDays, permission: "activities:view" },
  { label: "Catalog", path: "/modules/activities/catalog", icon: Trophy, permission: "activities:manage" },
];

const transportNavigation: LocalNavItem[] = [
  { label: "Routes", path: "/modules/transport/routes", icon: ListOrdered },
  { label: "Riders", path: "/modules/transport/riders", icon: UsersRound },
  { label: "Runs", path: "/modules/transport/runs", icon: Truck },
];

const hrNavigation: LocalNavItem[] = [
  {
    label: "Employees",
    path: "/modules/hr-payroll/employees",
    icon: UsersRound,
  },
  {
    label: "Employee imports",
    path: "/modules/hr-payroll/imports",
    icon: FileUp,
    anyPermissions: [...HR_ADMINISTRATION_PERMISSIONS],
  },
  {
    label: "Employment",
    path: "/modules/hr-payroll/employment",
    icon: BriefcaseBusiness,
  },
  {
    label: "Availability",
    path: "/modules/hr-payroll/availability",
    icon: CalendarClock,
  },
  {
    label: "Departments",
    path: "/modules/hr-payroll/departments",
    icon: Building2,
    anyPermissions: [...HR_ADMINISTRATION_PERMISSIONS],
  },
  {
    label: "Positions",
    path: "/modules/hr-payroll/positions",
    icon: BriefcaseBusiness,
    anyPermissions: [...HR_ADMINISTRATION_PERMISSIONS],
  },
];

const academicsNavigation: LocalNavItem[] = [
  {
    label: "Academic years",
    path: "/modules/academics/academic-years",
    icon: CalendarRange,
    anyPermissions: [...ACADEMIC_ADMINISTRATION_PERMISSIONS],
  },
  {
    label: "Academic terms",
    path: "/modules/academics/terms",
    icon: CalendarDays,
    anyPermissions: [...ACADEMIC_ADMINISTRATION_PERMISSIONS],
  },
  {
    label: "Grade levels",
    path: "/modules/academics/grade-levels",
    icon: ListOrdered,
    anyPermissions: [...ACADEMIC_ADMINISTRATION_PERMISSIONS],
  },
  { label: "Subjects", path: "/modules/academics/subjects", icon: BookOpen, anyPermissions: [...ACADEMIC_ADMINISTRATION_PERMISSIONS] },
  {
    label: "Teachers",
    path: "/modules/academics/teachers",
    icon: UserRoundCheck,
    anyPermissions: [...ACADEMIC_ADMINISTRATION_PERMISSIONS],
  },
  { label: "Classes", path: "/modules/academics/classes", icon: GraduationCap, anyPermissions: [...ACADEMIC_ADMINISTRATION_PERMISSIONS] },
  {
    label: "Teaching assignments",
    path: "/modules/academics/teaching-assignments",
    icon: ClipboardList,
    anyPermissions: [...ACADEMIC_ADMINISTRATION_PERMISSIONS],
  },
  {
    label: "Assessments",
    path: "/modules/academics/assessments",
    icon: FileCheck2,
    anyPermissions: [...ACADEMIC_ADMINISTRATION_PERMISSIONS],
  },
  {
    label: "Gradebook",
    path: "/modules/academics/gradebook",
    icon: BookOpenCheck,
    anyPermissions: ["academics:teach", "academics:manage"],
  },
  { label: "Reports", path: "/modules/academics/reporting", icon: BarChart3 },
];

const attendanceNavigation: LocalNavItem[] = [
  {
    label: "Registers",
    path: "/modules/attendance/registers",
    icon: CalendarCheck2,
  },
];

const learningNavigation: LocalNavItem[] = [
  { label: "Spaces", path: "/modules/learning/spaces", icon: BookOpenCheck },
  { label: "Settings", path: "/modules/learning/settings", icon: Settings2, permission: "learning:manage" },
];

function libraryNavigation(permissions: readonly string[]): LocalNavItem[] {
  const access = libraryAccessProfile(permissions);
  return [
    {
      label: access.canCirculate ? "Circulation" : "My loans",
      path: "/modules/library/circulation",
      icon: BookOpenCheck,
    },
    {
      label: access.canCirculate ? "Reservations" : "My holds",
      path: "/modules/library/holds",
      icon: Clock3,
    },
    {
      label: access.canManage ? "Members" : "My membership",
      path: "/modules/library/members",
      icon: UsersRound,
    },
    {
      label: access.canManage ? "Fines" : "My fines",
      path: "/modules/library/fines",
      icon: CircleDollarSign,
    },
    {
      label: "Settings",
      path: "/modules/library/settings",
      icon: Settings2,
      permission: "library:manage",
    },
  ];
}

const healthNavigation: LocalNavItem[] = [
  { label: "Clinic visits", path: "/modules/health/visits", icon: ClipboardList },
  { label: "Medication", path: "/modules/health/medication", icon: Pill },
  { label: "Follow-ups", path: "/modules/health/follow-ups", icon: CalendarCheck2 },
];

function hostelNavigation(
  permissions: readonly string[],
  recordScopes: Record<string, string> | undefined,
): LocalNavItem[] {
  const access = hostelAccessProfile(permissions, recordScopes);
  if (!access.hasCampusOccupancy) {
    return [];
  }
  return [
    { label: "Rooms & occupancy", path: "/modules/hostel/rooms", icon: DoorOpen },
    { label: "Allocations", path: "/modules/hostel/allocations", icon: ClipboardList },
    { label: "Pastoral records", path: "/modules/hostel/pastoral", icon: HeartHandshake, permission: "hostel:pastoral" },
  ];
}

const documentRegistryNavigation: LocalNavItem[] = [
  { label: "Classifications", path: "/modules/document-registry/classifications", icon: ArchiveRestore },
  { label: "Retention", path: "/modules/document-registry/retention", icon: Clock3, permission: "document_registry:dispose" },
  { label: "Disposition reviews", path: "/modules/document-registry/reviews", icon: FileArchive, permission: "document_registry:dispose" },
  { label: "Settings", path: "/modules/document-registry/settings", icon: Settings2 },
];

const internalAuditNavigation: LocalNavItem[] = [
  { label: "Audit plans", path: "/modules/internal-audit/plans", icon: ClipboardList },
  { label: "Findings", path: "/modules/internal-audit/findings", icon: ShieldAlert },
  { label: "Settings", path: "/modules/internal-audit/settings", icon: Settings2, permission: "internal_audit:manage" },
];

const messagingNavigation: LocalNavItem[] = [
  {
    label: "Inbox",
    path: "/modules/messaging/inbox",
    icon: Inbox,
    permission: "messaging:view",
  },
  {
    label: "Delivery history",
    path: "/modules/messaging/delivery-history",
    icon: RadioTower,
    permission: "messaging:send",
  },
];

const sisNavigation: LocalNavItem[] = [
  { label: "Learners", path: "/modules/sis/learners", icon: GraduationCap },
  { label: "Guardians", path: "/modules/sis/guardians", icon: UsersRound },
  {
    label: "Guardian relationships",
    path: "/modules/sis/guardian-relationships",
    icon: UserRoundCheck,
  },
  {
    label: "Applications",
    path: "/modules/sis/applications",
    icon: ClipboardList,
    anyPermissions: [...SIS_ADMINISTRATION_PERMISSIONS],
  },
  { label: "Enrolments", path: "/modules/sis/enrolments", icon: School },
  { label: "Data imports", path: "/modules/sis/imports", icon: FileUp, anyPermissions: [...SIS_IMPORT_ACCESS_PERMISSIONS] },
  { label: "Settings", path: "/modules/sis/settings", icon: Settings2, permission: "sis:edit" },
];

const financeNavigation: LocalNavItem[] = [
  {
    label: "Posting requests",
    path: "/modules/finance/posting-requests",
    icon: FileInput,
  },
  { label: "Journals", path: "/modules/finance/journals", icon: FileCheck2 },
  { label: "Currencies", path: "/modules/finance/currencies", icon: Coins },
  {
    label: "Chart of accounts",
    path: "/modules/finance/chart-of-accounts",
    icon: Landmark,
  },
  {
    label: "Fiscal years and periods",
    path: "/modules/finance/accounting-periods",
    icon: CalendarRange,
  },
];

const feesNavigation: LocalNavItem[] = [
  { label: "Invoices", path: "/modules/fees/invoices", icon: FileCheck2 },
  {
    label: "Billing accounts",
    path: "/modules/fees/billing-accounts",
    icon: UsersRound,
  },
  {
    label: "Fee structures",
    path: "/modules/fees/fee-structures",
    icon: ReceiptText,
  },
  {
    label: "Data imports",
    path: "/modules/fees/imports",
    icon: FileUp,
    permission: "fees:create",
  },
];

const procurementNavigation: LocalNavItem[] = [
  {
    label: "Requisitions",
    path: "/modules/procurement/requisitions",
    icon: ClipboardList,
  },
  {
    label: "Purchase orders",
    path: "/modules/procurement/purchase-orders",
    icon: ShoppingCart,
  },
  {
    label: "Goods receipts",
    path: "/modules/procurement/goods-receipts",
    icon: PackageCheck,
  },
  {
    label: "Suppliers",
    path: "/modules/procurement/suppliers",
    icon: PackageSearch,
  },
];

const assetsInventoryNavigation: LocalNavItem[] = [
  {
    label: "Stock",
    path: "/modules/assets-inventory/stock",
    icon: Boxes,
    permission: "assets_inventory:view",
  },
  {
    label: "Requests",
    path: "/modules/assets-inventory/requests",
    icon: ClipboardList,
    permission: "assets_inventory:view",
  },
  {
    label: "Movements",
    path: "/modules/assets-inventory/movements",
    icon: ArrowLeftRight,
    permission: "assets_inventory:view",
  },
  {
    label: "Procurement receipts",
    path: "/modules/assets-inventory/procurement-receipts",
    icon: PackageCheck,
    permission: "assets_inventory:receive",
    requiredModule: "procurement",
  },
  { label: "Items", path: "/modules/assets-inventory/items", icon: Boxes },
  {
    label: "Stores",
    path: "/modules/assets-inventory/stores",
    icon: Warehouse,
  },
];

const agentNavigation: LocalNavItem[] = [
  {
    label: "Personal usage",
    path: "/modules/agent/usage",
    icon: BarChart3,
    permission: "agent:view",
  },
];

export const ModuleLayout: React.FC<ModuleLayoutProps> = ({ children }) => (
  <PageChromeProvider>
    <ModuleLayoutShell>{children}</ModuleLayoutShell>
  </PageChromeProvider>
);

const ModuleLayoutShell: React.FC<ModuleLayoutProps> = ({ children }) => {
  const { title: pageTitle, action: pageAction } = usePageChromeContext();
  const location = useLocation();
  const navigate = useNavigate();
  const { user, logout } = useAuthStore();
  const [sidebarOpen, setSidebarOpen] = useState(false);
  const [school, setSchool] = useState<SchoolConfiguration | null>(null);
  const {
    desktopNavigation,
    navigationRef: sidebarRef,
    triggerRef: menuButtonRef,
  } = useNavigationDrawer(sidebarOpen, setSidebarOpen);
  const moduleKey = moduleKeyFromPath(location.pathname);
  const hostelAccess = hostelAccessProfile(
    user?.permissions ?? [],
    user?.record_scopes,
  );
  const hostelSelfService = moduleKey === "hostel" && !hostelAccess.hasCampusOccupancy;
  const hasSisAdministrationAccess =
    user?.permissions.includes("*") ||
    SIS_ADMINISTRATION_PERMISSIONS.some((permission) => user?.permissions.includes(permission));
  const moduleLabel = moduleKey === "sis" && !hasSisAdministrationAccess
    ? "Learners"
    : moduleLabels[moduleKey] || "Module workspace";
  const visual = moduleVisuals[moduleKey] ?? defaultModuleVisual;
  const ModuleIcon = visual.icon;
  const localNavigation = (
    moduleKey === "agent"
      ? agentNavigation
      : moduleKey === "fleet"
        ? fleetNavigation
        : moduleKey === "facilities"
          ? facilitiesNavigation
        : moduleKey === "activities"
          ? activitiesNavigation
        : moduleKey === "transport"
          ? transportNavigation
        : moduleKey === "hr_payroll"
          ? hrNavigation
          : moduleKey === "academics"
            ? academicsNavigation
            : moduleKey === "attendance"
              ? attendanceNavigation
              : moduleKey === "learning"
                ? learningNavigation
              : moduleKey === "student_support"
                ? []
              : moduleKey === "library"
                ? libraryNavigation(user?.permissions ?? [])
                : moduleKey === "health"
                  ? healthNavigation
                  : moduleKey === "hostel"
                    ? hostelNavigation(user?.permissions ?? [], user?.record_scopes)
                    : moduleKey === "document_registry"
                      ? documentRegistryNavigation
                    : moduleKey === "internal_audit"
                      ? internalAuditNavigation
                    : moduleKey === "messaging"
                    ? messagingNavigation
                    : moduleKey === "sis"
                      ? sisNavigation
                      : moduleKey === "finance"
                        ? financeNavigation
                        : moduleKey === "fees"
                          ? feesNavigation
                          : moduleKey === "procurement"
                            ? procurementNavigation
                            : moduleKey === "assets_inventory"
                              ? assetsInventoryNavigation
                              : []
  ).filter(
    (item) =>
      (!item.permission ||
        user?.permissions.includes("*") ||
        user?.permissions.includes(item.permission)) &&
      (!item.anyPermissions ||
        user?.permissions.includes("*") ||
        item.anyPermissions.some((permission) => user?.permissions.includes(permission))) &&
      (!item.requiredModule || user?.modules.includes(item.requiredModule)),
  );

  useEffect(() => {
    let active = true;
    void bootstrapService.checkStatus().then((response) => {
      if (active && response.success && response.data?.school)
        setSchool(response.data.school);
    });
    return () => {
      active = false;
    };
  }, []);

  useEffect(() => setSidebarOpen(false), [location.pathname]);

  const handleLogout = async () => {
    await logout();
    toast.success("Signed out");
    navigate({ to: "/login", replace: true });
  };

  const userName = user?.full_name || "Campus user";
  const userRole = user?.role_names?.[0] || "Campus access";

  return (
    <div className="min-h-[100dvh] bg-[var(--canvas)]">
      <a className="cp-skip-link" href="#main-content">
        Skip to main content
      </a>
      <aside
        aria-label={`${moduleLabel} navigation`}
        aria-hidden={!desktopNavigation && !sidebarOpen}
        className={`fixed inset-y-0 left-0 z-[70] flex w-[min(320px,calc(100vw-48px))] flex-col bg-[var(--sidebar)] text-[var(--sidebar-foreground)] transition-transform duration-300 ease-[var(--motion-ease-default)] lg:z-[var(--z-sidebar)] lg:w-[var(--sidebar-w)] lg:translate-x-0 ${sidebarOpen ? "translate-x-0" : "-translate-x-full"}`}
        id="module-navigation"
        ref={sidebarRef}
      >
        <div className="relative border-b border-[var(--sidebar-border)] px-5 pb-5 pt-6">
          <div
            aria-hidden="true"
            className="campus-grid-pattern absolute inset-0 opacity-40"
          />
          <div className="relative flex items-center gap-3">
            <span className="flex size-10 shrink-0 items-center justify-center rounded-[10px] bg-[var(--brand-highlight)] text-[var(--sidebar-active-fg)]">
              <ModuleIcon className="size-[18px]" />
            </span>
            <div className="min-w-0">
              <p className="truncate text-[15px] font-bold tracking-[-0.025em]">
                {moduleLabel}
              </p>
              <p className="truncate text-[11px] font-medium text-[var(--sidebar-muted)]">
                {school?.name || "Campus workspace"}
              </p>
            </div>
            <button
              aria-label="Close navigation"
              className="ml-auto inline-flex size-10 items-center justify-center rounded-[8px] border border-[var(--sidebar-border)] bg-white/5 text-[var(--sidebar-foreground)] hover:bg-[var(--sidebar-hover)] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--brand-highlight)] lg:hidden"
              onClick={() => setSidebarOpen(false)}
              type="button"
            >
              <X className="size-5" />
            </button>
          </div>
          <Link
            className="relative mt-5 flex min-h-10 items-center gap-2 rounded-[8px] border border-[var(--sidebar-border)] bg-white/5 px-3 text-[13px] font-medium text-[var(--sidebar-muted)] hover:bg-[var(--sidebar-hover)] hover:text-[var(--sidebar-foreground)]"
            to="/home"
          >
            <ChevronLeft className="size-4" />
            All modules
          </Link>
        </div>

        <nav
          className="cp-sidebar-scroll min-h-0 flex-1 overflow-y-auto px-3 py-4"
          aria-label="Module navigation"
        >
          <section aria-labelledby="module-workspace-nav">
            <h2
              className="mb-2 px-3 text-[10px] font-semibold uppercase tracking-[0.18em] text-[var(--sidebar-muted)]"
              id="module-workspace-nav"
            >
              Workspace
            </h2>
            <div className="space-y-1">
              <LocalOverviewLink
                active={
                  isModuleOverview(location.pathname) ||
                  (moduleKey === "agent" &&
                    location.pathname.startsWith("/modules/agent/sessions/")) ||
                  (moduleKey === "health" &&
                    location.pathname.startsWith("/modules/health/patients")) ||
                  (moduleKey === "facilities" &&
                    location.pathname.startsWith("/modules/facilities/requests")) ||
                  (hostelSelfService &&
                    location.pathname.startsWith("/modules/hostel/allocations"))
                }
                hostelSelfService={hostelSelfService}
                moduleKey={moduleKey}
              />
              {localNavigation.map((item) => (
                <LocalLink
                  active={
                    location.pathname === item.path ||
                    location.pathname.startsWith(`${item.path}/`)
                  }
                  item={item}
                  key={item.path}
                />
              ))}
            </div>
          </section>
        </nav>

        <div className="border-t border-[var(--sidebar-border)] p-3">
          {moduleKey !== "agent" ? (
            <AgentWidget
              context={{
                label: moduleLabel,
                moduleKey,
                route: location.pathname,
              }}
            />
          ) : null}
          <ThemeToggle className="w-full" variant="sidebar" />
          <div className="mt-3 flex items-center gap-3 px-2">
            <span className="flex size-9 items-center justify-center rounded-full border border-[var(--sidebar-border)] bg-white/10 text-xs font-semibold">
              {initials(userName)}
            </span>
            <div className="min-w-0 flex-1">
              <p className="truncate text-[13px] font-semibold">{userName}</p>
              <p className="truncate text-[11px] text-[var(--sidebar-muted)]">
                {userRole}
              </p>
            </div>
          </div>
          <button
            className="mt-2 flex min-h-10 w-full items-center gap-3 rounded-[8px] px-3 text-[13px] font-medium text-[var(--sidebar-muted)] hover:bg-[var(--sidebar-hover)] hover:text-[var(--sidebar-foreground)] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--brand-highlight)]"
            onClick={() => void handleLogout()}
            type="button"
          >
            <LogOut className="size-[17px]" /> Sign out
          </button>
        </div>
      </aside>

      {sidebarOpen ? (
        <button
          aria-label="Close navigation"
          className="fixed inset-0 z-[65] bg-[var(--surface-overlay)] lg:hidden"
          onClick={() => setSidebarOpen(false)}
          type="button"
        />
      ) : null}

      <div className="min-w-0 lg:pl-[var(--sidebar-w)]">
        <header className="fixed inset-x-0 top-0 z-[var(--z-nav)] flex h-[var(--app-bar-h)] items-center justify-between border-b border-[var(--border)] bg-[var(--surface)]/95 px-4 backdrop-blur-md lg:left-[var(--sidebar-w)] lg:px-8">
          <div className="flex min-w-0 items-center gap-3">
            <button
              aria-controls="module-navigation"
              aria-expanded={sidebarOpen}
              aria-hidden={sidebarOpen}
              aria-label="Open navigation"
              className={`inline-flex size-10 shrink-0 items-center justify-center rounded-[8px] border border-[var(--border)] bg-[var(--surface)] text-[var(--text-body)] hover:bg-[var(--surface-muted)] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--focus-ring)] lg:hidden ${sidebarOpen ? "invisible pointer-events-none" : ""}`}
              onClick={() => setSidebarOpen(true)}
              ref={menuButtonRef}
              tabIndex={sidebarOpen ? -1 : 0}
              type="button"
            >
              <Menu className="size-5" />
            </button>
            <div className="min-w-0">
              <p className="truncate text-[14px] font-semibold text-[var(--text-strong)]">
                {pageTitle || moduleLabel}
              </p>
              <p className="hidden truncate text-[12px] text-[var(--text-muted)] sm:block">
                {school?.name || "Campus workspace"}
              </p>
            </div>
          </div>
          <div className="flex items-center gap-3">
            {pageAction ? (
              <div className="hidden sm:block">{pageAction}</div>
            ) : null}
            <ThemeToggle className="lg:hidden" />
          </div>
        </header>
        <main
          className="min-h-[100dvh] pt-[var(--app-bar-h)]"
          id="main-content"
          tabIndex={-1}
        >
          <div className="campus-page-enter mx-auto max-w-[1480px] p-4 sm:p-6 lg:p-8">
            {pageAction ? (
              <div className="mb-4 sm:hidden">{pageAction}</div>
            ) : null}
            {children}
          </div>
        </main>
      </div>
    </div>
  );
};

const LocalOverviewLink: React.FC<{ active: boolean; hostelSelfService: boolean; moduleKey: string }> = ({
  active,
  hostelSelfService,
  moduleKey,
}) => {
  const OverviewIcon = hostelSelfService ? BedDouble : LayoutDashboard;
  return (
  <Link
    aria-current={active ? "page" : undefined}
    className={navClass(active)}
    params={{ moduleKey: moduleRouteKey(moduleKey) }}
    to="/modules/$moduleKey"
  >
    <OverviewIcon className="size-[17px]" />
    <span className="flex-1">
      {moduleKey === "agent"
        ? "Sessions"
        : moduleKey === "messaging"
          ? "Announcements"
          : moduleKey === "library"
            ? "Catalogue"
            : moduleKey === "health"
              ? "Patients"
              : moduleKey === "hostel"
                ? hostelSelfService ? "My stay" : "Residences"
              : moduleKey === "document_registry"
                ? "Documents"
              : moduleKey === "internal_audit"
                ? "Engagements"
              : moduleKey === "learning"
                ? "Spaces"
              : moduleKey === "student_support"
                ? "Cases"
              : moduleKey === "transport"
                ? "Routes"
              : moduleKey === "facilities"
                ? "Service requests"
            : "Overview"}
    </span>
    {active ? <ChevronRight className="size-3.5" /> : null}
  </Link>
  );
};

const LocalLink: React.FC<{ active: boolean; item: LocalNavItem }> = ({
  active,
  item,
}) => {
  const Icon = item.icon;
  if (item.path === "/modules/agent/usage")
    return (
      <Link className={navClass(active)} to="/modules/agent/usage">
        <Icon className="size-[17px]" />
        <span className="flex-1">{item.label}</span>
        {active ? <ChevronRight className="size-3.5" /> : null}
      </Link>
    );
  if (item.path === "/modules/fleet/vehicles")
    return (
      <Link className={navClass(active)} to="/modules/fleet/vehicles">
        <Icon className="size-[17px]" />
        <span className="flex-1">{item.label}</span>
        {active ? <ChevronRight className="size-3.5" /> : null}
      </Link>
    );
  if (item.path === "/modules/fleet/drivers")
    return (
      <Link className={navClass(active)} to="/modules/fleet/drivers">
        <Icon className="size-[17px]" />
        <span className="flex-1">{item.label}</span>
        {active ? <ChevronRight className="size-3.5" /> : null}
      </Link>
    );
  if (item.path === "/modules/fleet/daily-log")
    return (
      <Link className={navClass(active)} to="/modules/fleet/daily-log">
        <Icon className="size-[17px]" />
        <span className="flex-1">{item.label}</span>
        {active ? <ChevronRight className="size-3.5" /> : null}
      </Link>
    );
  if (item.path === "/modules/facilities/requests")
    return <Link className={navClass(active)} to="/modules/facilities/requests"><Icon className="size-[17px]" /><span className="flex-1">{item.label}</span>{active ? <ChevronRight className="size-3.5" /> : null}</Link>;
  if (item.path === "/modules/facilities/work-orders")
    return <Link className={navClass(active)} to="/modules/facilities/work-orders"><Icon className="size-[17px]" /><span className="flex-1">{item.label}</span>{active ? <ChevronRight className="size-3.5" /> : null}</Link>;
  if (item.path === "/modules/facilities/locations")
    return <Link className={navClass(active)} to="/modules/facilities/locations"><Icon className="size-[17px]" /><span className="flex-1">{item.label}</span>{active ? <ChevronRight className="size-3.5" /> : null}</Link>;
  if (item.path === "/modules/activities/groups")
    return <Link className={navClass(active)} to="/modules/activities/groups"><Icon className="size-[17px]" /><span className="flex-1">{item.label}</span>{active ? <ChevronRight className="size-3.5" /> : null}</Link>;
  if (item.path === "/modules/activities/sessions")
    return <Link className={navClass(active)} to="/modules/activities/sessions"><Icon className="size-[17px]" /><span className="flex-1">{item.label}</span>{active ? <ChevronRight className="size-3.5" /> : null}</Link>;
  if (item.path === "/modules/activities/catalog")
    return <Link className={navClass(active)} to="/modules/activities/catalog"><Icon className="size-[17px]" /><span className="flex-1">{item.label}</span>{active ? <ChevronRight className="size-3.5" /> : null}</Link>;
  if (item.path === "/modules/hr-payroll/employees")
    return (
      <Link className={navClass(active)} to="/modules/hr-payroll/employees">
        <Icon className="size-[17px]" />
        <span className="flex-1">{item.label}</span>
        {active ? <ChevronRight className="size-3.5" /> : null}
      </Link>
    );
  if (item.path === "/modules/hr-payroll/imports")
    return (
      <Link className={navClass(active)} to="/modules/hr-payroll/imports">
        <Icon className="size-[17px]" />
        <span className="flex-1">{item.label}</span>
        {active ? <ChevronRight className="size-3.5" /> : null}
      </Link>
    );
  if (item.path === "/modules/hr-payroll/employment")
    return (
      <Link className={navClass(active)} to="/modules/hr-payroll/employment">
        <Icon className="size-[17px]" />
        <span className="flex-1">{item.label}</span>
        {active ? <ChevronRight className="size-3.5" /> : null}
      </Link>
    );
  if (item.path === "/modules/hr-payroll/availability")
    return (
      <Link className={navClass(active)} to="/modules/hr-payroll/availability">
        <Icon className="size-[17px]" />
        <span className="flex-1">{item.label}</span>
        {active ? <ChevronRight className="size-3.5" /> : null}
      </Link>
    );
  if (item.path === "/modules/hr-payroll/departments")
    return (
      <Link className={navClass(active)} to="/modules/hr-payroll/departments">
        <Icon className="size-[17px]" />
        <span className="flex-1">{item.label}</span>
        {active ? <ChevronRight className="size-3.5" /> : null}
      </Link>
    );
  if (item.path === "/modules/hr-payroll/positions")
    return (
      <Link className={navClass(active)} to="/modules/hr-payroll/positions">
        <Icon className="size-[17px]" />
        <span className="flex-1">{item.label}</span>
        {active ? <ChevronRight className="size-3.5" /> : null}
      </Link>
    );
  if (item.path === "/modules/academics/academic-years")
    return (
      <Link className={navClass(active)} to="/modules/academics/academic-years">
        <Icon className="size-[17px]" />
        <span className="flex-1">{item.label}</span>
        {active ? <ChevronRight className="size-3.5" /> : null}
      </Link>
    );
  if (item.path === "/modules/academics/terms")
    return (
      <Link className={navClass(active)} to="/modules/academics/terms">
        <Icon className="size-[17px]" />
        <span className="flex-1">{item.label}</span>
        {active ? <ChevronRight className="size-3.5" /> : null}
      </Link>
    );
  if (item.path === "/modules/academics/grade-levels")
    return (
      <Link className={navClass(active)} to="/modules/academics/grade-levels">
        <Icon className="size-[17px]" />
        <span className="flex-1">{item.label}</span>
        {active ? <ChevronRight className="size-3.5" /> : null}
      </Link>
    );
  if (item.path === "/modules/academics/subjects")
    return (
      <Link className={navClass(active)} to="/modules/academics/subjects">
        <Icon className="size-[17px]" />
        <span className="flex-1">{item.label}</span>
        {active ? <ChevronRight className="size-3.5" /> : null}
      </Link>
    );
  if (item.path === "/modules/academics/teachers")
    return (
      <Link className={navClass(active)} to="/modules/academics/teachers">
        <Icon className="size-[17px]" />
        <span className="flex-1">{item.label}</span>
        {active ? <ChevronRight className="size-3.5" /> : null}
      </Link>
    );
  if (item.path === "/modules/academics/classes")
    return (
      <Link className={navClass(active)} to="/modules/academics/classes">
        <Icon className="size-[17px]" />
        <span className="flex-1">{item.label}</span>
        {active ? <ChevronRight className="size-3.5" /> : null}
      </Link>
    );
  if (item.path === "/modules/academics/teaching-assignments")
    return (
      <Link
        className={navClass(active)}
        to="/modules/academics/teaching-assignments"
      >
        <Icon className="size-[17px]" />
        <span className="flex-1">{item.label}</span>
        {active ? <ChevronRight className="size-3.5" /> : null}
      </Link>
    );
  if (item.path === "/modules/academics/assessments")
    return (
      <Link className={navClass(active)} to="/modules/academics/assessments">
        <Icon className="size-[17px]" />
        <span className="flex-1">{item.label}</span>
        {active ? <ChevronRight className="size-3.5" /> : null}
      </Link>
    );
  if (item.path === "/modules/academics/gradebook")
    return (
      <Link className={navClass(active)} to="/modules/academics/gradebook">
        <Icon className="size-[17px]" />
        <span className="flex-1">{item.label}</span>
        {active ? <ChevronRight className="size-3.5" /> : null}
      </Link>
    );
  if (item.path === "/modules/academics/reporting")
    return (
      <Link className={navClass(active)} to="/modules/academics/reporting">
        <Icon className="size-[17px]" />
        <span className="flex-1">{item.label}</span>
        {active ? <ChevronRight className="size-3.5" /> : null}
      </Link>
    );
  if (item.path === "/modules/finance/currencies")
    return (
      <Link className={navClass(active)} to="/modules/finance/currencies">
        <Icon className="size-[17px]" />
        <span className="flex-1">{item.label}</span>
        {active ? <ChevronRight className="size-3.5" /> : null}
      </Link>
    );
  if (item.path === "/modules/finance/chart-of-accounts")
    return (
      <Link
        className={navClass(active)}
        to="/modules/finance/chart-of-accounts"
      >
        <Icon className="size-[17px]" />
        <span className="flex-1">{item.label}</span>
        {active ? <ChevronRight className="size-3.5" /> : null}
      </Link>
    );
  if (item.path === "/modules/finance/accounting-periods")
    return (
      <Link
        className={navClass(active)}
        to="/modules/finance/accounting-periods"
      >
        <Icon className="size-[17px]" />
        <span className="flex-1">{item.label}</span>
        {active ? <ChevronRight className="size-3.5" /> : null}
      </Link>
    );
  if (item.path === "/modules/finance/journals")
    return (
      <Link className={navClass(active)} to="/modules/finance/journals">
        <Icon className="size-[17px]" />
        <span className="flex-1">{item.label}</span>
        {active ? <ChevronRight className="size-3.5" /> : null}
      </Link>
    );
  if (item.path === "/modules/finance/posting-requests")
    return (
      <Link className={navClass(active)} to="/modules/finance/posting-requests">
        <Icon className="size-[17px]" />
        <span className="flex-1">{item.label}</span>
        {active ? <ChevronRight className="size-3.5" /> : null}
      </Link>
    );
  if (item.path === "/modules/fees/billing-accounts")
    return (
      <Link className={navClass(active)} to="/modules/fees/billing-accounts">
        <Icon className="size-[17px]" />
        <span className="flex-1">{item.label}</span>
        {active ? <ChevronRight className="size-3.5" /> : null}
      </Link>
    );
  if (item.path === "/modules/fees/fee-structures")
    return (
      <Link className={navClass(active)} to="/modules/fees/fee-structures">
        <Icon className="size-[17px]" />
        <span className="flex-1">{item.label}</span>
        {active ? <ChevronRight className="size-3.5" /> : null}
      </Link>
    );
  if (item.path === "/modules/fees/invoices")
    return (
      <Link className={navClass(active)} to="/modules/fees/invoices">
        <Icon className="size-[17px]" />
        <span className="flex-1">{item.label}</span>
        {active ? <ChevronRight className="size-3.5" /> : null}
      </Link>
    );
  if (item.path === "/modules/fees/imports")
    return (
      <Link className={navClass(active)} to="/modules/fees/imports">
        <Icon className="size-[17px]" />
        <span className="flex-1">{item.label}</span>
        {active ? <ChevronRight className="size-3.5" /> : null}
      </Link>
    );
  if (item.path === "/modules/procurement/requisitions")
    return (
      <Link className={navClass(active)} to="/modules/procurement/requisitions">
        <Icon className="size-[17px]" />
        <span className="flex-1">{item.label}</span>
        {active ? <ChevronRight className="size-3.5" /> : null}
      </Link>
    );
  if (item.path === "/modules/procurement/purchase-orders")
    return (
      <Link
        className={navClass(active)}
        to="/modules/procurement/purchase-orders"
      >
        <Icon className="size-[17px]" />
        <span className="flex-1">{item.label}</span>
        {active ? <ChevronRight className="size-3.5" /> : null}
      </Link>
    );
  if (item.path === "/modules/procurement/goods-receipts")
    return (
      <Link
        className={navClass(active)}
        to="/modules/procurement/goods-receipts"
      >
        <Icon className="size-[17px]" />
        <span className="flex-1">{item.label}</span>
        {active ? <ChevronRight className="size-3.5" /> : null}
      </Link>
    );
  if (item.path === "/modules/procurement/suppliers")
    return (
      <Link className={navClass(active)} to="/modules/procurement/suppliers">
        <Icon className="size-[17px]" />
        <span className="flex-1">{item.label}</span>
        {active ? <ChevronRight className="size-3.5" /> : null}
      </Link>
    );
  if (item.path === "/modules/assets-inventory/stock")
    return (
      <Link className={navClass(active)} to="/modules/assets-inventory/stock">
        <Icon className="size-[17px]" />
        <span className="flex-1">{item.label}</span>
        {active ? <ChevronRight className="size-3.5" /> : null}
      </Link>
    );
  if (item.path === "/modules/assets-inventory/requests")
    return (
      <Link
        className={navClass(active)}
        to="/modules/assets-inventory/requests"
      >
        <Icon className="size-[17px]" />
        <span className="flex-1">{item.label}</span>
        {active ? <ChevronRight className="size-3.5" /> : null}
      </Link>
    );
  if (item.path === "/modules/assets-inventory/movements")
    return (
      <Link
        className={navClass(active)}
        to="/modules/assets-inventory/movements"
      >
        <Icon className="size-[17px]" />
        <span className="flex-1">{item.label}</span>
        {active ? <ChevronRight className="size-3.5" /> : null}
      </Link>
    );
  if (item.path === "/modules/assets-inventory/procurement-receipts")
    return (
      <Link
        className={navClass(active)}
        to="/modules/assets-inventory/procurement-receipts"
      >
        <Icon className="size-[17px]" />
        <span className="flex-1">{item.label}</span>
        {active ? <ChevronRight className="size-3.5" /> : null}
      </Link>
    );
  if (item.path === "/modules/assets-inventory/items")
    return (
      <Link className={navClass(active)} to="/modules/assets-inventory/items">
        <Icon className="size-[17px]" />
        <span className="flex-1">{item.label}</span>
        {active ? <ChevronRight className="size-3.5" /> : null}
      </Link>
    );
  if (item.path === "/modules/assets-inventory/stores")
    return (
      <Link className={navClass(active)} to="/modules/assets-inventory/stores">
        <Icon className="size-[17px]" />
        <span className="flex-1">{item.label}</span>
        {active ? <ChevronRight className="size-3.5" /> : null}
      </Link>
    );
  if (item.path === "/modules/attendance/registers")
    return (
      <Link className={navClass(active)} to="/modules/attendance/registers">
        <Icon className="size-[17px]" />
        <span className="flex-1">{item.label}</span>
        {active ? <ChevronRight className="size-3.5" /> : null}
      </Link>
    );
  if (item.path === "/modules/learning/spaces")
    return <Link className={navClass(active)} search={{ page: 1, q: "", status: "all" }} to="/modules/learning/spaces"><Icon className="size-[17px]"/><span className="flex-1">{item.label}</span>{active?<ChevronRight className="size-3.5"/>:null}</Link>;
  if (item.path === "/modules/student-support/cases")
    return <Link className={navClass(active)} to="/modules/student-support/cases"><Icon className="size-[17px]"/><span className="flex-1">{item.label}</span>{active?<ChevronRight className="size-3.5"/>:null}</Link>;
  if (item.path === "/modules/transport/routes")
    return <Link className={navClass(active)} to="/modules/transport/routes"><Icon className="size-[17px]"/><span className="flex-1">{item.label}</span>{active?<ChevronRight className="size-3.5"/>:null}</Link>;
  if (item.path === "/modules/transport/riders")
    return <Link className={navClass(active)} to="/modules/transport/riders"><Icon className="size-[17px]"/><span className="flex-1">{item.label}</span>{active?<ChevronRight className="size-3.5"/>:null}</Link>;
  if (item.path === "/modules/transport/runs")
    return <Link className={navClass(active)} to="/modules/transport/runs"><Icon className="size-[17px]"/><span className="flex-1">{item.label}</span>{active?<ChevronRight className="size-3.5"/>:null}</Link>;
  if (item.path === "/modules/learning/settings")
    return <Link className={navClass(active)} to="/modules/learning/settings"><Icon className="size-[17px]"/><span className="flex-1">{item.label}</span>{active?<ChevronRight className="size-3.5"/>:null}</Link>;
  if (item.path === "/modules/library/circulation")
    return (
      <Link className={navClass(active)} to="/modules/library/circulation">
        <Icon className="size-[17px]" />
        <span className="flex-1">{item.label}</span>
        {active ? <ChevronRight className="size-3.5" /> : null}
      </Link>
    );
  if (item.path === "/modules/library/holds")
    return (
      <Link className={navClass(active)} to="/modules/library/holds">
        <Icon className="size-[17px]" />
        <span className="flex-1">{item.label}</span>
        {active ? <ChevronRight className="size-3.5" /> : null}
      </Link>
    );
  if (item.path === "/modules/library/members")
    return (
      <Link className={navClass(active)} to="/modules/library/members">
        <Icon className="size-[17px]" />
        <span className="flex-1">{item.label}</span>
        {active ? <ChevronRight className="size-3.5" /> : null}
      </Link>
    );
  if (item.path === "/modules/library/fines")
    return (
      <Link className={navClass(active)} to="/modules/library/fines">
        <Icon className="size-[17px]" />
        <span className="flex-1">{item.label}</span>
        {active ? <ChevronRight className="size-3.5" /> : null}
      </Link>
    );
  if (item.path === "/modules/library/settings")
    return (
      <Link className={navClass(active)} to="/modules/library/settings">
        <Icon className="size-[17px]" />
        <span className="flex-1">{item.label}</span>
        {active ? <ChevronRight className="size-3.5" /> : null}
      </Link>
    );
  if (item.path === "/modules/health/visits")
    return (
      <Link className={navClass(active)} to="/modules/health/visits">
        <Icon className="size-[17px]" />
        <span className="flex-1">{item.label}</span>
        {active ? <ChevronRight className="size-3.5" /> : null}
      </Link>
    );
  if (item.path === "/modules/health/medication")
    return (
      <Link className={navClass(active)} to="/modules/health/medication">
        <Icon className="size-[17px]" />
        <span className="flex-1">{item.label}</span>
        {active ? <ChevronRight className="size-3.5" /> : null}
      </Link>
    );
  if (item.path === "/modules/health/follow-ups")
    return (
      <Link className={navClass(active)} to="/modules/health/follow-ups">
        <Icon className="size-[17px]" />
        <span className="flex-1">{item.label}</span>
        {active ? <ChevronRight className="size-3.5" /> : null}
      </Link>
    );
  if (item.path === "/modules/hostel/rooms")
    return (
      <Link className={navClass(active)} to="/modules/hostel/rooms">
        <Icon className="size-[17px]" />
        <span className="flex-1">{item.label}</span>
        {active ? <ChevronRight className="size-3.5" /> : null}
      </Link>
    );
  if (item.path === "/modules/hostel/allocations")
    return (
      <Link className={navClass(active)} to="/modules/hostel/allocations">
        <Icon className="size-[17px]" />
        <span className="flex-1">{item.label}</span>
        {active ? <ChevronRight className="size-3.5" /> : null}
      </Link>
    );
  if (item.path === "/modules/hostel/pastoral")
    return (
      <Link className={navClass(active)} to="/modules/hostel/pastoral">
        <Icon className="size-[17px]" />
        <span className="flex-1">{item.label}</span>
        {active ? <ChevronRight className="size-3.5" /> : null}
      </Link>
    );
  if (item.path === "/modules/document-registry/classifications")
    return <Link className={navClass(active)} to="/modules/document-registry/classifications"><Icon className="size-[17px]"/><span className="flex-1">{item.label}</span>{active?<ChevronRight className="size-3.5"/>:null}</Link>;
  if (item.path === "/modules/document-registry/retention")
    return <Link className={navClass(active)} to="/modules/document-registry/retention"><Icon className="size-[17px]"/><span className="flex-1">{item.label}</span>{active?<ChevronRight className="size-3.5"/>:null}</Link>;
  if (item.path === "/modules/document-registry/reviews")
    return <Link className={navClass(active)} to="/modules/document-registry/reviews"><Icon className="size-[17px]"/><span className="flex-1">{item.label}</span>{active?<ChevronRight className="size-3.5"/>:null}</Link>;
  if (item.path === "/modules/document-registry/settings")
    return <Link className={navClass(active)} to="/modules/document-registry/settings"><Icon className="size-[17px]"/><span className="flex-1">{item.label}</span>{active?<ChevronRight className="size-3.5"/>:null}</Link>;
  if (item.path === "/modules/internal-audit/plans")
    return <Link className={navClass(active)} to="/modules/internal-audit/plans"><Icon className="size-[17px]"/><span className="flex-1">{item.label}</span>{active?<ChevronRight className="size-3.5"/>:null}</Link>;
  if (item.path === "/modules/internal-audit/findings")
    return <Link className={navClass(active)} to="/modules/internal-audit/findings"><Icon className="size-[17px]"/><span className="flex-1">{item.label}</span>{active?<ChevronRight className="size-3.5"/>:null}</Link>;
  if (item.path === "/modules/internal-audit/settings")
    return <Link className={navClass(active)} to="/modules/internal-audit/settings"><Icon className="size-[17px]"/><span className="flex-1">{item.label}</span>{active?<ChevronRight className="size-3.5"/>:null}</Link>;
  if (item.path === "/modules/messaging/inbox")
    return (
      <Link className={navClass(active)} search={{ filter: "all", page: 1, q: "", status: "all" }} to="/modules/messaging/inbox">
        <Icon className="size-[17px]" />
        <span className="flex-1">{item.label}</span>
        {active ? <ChevronRight className="size-3.5" /> : null}
      </Link>
    );
  if (item.path === "/modules/messaging/delivery-history")
    return (
      <Link
        className={navClass(active)}
        search={{ filter: "all", page: 1, q: "", status: "all" }}
        to="/modules/messaging/delivery-history"
      >
        <Icon className="size-[17px]" />
        <span className="flex-1">{item.label}</span>
        {active ? <ChevronRight className="size-3.5" /> : null}
      </Link>
    );
  if (item.path === "/modules/sis/learners")
    return (
      <Link className={navClass(active)} to="/modules/sis/learners">
        <Icon className="size-[17px]" />
        <span className="flex-1">{item.label}</span>
        {active ? <ChevronRight className="size-3.5" /> : null}
      </Link>
    );
  if (item.path === "/modules/sis/guardians")
    return (
      <Link className={navClass(active)} to="/modules/sis/guardians">
        <Icon className="size-[17px]" />
        <span className="flex-1">{item.label}</span>
        {active ? <ChevronRight className="size-3.5" /> : null}
      </Link>
    );
  if (item.path === "/modules/sis/guardian-relationships")
    return (
      <Link
        className={navClass(active)}
        to="/modules/sis/guardian-relationships"
      >
        <Icon className="size-[17px]" />
        <span className="flex-1">{item.label}</span>
        {active ? <ChevronRight className="size-3.5" /> : null}
      </Link>
    );
  if (item.path === "/modules/sis/applications")
    return (
      <Link className={navClass(active)} to="/modules/sis/applications">
        <Icon className="size-[17px]" />
        <span className="flex-1">{item.label}</span>
        {active ? <ChevronRight className="size-3.5" /> : null}
      </Link>
    );
  if (item.path === "/modules/sis/imports")
    return (
      <Link className={navClass(active)} to="/modules/sis/imports">
        <Icon className="size-[17px]" />
        <span className="flex-1">{item.label}</span>
        {active ? <ChevronRight className="size-3.5" /> : null}
      </Link>
    );
  if (item.path === "/modules/sis/settings")
    return (
      <Link className={navClass(active)} to="/modules/sis/settings">
        <Icon className="size-[17px]" />
        <span className="flex-1">{item.label}</span>
        {active ? <ChevronRight className="size-3.5" /> : null}
      </Link>
    );
  return (
    <Link className={navClass(active)} to="/modules/sis/enrolments">
      <Icon className="size-[17px]" />
      <span className="flex-1">{item.label}</span>
      {active ? <ChevronRight className="size-3.5" /> : null}
    </Link>
  );
};

function navClass(active: boolean) {
  return `flex min-h-10 items-center gap-3 rounded-[8px] px-3 text-[13px] font-medium focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--brand-highlight)] ${active ? "bg-[var(--sidebar-active)] text-[var(--sidebar-active-fg)]" : "text-[var(--sidebar-muted)] hover:bg-[var(--sidebar-hover)] hover:text-[var(--sidebar-foreground)]"}`;
}

function moduleKeyFromPath(pathname: string) {
  const key = pathname.split("/")[2] || "";
  return key.replace(/-/g, "_");
}

function isModuleOverview(pathname: string) {
  return pathname.split("/").filter(Boolean).length === 2;
}

function initials(name: string) {
  return name
    .trim()
    .split(/\s+/)
    .slice(0, 2)
    .map((part) => part[0]?.toUpperCase() || "")
    .join("");
}

import React, { useEffect, useState } from "react";
import { Link, useLocation, useNavigate } from "@tanstack/react-router";
import {
  ChevronLeft,
  ChevronRight,
  BookOpen,
  BriefcaseBusiness,
  Building2,
  CalendarRange,
  ClipboardList,
  GraduationCap,
  LayoutDashboard,
  LogOut,
  Menu,
  ReceiptText,
  School,
  Truck,
  UserRoundCheck,
  UsersRound,
  X,
} from "lucide-react";
import toast from "react-hot-toast";

import { useNavigationDrawer } from "@/hooks/use-navigation-drawer";
import { ThemeToggle } from "@/lib/theme";
import { bootstrapService } from "@/modules/configs";
import type { SchoolConfiguration } from "@/modules/configs/types";
import { useAuthStore } from "@/stores/auth-store";

import { defaultModuleVisual, moduleRouteKey, moduleVisuals } from "./module-registry";
import { PageChromeProvider, usePageChromeContext } from "@/modules/admin/layouts/page-chrome";

interface ModuleLayoutProps {
  children: React.ReactNode;
}

type LocalNavItem = {
  label: string;
  path: string;
  icon: React.ComponentType<{ className?: string }>;
};

const moduleLabels: Record<string, string> = {
  sis: "People and admissions",
  academics: "Academics",
  timetabling: "Timetabling",
  messaging: "Communication",
  finance: "Finance",
  fees: "Fees and billing",
  library: "Library",
  hr_payroll: "HR and payroll",
  procurement: "Procurement",
  fleet: "Fleet",
  hostel: "Hostel",
  health: "Health services",
  assets_inventory: "Assets and inventory",
  document_registry: "Document registry",
  internal_audit: "Internal audit",
};

const fleetNavigation: LocalNavItem[] = [
  { label: "Vehicles", path: "/modules/fleet/vehicles", icon: Truck },
  { label: "Drivers", path: "/modules/fleet/drivers", icon: ClipboardList },
  { label: "Daily vehicle log", path: "/modules/fleet/daily-log", icon: ReceiptText },
];

const hrNavigation: LocalNavItem[] = [
  { label: "Employees", path: "/modules/hr-payroll/employees", icon: UsersRound },
  { label: "Departments", path: "/modules/hr-payroll/departments", icon: Building2 },
  { label: "Positions", path: "/modules/hr-payroll/positions", icon: BriefcaseBusiness },
];

const academicsNavigation: LocalNavItem[] = [
  { label: "Academic years", path: "/modules/academics/academic-years", icon: CalendarRange },
  { label: "Subjects", path: "/modules/academics/subjects", icon: BookOpen },
  { label: "Teachers", path: "/modules/academics/teachers", icon: UserRoundCheck },
  { label: "Classes", path: "/modules/academics/classes", icon: GraduationCap },
  { label: "Teaching assignments", path: "/modules/academics/teaching-assignments", icon: ClipboardList },
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
  const moduleLabel = moduleLabels[moduleKey] || "Module workspace";
  const visual = moduleVisuals[moduleKey] ?? defaultModuleVisual;
  const ModuleIcon = visual.icon;
  const localNavigation = moduleKey === "fleet" ? fleetNavigation : moduleKey === "hr_payroll" ? hrNavigation : moduleKey === "academics" ? academicsNavigation : [];

  useEffect(() => {
    let active = true;
    void bootstrapService.checkStatus().then((response) => {
      if (active && response.success && response.data?.school) setSchool(response.data.school);
    });
    return () => { active = false; };
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
      <a className="cp-skip-link" href="#main-content">Skip to main content</a>
      <aside
        aria-label={`${moduleLabel} navigation`}
        aria-hidden={!desktopNavigation && !sidebarOpen}
        className={`fixed inset-y-0 left-0 z-[70] flex w-[min(320px,calc(100vw-48px))] flex-col bg-[var(--sidebar)] text-[var(--sidebar-foreground)] transition-transform duration-300 ease-[var(--motion-ease-default)] lg:z-[var(--z-sidebar)] lg:w-[var(--sidebar-w)] lg:translate-x-0 ${sidebarOpen ? "translate-x-0" : "-translate-x-full"}`}
        id="module-navigation"
        ref={sidebarRef}
      >
        <div className="relative border-b border-[var(--sidebar-border)] px-5 pb-5 pt-6">
          <div aria-hidden="true" className="campus-grid-pattern absolute inset-0 opacity-40" />
          <div className="relative flex items-center gap-3">
            <span className="flex size-10 shrink-0 items-center justify-center rounded-[10px] bg-[var(--brand-highlight)] text-[var(--sidebar-active-fg)]">
              <ModuleIcon className="size-[18px]" />
            </span>
            <div className="min-w-0">
              <p className="truncate text-[15px] font-bold tracking-[-0.025em]">{moduleLabel}</p>
              <p className="truncate text-[11px] font-medium text-[var(--sidebar-muted)]">{school?.name || "Campus workspace"}</p>
            </div>
            <button aria-label="Close navigation" className="ml-auto inline-flex size-10 items-center justify-center rounded-[8px] border border-[var(--sidebar-border)] bg-white/5 text-[var(--sidebar-foreground)] hover:bg-[var(--sidebar-hover)] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--brand-highlight)] lg:hidden" onClick={() => setSidebarOpen(false)} type="button">
              <X className="size-5" />
            </button>
          </div>
          <Link className="relative mt-5 flex min-h-10 items-center gap-2 rounded-[8px] border border-[var(--sidebar-border)] bg-white/5 px-3 text-[13px] font-medium text-[var(--sidebar-muted)] hover:bg-[var(--sidebar-hover)] hover:text-[var(--sidebar-foreground)]" to="/home">
            <ChevronLeft className="size-4" />
            All modules
          </Link>
        </div>

        <nav className="cp-sidebar-scroll min-h-0 flex-1 overflow-y-auto px-3 py-4" aria-label="Module navigation">
          <section aria-labelledby="module-workspace-nav">
            <h2 className="mb-2 px-3 text-[10px] font-semibold uppercase tracking-[0.18em] text-[var(--sidebar-muted)]" id="module-workspace-nav">Workspace</h2>
            <div className="space-y-1">
              <LocalOverviewLink active={isModuleOverview(location.pathname)} moduleKey={moduleKey} />
              {localNavigation.map((item) => <LocalLink active={location.pathname === item.path} item={item} key={item.path} />)}
            </div>
          </section>
        </nav>

        <div className="border-t border-[var(--sidebar-border)] p-3">
          <ThemeToggle className="w-full" variant="sidebar" />
          <div className="mt-3 flex items-center gap-3 px-2">
            <span className="flex size-9 items-center justify-center rounded-full border border-[var(--sidebar-border)] bg-white/10 text-xs font-semibold">{initials(userName)}</span>
            <div className="min-w-0 flex-1">
              <p className="truncate text-[13px] font-semibold">{userName}</p>
              <p className="truncate text-[11px] text-[var(--sidebar-muted)]">{userRole}</p>
            </div>
          </div>
          <button className="mt-2 flex min-h-10 w-full items-center gap-3 rounded-[8px] px-3 text-[13px] font-medium text-[var(--sidebar-muted)] hover:bg-[var(--sidebar-hover)] hover:text-[var(--sidebar-foreground)] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--brand-highlight)]" onClick={() => void handleLogout()} type="button">
            <LogOut className="size-[17px]" /> Sign out
          </button>
        </div>
      </aside>

      {sidebarOpen ? <button aria-label="Close navigation" className="fixed inset-0 z-[65] bg-[var(--surface-overlay)] lg:hidden" onClick={() => setSidebarOpen(false)} type="button" /> : null}

      <div className="min-w-0 lg:pl-[var(--sidebar-w)]">
        <header className="fixed inset-x-0 top-0 z-[var(--z-nav)] flex h-[var(--app-bar-h)] items-center justify-between border-b border-[var(--border)] bg-[var(--surface)]/95 px-4 backdrop-blur-md lg:left-[var(--sidebar-w)] lg:px-8">
          <div className="flex min-w-0 items-center gap-3">
            <button aria-controls="module-navigation" aria-expanded={sidebarOpen} aria-hidden={sidebarOpen} aria-label="Open navigation" className={`inline-flex size-10 shrink-0 items-center justify-center rounded-[8px] border border-[var(--border)] bg-[var(--surface)] text-[var(--text-body)] hover:bg-[var(--surface-muted)] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--focus-ring)] lg:hidden ${sidebarOpen ? "invisible pointer-events-none" : ""}`} onClick={() => setSidebarOpen(true)} ref={menuButtonRef} tabIndex={sidebarOpen ? -1 : 0} type="button">
              <Menu className="size-5" />
            </button>
            <div className="min-w-0">
              <p className="truncate text-[14px] font-semibold text-[var(--text-strong)]">{pageTitle || moduleLabel}</p>
              <p className="hidden truncate text-[12px] text-[var(--text-muted)] sm:block">{school?.name || "Campus workspace"}</p>
            </div>
          </div>
          <div className="flex items-center gap-3">
            {pageAction ? <div className="hidden sm:block">{pageAction}</div> : null}
            <ThemeToggle className="lg:hidden" />
          </div>
        </header>
        <main className="min-h-[100dvh] pt-[var(--app-bar-h)]" id="main-content" tabIndex={-1}>
          <div className="campus-page-enter mx-auto max-w-[1480px] p-4 sm:p-6 lg:p-8">
            {pageAction ? <div className="mb-4 sm:hidden">{pageAction}</div> : null}
            {children}
          </div>
        </main>
      </div>
    </div>
  );
};

const LocalOverviewLink: React.FC<{ active: boolean; moduleKey: string }> = ({ active, moduleKey }) => (
  <Link
    aria-current={active ? "page" : undefined}
    className={navClass(active)}
    params={{ moduleKey: moduleRouteKey(moduleKey) }}
    to="/modules/$moduleKey"
  >
    <LayoutDashboard className="size-[17px]" />
    <span className="flex-1">Overview</span>
    {active ? <ChevronRight className="size-3.5" /> : null}
  </Link>
);

const LocalLink: React.FC<{ active: boolean; item: LocalNavItem }> = ({ active, item }) => {
  const Icon = item.icon;
  if (item.path === "/modules/fleet/vehicles") return <Link className={navClass(active)} to="/modules/fleet/vehicles"><Icon className="size-[17px]" /><span className="flex-1">{item.label}</span>{active ? <ChevronRight className="size-3.5" /> : null}</Link>;
  if (item.path === "/modules/fleet/drivers") return <Link className={navClass(active)} to="/modules/fleet/drivers"><Icon className="size-[17px]" /><span className="flex-1">{item.label}</span>{active ? <ChevronRight className="size-3.5" /> : null}</Link>;
  if (item.path === "/modules/fleet/daily-log") return <Link className={navClass(active)} to="/modules/fleet/daily-log"><Icon className="size-[17px]" /><span className="flex-1">{item.label}</span>{active ? <ChevronRight className="size-3.5" /> : null}</Link>;
  if (item.path === "/modules/hr-payroll/employees") return <Link className={navClass(active)} to="/modules/hr-payroll/employees"><Icon className="size-[17px]" /><span className="flex-1">{item.label}</span>{active ? <ChevronRight className="size-3.5" /> : null}</Link>;
  if (item.path === "/modules/hr-payroll/departments") return <Link className={navClass(active)} to="/modules/hr-payroll/departments"><Icon className="size-[17px]" /><span className="flex-1">{item.label}</span>{active ? <ChevronRight className="size-3.5" /> : null}</Link>;
  if (item.path === "/modules/hr-payroll/positions") return <Link className={navClass(active)} to="/modules/hr-payroll/positions"><Icon className="size-[17px]" /><span className="flex-1">{item.label}</span>{active ? <ChevronRight className="size-3.5" /> : null}</Link>;
  if (item.path === "/modules/academics/academic-years") return <Link className={navClass(active)} to="/modules/academics/academic-years"><Icon className="size-[17px]" /><span className="flex-1">{item.label}</span>{active ? <ChevronRight className="size-3.5" /> : null}</Link>;
  if (item.path === "/modules/academics/subjects") return <Link className={navClass(active)} to="/modules/academics/subjects"><Icon className="size-[17px]" /><span className="flex-1">{item.label}</span>{active ? <ChevronRight className="size-3.5" /> : null}</Link>;
  if (item.path === "/modules/academics/teachers") return <Link className={navClass(active)} to="/modules/academics/teachers"><Icon className="size-[17px]" /><span className="flex-1">{item.label}</span>{active ? <ChevronRight className="size-3.5" /> : null}</Link>;
  if (item.path === "/modules/academics/classes") return <Link className={navClass(active)} to="/modules/academics/classes"><Icon className="size-[17px]" /><span className="flex-1">{item.label}</span>{active ? <ChevronRight className="size-3.5" /> : null}</Link>;
  return <Link className={navClass(active)} to="/modules/academics/teaching-assignments"><Icon className="size-[17px]" /><span className="flex-1">{item.label}</span>{active ? <ChevronRight className="size-3.5" /> : null}</Link>;
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
  return name.trim().split(/\s+/).slice(0, 2).map((part) => part[0]?.toUpperCase() || "").join("");
}

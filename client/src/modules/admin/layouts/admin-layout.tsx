//
//  campus-pilot
//  admin-layout.tsx - CCS-inspired school operations shell
//

import React, { useEffect, useState } from "react";
import { Link, useLocation, useNavigate } from "@tanstack/react-router";
import {
  ChevronRight,
  Grid2X2,
  KeyRound,
  LayoutDashboard,
  LogOut,
  Menu,
  School,
  Settings2,
  ShieldCheck,
  UsersRound,
  X,
} from "lucide-react";
import toast from "react-hot-toast";

import { ThemeToggle } from "../../../lib/theme";
import { bootstrapService } from "../../configs";
import type { SchoolConfiguration } from "../../configs/types";
import { useAuthStore } from "../../../stores/auth-store";
import { PageChromeProvider, usePageChromeContext } from "./page-chrome";

interface AdminLayoutProps {
  children: React.ReactNode;
}

type NavItem = {
  label: string;
  href: string;
  permission: string;
  icon: React.ComponentType<{ className?: string }>;
};

type NavGroup = {
  label: string;
  items: NavItem[];
};

const navigationGroups: NavGroup[] = [
  {
    label: "Workspace",
    items: [{ label: "Overview", href: "/admin", permission: "administration:view", icon: LayoutDashboard }],
  },
  {
    label: "People & access",
    items: [
      { label: "Users", href: "/admin/users", permission: "users:view", icon: UsersRound },
      { label: "Roles and access", href: "/admin/roles", permission: "roles:view", icon: ShieldCheck },
    ],
  },
  {
    label: "Configuration",
    items: [
      { label: "Licensing", href: "/admin/licensing", permission: "licensing:view", icon: KeyRound },
      { label: "School settings", href: "/admin/settings", permission: "school_settings:view", icon: Settings2 },
    ],
  },
];

export const AdminLayout: React.FC<AdminLayoutProps> = ({ children }) => (
  <PageChromeProvider>
    <AdminLayoutShell>{children}</AdminLayoutShell>
  </PageChromeProvider>
);

const AdminLayoutShell: React.FC<AdminLayoutProps> = ({ children }) => {
  const { title: pageTitle, action: pageAction } = usePageChromeContext();
  const navigate = useNavigate();
  const location = useLocation();
  const { user, logout } = useAuthStore();
  const [sidebarOpen, setSidebarOpen] = useState(false);
  const [school, setSchool] = useState<SchoolConfiguration | null>(null);

  useEffect(() => {
    let active = true;
    void bootstrapService
      .checkStatus()
      .then((response) => {
        if (active && response.success && response.data?.school) {
          setSchool(response.data.school);
        }
      })
      .catch(() => undefined);
    return () => {
      active = false;
    };
  }, []);

  useEffect(() => {
    setSidebarOpen(false);
  }, [location.pathname]);

  useEffect(() => {
    if (!sidebarOpen) return;
    const previousOverflow = document.body.style.overflow;
    document.body.style.overflow = "hidden";
    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key === "Escape") setSidebarOpen(false);
    };
    window.addEventListener("keydown", closeOnEscape);
    return () => {
      document.body.style.overflow = previousOverflow;
      window.removeEventListener("keydown", closeOnEscape);
    };
  }, [sidebarOpen]);

  const handleLogout = async () => {
    await logout();
    toast.success("Signed out");
    navigate({ to: "/login", replace: true });
  };

  const schoolName = school?.name || "Campus Pilot School";
  const userName = user?.full_name || "Administrator";
  const userRole = user?.role_names?.[0] || "Campus administrator";
  const visibleNavigationGroups = navigationGroups
    .map((group) => ({
      ...group,
      items: group.items.filter((item) => hasPermission(user?.permissions, item.permission)),
    }))
    .filter((group) => group.items.length > 0);

  return (
    <div className="min-h-[100dvh] bg-[var(--canvas)]">
      <a className="cp-skip-link" href="#main-content">
        Skip to main content
      </a>

      <aside
        aria-label="Administration navigation"
        className={`fixed inset-y-0 left-0 z-[70] flex w-[min(320px,calc(100vw-48px))] flex-col bg-[var(--sidebar)] text-[var(--sidebar-foreground)] transition-transform duration-300 ease-[var(--motion-ease-default)] lg:z-[var(--z-sidebar)] lg:w-[var(--sidebar-w)] lg:translate-x-0 ${
          sidebarOpen ? "translate-x-0" : "-translate-x-full"
        }`}
        id="campus-navigation"
      >
        <div className="relative border-b border-[var(--sidebar-border)] px-5 pb-5 pt-6">
          <div aria-hidden="true" className="campus-grid-pattern absolute inset-0 opacity-45" />
          <div className="relative flex items-center gap-3">
            <span className="flex size-10 shrink-0 items-center justify-center rounded-[10px] bg-[var(--sidebar-active)] text-[var(--sidebar-active-fg)] shadow-sm">
              <img
                alt=""
                aria-hidden="true"
                className="size-7 rounded-full object-cover mix-blend-multiply"
                src="/assets/images/campus-pilot-logo.svg"
              />
            </span>
            <div className="min-w-0">
              <p className="text-[15px] font-bold tracking-[-0.025em] text-[var(--sidebar-foreground)]">
                Administration
              </p>
              <p className="text-[11px] font-medium uppercase tracking-[0.16em] text-[var(--sidebar-muted)]">
                Campus management
              </p>
            </div>
            <button
              aria-label="Close navigation"
              className="ml-auto inline-flex size-10 shrink-0 items-center justify-center rounded-[8px] border border-[var(--sidebar-border)] bg-white/5 text-[var(--sidebar-foreground)] hover:bg-[var(--sidebar-hover)] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--brand-highlight)] lg:hidden"
              onClick={() => setSidebarOpen(false)}
              type="button"
            >
              <X className="size-5" />
            </button>
          </div>

          <div className="relative mt-5 rounded-[10px] border border-[var(--sidebar-border)] bg-white/5 p-3">
            <div className="flex items-center gap-3">
              <span className="flex size-9 shrink-0 items-center justify-center overflow-hidden rounded-[8px] bg-white/10 text-[var(--brand-highlight)]">
                {school?.logo_dark_url || school?.logo_light_url ? (
                  <img
                    alt=""
                    className="size-full object-cover"
                    src={school.logo_dark_url || school.logo_light_url || undefined}
                  />
                ) : (
                  <School className="size-4" />
                )}
              </span>
              <div className="min-w-0">
                <p className="truncate text-[13px] font-semibold text-[var(--sidebar-foreground)]">
                  {schoolName}
                </p>
                <p className="mt-0.5 truncate text-[11px] text-[var(--sidebar-muted)]">
                  Active campus
                </p>
              </div>
            </div>
          </div>

          <Link
            className="relative mt-3 flex min-h-10 items-center gap-2 rounded-[8px] border border-[var(--sidebar-border)] bg-white/5 px-3 text-[13px] font-medium text-[var(--sidebar-muted)] hover:bg-[var(--sidebar-hover)] hover:text-[var(--sidebar-foreground)]"
            to="/home"
          >
            <Grid2X2 className="size-4" />
            All modules
          </Link>
        </div>

        <nav className="cp-sidebar-scroll min-h-0 flex-1 overflow-y-auto px-3 py-4" aria-label="Primary navigation">
          <div className="space-y-6">
            {visibleNavigationGroups.map((group) => (
              <section aria-labelledby={`nav-${group.label}`} key={group.label}>
                <h2
                  className="mb-2 px-3 text-[10px] font-semibold uppercase tracking-[0.18em] text-[var(--sidebar-muted)]"
                  id={`nav-${group.label}`}
                >
                  {group.label}
                </h2>
                <div className="space-y-1">
                  {group.items.map(({ href, icon: Icon, label }) => {
                    const active =
                      href === "/admin"
                        ? location.pathname === href
                        : location.pathname === href || location.pathname.startsWith(`${href}/`);
                    return (
                      <Link
                        aria-current={active ? "page" : undefined}
                        className={`group flex min-h-10 items-center gap-3 rounded-[8px] px-3 text-[13px] font-medium focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--brand-highlight)] ${
                          active
                            ? "bg-[var(--sidebar-active)] text-[var(--sidebar-active-fg)]"
                            : "text-[var(--sidebar-muted)] hover:bg-[var(--sidebar-hover)] hover:text-[var(--sidebar-foreground)]"
                        }`}
                        key={href}
                        to={href}
                      >
                        <Icon className="size-[17px] shrink-0" />
                        <span className="min-w-0 flex-1 truncate">{label}</span>
                        {active ? <ChevronRight className="size-3.5 shrink-0" /> : null}
                      </Link>
                    );
                  })}
                </div>
              </section>
            ))}
          </div>
        </nav>

        <div className="border-t border-[var(--sidebar-border)] p-3">
          <ThemeToggle className="w-full" variant="sidebar" />
          <div className="mt-3 flex items-center gap-3 px-2">
            <span className="flex size-9 shrink-0 items-center justify-center rounded-full border border-[var(--sidebar-border)] bg-white/10 text-xs font-semibold text-[var(--sidebar-foreground)]">
              {initials(userName)}
            </span>
            <div className="min-w-0 flex-1">
              <p className="truncate text-[13px] font-semibold text-[var(--sidebar-foreground)]">{userName}</p>
              <p className="truncate text-[11px] text-[var(--sidebar-muted)]">{userRole}</p>
            </div>
          </div>
          <button
            className="mt-3 flex min-h-10 w-full items-center gap-3 rounded-[8px] px-3 text-left text-[13px] font-medium text-[var(--sidebar-muted)] hover:bg-[var(--sidebar-hover)] hover:text-[var(--sidebar-foreground)] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--brand-highlight)]"
            onClick={() => void handleLogout()}
            type="button"
          >
            <LogOut className="size-4" />
            Sign out
          </button>
        </div>
      </aside>

      {sidebarOpen ? (
        <button
          aria-label="Dismiss navigation"
          className="fixed inset-0 z-[65] bg-[var(--surface-overlay)] lg:hidden"
          onClick={() => setSidebarOpen(false)}
          type="button"
        />
      ) : null}

      <div className="min-w-0 lg:pl-[var(--sidebar-w)]">
        <header className="fixed inset-x-0 top-0 z-[var(--z-nav)] flex h-[var(--app-bar-h)] items-center justify-between border-b border-[var(--border)] bg-[var(--surface)]/95 px-4 backdrop-blur-md lg:left-[var(--sidebar-w)] lg:px-8">
          <div className="flex min-w-0 items-center gap-3">
            <button
              aria-controls="campus-navigation"
              aria-expanded={sidebarOpen}
              aria-hidden={sidebarOpen}
              aria-label="Open navigation"
              className={`inline-flex size-10 shrink-0 items-center justify-center rounded-[8px] border border-[var(--border)] bg-[var(--surface)] text-[var(--text-body)] hover:bg-[var(--surface-muted)] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--focus-ring)] lg:hidden ${sidebarOpen ? "invisible pointer-events-none" : ""}`}
              onClick={() => setSidebarOpen((open) => !open)}
              tabIndex={sidebarOpen ? -1 : 0}
              type="button"
            >
              <Menu className="size-5" />
            </button>
            <div className="min-w-0">
              <p className="truncate text-[14px] font-semibold text-[var(--text-strong)]">{pageTitle}</p>
              <p className="hidden truncate text-[12px] text-[var(--text-muted)] sm:block">{schoolName}</p>
            </div>
          </div>

          <div className="flex items-center gap-3">
            {pageAction ? <div className="hidden sm:block">{pageAction}</div> : null}
            <ThemeToggle className="lg:hidden" />
            <span className="hidden size-9 items-center justify-center rounded-full bg-[var(--brand-soft)] text-xs font-semibold text-[var(--brand-strong)] sm:flex lg:hidden">
              {initials(userName)}
            </span>
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

function initials(name: string) {
  return name
    .trim()
    .split(/\s+/)
    .slice(0, 2)
    .map((part) => part[0]?.toUpperCase() || "")
    .join("");
}

function hasPermission(permissions: string[] | undefined, permission: string) {
  return permissions?.includes("*") || permissions?.includes(permission) || false;
}

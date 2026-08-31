import React, { useEffect, useMemo, useState } from "react";
import { Link, useNavigate } from "@tanstack/react-router";
import {
  ArrowRight,
  CalendarDays,
  ChevronRight,
  Grid2X2,
  LogOut,
  Menu,
  Search,
  School,
  ShieldCheck,
  X,
} from "lucide-react";
import toast from "react-hot-toast";

import { Button } from "@/components/ui/button";
import { useNavigationDrawer } from "@/hooks/use-navigation-drawer";
import { ThemeToggle } from "@/lib/theme";
import { bootstrapService } from "@/modules/configs";
import type { SchoolConfiguration } from "@/modules/configs/types";
import { AgentWidget } from "@/modules/agent";
import { useAuthStore } from "@/stores/auth-store";

import { accessService } from "./access-service";
import { defaultModuleVisual, moduleRouteKey, moduleVisuals, stageLabel } from "./module-registry";
import type { ModuleDefinition } from "./types";

const RECENT_MODULE_KEY = "campuspilot_recent_module";

export const CampusHome: React.FC = () => {
  const navigate = useNavigate();
  const { user, logout } = useAuthStore();
  const [school, setSchool] = useState<SchoolConfiguration | null>(null);
  const [catalog, setCatalog] = useState<ModuleDefinition[]>([]);
  const [searchQuery, setSearchQuery] = useState("");
  const [recentModuleKey, setRecentModuleKey] = useState(() => localStorage.getItem(RECENT_MODULE_KEY));
  const [isLoading, setIsLoading] = useState(true);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [sidebarOpen, setSidebarOpen] = useState(false);
  const {
    desktopNavigation,
    navigationRef: sidebarRef,
    triggerRef: menuButtonRef,
  } = useNavigationDrawer(sidebarOpen, setSidebarOpen);

  useEffect(() => {
    let active = true;
    void Promise.all([bootstrapService.checkStatus(), accessService.getCatalog()])
      .then(([schoolResponse, catalogResponse]) => {
        if (!active) return;
        if (schoolResponse.success && schoolResponse.data?.school) {
          setSchool(schoolResponse.data.school);
        }
        if (catalogResponse.success && catalogResponse.data) {
          setCatalog(catalogResponse.data.modules);
          setLoadError(null);
        } else {
          setLoadError("Your campus modules could not be loaded. Refresh the page to try again.");
        }
      })
      .catch(() => {
        if (active) {
          setLoadError("Campus Pilot could not reach the module catalog. Check your connection and try again.");
        }
      })
      .finally(() => {
        if (active) setIsLoading(false);
      });
    return () => {
      active = false;
    };
  }, []);

  const accessibleModules = useMemo(() => {
    if (!user) return [];
    const hasOwnerAccess =
      user.roles?.includes("campus_owner") || user.permissions?.includes("*");

    return catalog.filter((module) => {
      // Core workspaces cannot be disabled. Keeping the owner fallback here also
      // prevents an older persisted session from hiding Administration while the
      // authenticated profile refresh completes.
      const enabled =
        (user.modules?.includes(module.key) ?? false) ||
        (hasOwnerAccess && module.core);
      const authorized =
        hasOwnerAccess ||
        user.permissions?.some((permission) => permission.startsWith(`${module.permission_namespace}:`));
      return enabled && authorized;
    });
  }, [catalog, user]);

  const filteredModules = useMemo(() => {
    const query = searchQuery.trim().toLowerCase();
    if (!query) return accessibleModules;
    return accessibleModules.filter((module) =>
      [module.label, module.group, module.description].some((value) => value.toLowerCase().includes(query)),
    );
  }, [accessibleModules, searchQuery]);

  const recentModule = accessibleModules.find((module) => module.key === recentModuleKey);
  const featuredModule =
    accessibleModules.find((module) => module.key === "administration") ??
    recentModule ??
    accessibleModules[0];

  const groupedModules = useMemo(() => {
    const modules = searchQuery
      ? filteredModules
      : filteredModules.filter((module) => module.key !== featuredModule?.key);
    return modules.reduce<Record<string, ModuleDefinition[]>>((groups, module) => {
      (groups[module.group] ||= []).push(module);
      return groups;
    }, {});
  }, [featuredModule?.key, filteredModules, searchQuery]);

  const handleModuleOpen = (moduleKey: string) => {
    localStorage.setItem(RECENT_MODULE_KEY, moduleKey);
    setRecentModuleKey(moduleKey);
  };

  const handleLogout = async () => {
    await logout();
    toast.success("Signed out");
    navigate({ to: "/login", replace: true });
  };

  const schoolName = school?.name || "Your campus";
  const firstName = user?.full_name.trim().split(/\s+/)[0] || "there";
  const hasOwnerAccess =
    user?.roles?.includes("campus_owner") || user?.permissions?.includes("*");
  const userName = user?.full_name || "Campus user";
  const userRole = user?.role_names?.[0] || "Campus access";
  const sidebarModules = [
    accessibleModules.find((module) => module.key === "administration"),
    recentModule,
  ].filter((module, index, modules): module is ModuleDefinition =>
    Boolean(module) && modules.findIndex((candidate) => candidate?.key === module?.key) === index,
  );
  const sidebarGroups = Array.from(
    new Set(accessibleModules.filter((module) => module.key !== "administration").map((module) => module.group)),
  );

  return (
    <div className="min-h-[100dvh] bg-[var(--canvas)]">
      <a className="cp-skip-link" href="#main-content">Skip to main content</a>
      <aside
        aria-label="Campus navigation"
        aria-hidden={!desktopNavigation && !sidebarOpen}
        className={`fixed inset-y-0 left-0 z-[70] flex w-[min(320px,calc(100vw-48px))] flex-col bg-[var(--sidebar)] text-[var(--sidebar-foreground)] transition-transform duration-300 ease-[var(--motion-ease-default)] lg:z-[var(--z-sidebar)] lg:w-[var(--sidebar-w)] lg:translate-x-0 ${sidebarOpen ? "translate-x-0" : "-translate-x-full"}`}
        id="launcher-navigation"
        ref={sidebarRef}
      >
        <div className="relative border-b border-[var(--sidebar-border)] px-5 pb-5 pt-6">
          <div aria-hidden="true" className="campus-grid-pattern absolute inset-0 opacity-45" />
          <div className="relative flex items-center gap-3">
            <span className="flex size-10 shrink-0 items-center justify-center rounded-[10px] bg-[var(--sidebar-active)] text-[var(--sidebar-active-fg)] shadow-sm">
              <img alt="" aria-hidden="true" className="size-7 rounded-full object-cover mix-blend-multiply" src="/assets/images/campus-pilot-logo.svg" />
            </span>
            <div className="min-w-0">
              <p className="text-[15px] font-bold tracking-[-0.025em]">Campus Pilot</p>
              <p className="text-[11px] font-medium uppercase tracking-[0.16em] text-[var(--sidebar-muted)]">Module launcher</p>
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
                  <img alt="" className="size-full object-cover" src={school.logo_dark_url || school.logo_light_url || undefined} />
                ) : (
                  <School className="size-4" />
                )}
              </span>
              <div className="min-w-0">
                <p className="truncate text-[13px] font-semibold">{schoolName}</p>
                <p className="mt-0.5 truncate text-[11px] text-[var(--sidebar-muted)]">Active campus</p>
              </div>
            </div>
          </div>
        </div>

        <nav aria-label="Primary navigation" className="cp-sidebar-scroll min-h-0 flex-1 overflow-y-auto px-3 py-4">
          <div className="space-y-6">
            <section aria-labelledby="launcher-workspace-nav">
              <h2 className="mb-2 px-3 text-[10px] font-semibold uppercase tracking-[0.18em] text-[var(--sidebar-muted)]" id="launcher-workspace-nav">Workspace</h2>
              <Link
                aria-current="page"
                className="flex min-h-10 items-center gap-3 rounded-[8px] bg-[var(--sidebar-active)] px-3 text-[13px] font-medium text-[var(--sidebar-active-fg)] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--brand-highlight)]"
                onClick={() => setSidebarOpen(false)}
                to="/home"
              >
                <Grid2X2 className="size-[17px] shrink-0" />
                <span className="min-w-0 flex-1 truncate">All modules</span>
                <ChevronRight className="size-3.5 shrink-0" />
              </Link>
            </section>

            {sidebarModules.length > 0 ? (
              <section aria-labelledby="launcher-quick-nav">
                <h2 className="mb-2 px-3 text-[10px] font-semibold uppercase tracking-[0.18em] text-[var(--sidebar-muted)]" id="launcher-quick-nav">Quick access</h2>
                <div className="space-y-1">
                  {sidebarModules.map((module) => (
                    <SidebarModuleLink
                      key={module.key}
                      module={module}
                      onNavigate={() => setSidebarOpen(false)}
                      onOpen={handleModuleOpen}
                    />
                  ))}
                </div>
              </section>
            ) : null}

            {sidebarGroups.length > 0 ? (
              <section aria-labelledby="launcher-browse-nav" className="hidden lg:block">
                <h2 className="mb-2 px-3 text-[10px] font-semibold uppercase tracking-[0.18em] text-[var(--sidebar-muted)]" id="launcher-browse-nav">Browse by task</h2>
                <div className="space-y-1">
                  {sidebarGroups.map((group) => (
                    <a
                      className="flex min-h-10 items-center gap-3 rounded-[8px] px-3 text-[13px] font-medium text-[var(--sidebar-muted)] hover:bg-[var(--sidebar-hover)] hover:text-[var(--sidebar-foreground)] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--brand-highlight)]"
                      href={`#module-group-${slug(group)}`}
                      key={group}
                      onClick={() => setSidebarOpen(false)}
                    >
                      <span aria-hidden="true" className="size-1.5 rounded-full bg-[var(--brand-highlight)]" />
                      <span className="truncate">{group}</span>
                    </a>
                  ))}
                </div>
              </section>
            ) : null}
          </div>
        </nav>

        <div className="border-t border-[var(--sidebar-border)] p-3">
          <AgentWidget context={{ label: "All modules", moduleKey: "home", route: "/home" }} />
          <ThemeToggle className="w-full" variant="sidebar" />
          <div className="mt-3 flex items-center gap-3 px-2">
            <span className="flex size-9 shrink-0 items-center justify-center rounded-full border border-[var(--sidebar-border)] bg-white/10 text-xs font-semibold">{initials(userName)}</span>
            <div className="min-w-0 flex-1">
              <p className="truncate text-[13px] font-semibold">{userName}</p>
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
        <button aria-label="Dismiss navigation" className="fixed inset-0 z-[65] bg-[var(--surface-overlay)] lg:hidden" onClick={() => setSidebarOpen(false)} type="button" />
      ) : null}

      <div className="min-w-0 lg:pl-[var(--sidebar-w)]">
        <header className="fixed inset-x-0 top-0 z-[var(--z-nav)] flex h-[var(--app-bar-h)] items-center justify-between border-b border-[var(--border)] bg-[var(--surface)]/95 px-4 backdrop-blur-md lg:left-[var(--sidebar-w)] lg:px-8">
          <div className="flex min-w-0 items-center gap-3">
            <button
              aria-controls="launcher-navigation"
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
              <p className="truncate text-[14px] font-semibold text-[var(--text-strong)]">All modules</p>
              <p className="hidden truncate text-[12px] text-[var(--text-muted)] sm:block">{schoolName}</p>
            </div>
          </div>
          <div className="flex items-center gap-3 lg:hidden">
            <ThemeToggle />
            <span className="hidden size-9 items-center justify-center rounded-full bg-[var(--brand-soft)] text-xs font-semibold text-[var(--brand-strong)] sm:flex">{initials(userName)}</span>
          </div>
        </header>

        <main className="min-h-[100dvh] pt-[var(--app-bar-h)]" id="main-content" tabIndex={-1}>
          <div className="campus-page-enter mx-auto max-w-[1280px] px-4 py-7 sm:px-6 sm:py-9 lg:px-8 lg:py-10">
            <section className="flex flex-col gap-5 border-b border-[var(--border)] pb-7 sm:flex-row sm:items-end sm:justify-between">
              <div>
                <div className="flex items-center gap-2 text-[11px] font-semibold uppercase tracking-[0.18em] text-[var(--brand-strong)]">
                  <Grid2X2 className="size-3.5" />
                  Campus workspace
                </div>
                <h1 className="mt-3 text-[clamp(2rem,4vw,2.75rem)] font-semibold leading-[1.08] tracking-[-0.045em] text-[var(--text-strong)]">
                  {greeting()}, {firstName}.
                </h1>
                <p className="mt-3 max-w-[42em] text-sm leading-6 text-[var(--text-muted)] sm:text-base">
                  Open a workspace for {schoolName}.
                </p>
              </div>
              <div className="flex shrink-0 items-center gap-2 text-xs font-medium text-[var(--text-muted)] sm:pb-1">
                <CalendarDays className="size-4 text-[var(--brand-strong)]" />
                {formattedDate()}
              </div>
            </section>

            <section aria-labelledby="workspace-heading" className="mt-7 flex flex-col gap-4 sm:flex-row sm:items-end sm:justify-between">
              <div>
                <h2 className="text-xl font-semibold tracking-[-0.025em] text-[var(--text-strong)]" id="workspace-heading">Your workspaces</h2>
                <p className="mt-1 text-sm text-[var(--text-muted)]">
                  {accessibleModules.length > 0
                    ? `${accessibleModules.length} ${accessibleModules.length === 1 ? "module" : "modules"} available to you`
                    : "Your available modules will appear here"}
                </p>
              </div>
              <label className="relative block w-full sm:max-w-[360px]" htmlFor="module-search">
                <span className="sr-only">Search modules</span>
                <Search className="pointer-events-none absolute left-3.5 top-1/2 size-[17px] -translate-y-1/2 text-[var(--text-muted)]" />
                <input
                  className="h-11 w-full rounded-[var(--radius-lg)] border border-[var(--input-border)] bg-[var(--surface)] pl-10 pr-4 text-base text-[var(--text-strong)] shadow-[var(--shadow-rest)] placeholder:text-[var(--text-subtle)] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--focus-ring)] sm:text-sm"
                  id="module-search"
                  onChange={(event) => setSearchQuery(event.target.value)}
                  placeholder="Find a workspace"
                  type="search"
                  value={searchQuery}
                />
              </label>
            </section>

        {isLoading ? <ModuleLauncherSkeleton /> : null}

        {!isLoading && loadError ? (
          <section className="mt-12 rounded-[var(--radius-xl)] border border-[var(--tone-danger-bd)] bg-[var(--tone-danger-bg)] p-6" role="alert">
            <h2 className="font-semibold text-[var(--tone-danger-strong)]">Modules could not be loaded</h2>
            <p className="mt-2 max-w-[34em] text-sm leading-6 text-[var(--tone-danger-strong)]">{loadError}</p>
            <Button className="mt-4" onClick={() => window.location.reload()} variant="secondary">Refresh page</Button>
          </section>
        ) : null}

        {!isLoading && !loadError && accessibleModules.length === 0 ? (
          <section className="mt-6 flex items-start gap-4 border-y border-[var(--border)] py-8">
            <span className="flex size-11 shrink-0 items-center justify-center rounded-[var(--radius-lg)] bg-[var(--brand-soft)] text-[var(--brand-strong)]">
              <ShieldCheck className="size-5" />
            </span>
            <div>
              <h2 className="text-lg font-semibold text-[var(--text-strong)]">
                {hasOwnerAccess ? "Administration is temporarily unavailable" : "No workspaces are assigned yet"}
              </h2>
              <p className="mt-1 max-w-[40em] text-sm leading-6 text-[var(--text-muted)]">
                {hasOwnerAccess
                  ? "Refresh to try loading Administration again."
                  : "Ask your campus administrator to assign a role with access to an enabled module."}
              </p>
              {hasOwnerAccess ? <Button className="mt-4" onClick={() => window.location.reload()} variant="secondary">Refresh access</Button> : null}
            </div>
          </section>
        ) : null}

        {!isLoading && !loadError && featuredModule && !searchQuery ? (
          <section className="mt-6" aria-label={featuredModule.key === "administration" ? "Administration workspace" : "Continue working"}>
            <FeaturedModule
              context={featuredModule.key === "administration" ? "Pinned" : recentModule ? "Continue where you left off" : "Start here"}
              module={featuredModule}
              onOpen={handleModuleOpen}
            />
          </section>
        ) : null}

        {!isLoading && !loadError && filteredModules.length === 0 && searchQuery ? (
          <section className="mt-6 flex items-start gap-4 border-y border-[var(--border)] py-8">
            <Search className="mt-0.5 size-5 shrink-0 text-[var(--text-subtle)]" />
            <div>
              <h2 className="text-lg font-semibold text-[var(--text-strong)]">No workspaces match “{searchQuery}”</h2>
              <p className="mt-1 text-sm text-[var(--text-muted)]">Try a module name or school task.</p>
            </div>
          </section>
        ) : null}

            <div className="mt-9 space-y-10">
              {Object.entries(groupedModules).map(([group, modules]) => (
                <section aria-labelledby={`module-group-${slug(group)}`} className="scroll-mt-24" key={group}>
                  <div className="mb-1 flex items-center gap-3">
                    <h2 className="shrink-0 text-[11px] font-semibold uppercase tracking-[0.16em] text-[var(--brand-strong)]" id={`module-group-${slug(group)}`}>{group}</h2>
                    <span aria-hidden="true" className="h-px flex-1 bg-[var(--border)]" />
                  </div>
                  <div className="grid gap-x-8 md:grid-cols-2 xl:grid-cols-3">
                    {modules.map((module) => <ModuleRow key={module.key} module={module} onOpen={handleModuleOpen} />)}
                  </div>
                </section>
              ))}
            </div>
          </div>
        </main>
      </div>
    </div>
  );
};

const SidebarModuleLink: React.FC<{
  module: ModuleDefinition;
  onNavigate: () => void;
  onOpen: (key: string) => void;
}> = ({ module, onNavigate, onOpen }) => {
  const visual = moduleVisuals[module.key] ?? defaultModuleVisual;
  const Icon = visual.icon;

  return (
    <ModuleDestination
      module={module}
      onOpen={(key) => {
        onOpen(key);
        onNavigate();
      }}
    >
      <span className="group flex min-h-10 items-center gap-3 rounded-[8px] px-3 text-[13px] font-medium text-[var(--sidebar-muted)] hover:bg-[var(--sidebar-hover)] hover:text-[var(--sidebar-foreground)]">
        <Icon className="size-[17px] shrink-0 text-[var(--brand-highlight)]" />
        <span className="min-w-0 flex-1 truncate">{module.label}</span>
        <ArrowRight className="size-3.5 shrink-0 transition-transform group-hover:translate-x-0.5" />
      </span>
    </ModuleDestination>
  );
};

const FeaturedModule: React.FC<{ context: string; module: ModuleDefinition; onOpen: (key: string) => void }> = ({ context, module, onOpen }) => {
  const visual = moduleVisuals[module.key] ?? defaultModuleVisual;
  const Icon = visual.icon;
  return (
    <ModuleDestination module={module} onOpen={onOpen}>
      <div className="group relative overflow-hidden rounded-[var(--radius-xl)] bg-[var(--sidebar)] px-5 py-5 text-[var(--sidebar-foreground)] sm:px-6">
        <div aria-hidden="true" className="campus-grid-pattern absolute inset-0 opacity-25" />
        <div className="relative grid gap-5 md:grid-cols-[minmax(0,1fr)_auto] md:items-center">
          <div className="flex items-start gap-4 sm:items-center">
            <span className="flex size-11 shrink-0 items-center justify-center rounded-[10px] bg-[var(--brand-highlight)] text-[var(--sidebar-active-fg)]">
              <Icon className="size-[19px]" />
            </span>
            <div className="min-w-0">
              <div className="flex flex-wrap items-center gap-2">
                <span className="text-[11px] font-semibold uppercase tracking-[0.16em] text-[var(--brand-highlight)]">{context}</span>
                <span aria-hidden="true" className="size-1 rounded-full bg-[var(--brand-highlight)]" />
                <span className="text-[11px] font-medium text-[var(--on-brand-muted)]">{module.group}</span>
              </div>
              <h3 className="mt-1.5 text-xl font-semibold tracking-[-0.025em]">{module.label}</h3>
              <p className="mt-1 max-w-2xl text-sm leading-5 text-[var(--sidebar-muted)]">{module.description}</p>
            </div>
          </div>
          <div className="flex items-center justify-between gap-4 border-t border-[var(--sidebar-border)] pt-4 md:border-l md:border-t-0 md:pl-6 md:pt-0">
            <span className="text-sm font-semibold">Open {module.key === "administration" ? "Administration" : "workspace"}</span>
            <span className="flex size-9 items-center justify-center rounded-[var(--radius-md)] bg-white/10 transition-transform duration-200 ease-[var(--motion-ease-default)] group-hover:translate-x-1">
              <ArrowRight className="size-4" />
            </span>
          </div>
        </div>
      </div>
    </ModuleDestination>
  );
};

const ModuleRow: React.FC<{ module: ModuleDefinition; onOpen: (key: string) => void }> = ({ module, onOpen }) => {
  const visual = moduleVisuals[module.key] ?? defaultModuleVisual;
  const Icon = visual.icon;
  return (
    <ModuleDestination module={module} onOpen={onOpen}>
      <div className="group flex min-h-[110px] items-start gap-3.5 border-b border-[var(--border-subtle)] py-4">
        <span className="flex size-10 shrink-0 items-center justify-center rounded-[9px] bg-[var(--brand-soft)] text-[var(--brand-strong)] transition-colors group-hover:bg-[var(--brand)] group-hover:text-[var(--on-brand)]">
          <Icon className="size-[18px]" />
        </span>
        <span className="min-w-0 flex-1">
          <span className="flex items-baseline justify-between gap-3">
            <span className="text-base font-semibold tracking-[-0.015em] text-[var(--text-strong)]">{module.label}</span>
            <span className="shrink-0 text-[11px] font-medium text-[var(--text-subtle)]">{stageLabel(module.stage)}</span>
          </span>
          <span className="mt-1.5 block text-sm leading-5 text-[var(--text-muted)]">{module.description}</span>
        </span>
        <ArrowRight className="mt-1 size-4 shrink-0 text-[var(--text-subtle)] transition-transform group-hover:translate-x-1 group-hover:text-[var(--brand-strong)]" />
      </div>
    </ModuleDestination>
  );
};

const ModuleDestination: React.FC<{
  children: React.ReactNode;
  module: ModuleDefinition;
  onOpen: (key: string) => void;
}> = ({ children, module, onOpen }) => {
  if (module.key === "administration") {
    return <Link className="block focus-visible:rounded-[var(--radius-2xl)]" onClick={() => onOpen(module.key)} to="/admin">{children}</Link>;
  }
  return (
    <Link
      className="block focus-visible:rounded-[var(--radius-xl)]"
      onClick={() => onOpen(module.key)}
      params={{ moduleKey: moduleRouteKey(module.key) }}
      to="/modules/$moduleKey"
    >
      {children}
    </Link>
  );
};

const ModuleLauncherSkeleton = () => (
  <div aria-label="Loading campus modules" className="mt-6 space-y-7" role="status">
    <div className="h-32 animate-pulse rounded-[var(--radius-xl)] bg-[var(--surface-sunken)]" />
    <div className="grid gap-8 md:grid-cols-2 xl:grid-cols-3">
      {[0, 1, 2, 3, 4, 5].map((item) => <div className="h-28 animate-pulse border-b border-[var(--border)] bg-[var(--surface-muted)]" key={item} />)}
    </div>
  </div>
);

function greeting() {
  const hour = new Date().getHours();
  if (hour < 12) return "Good morning";
  if (hour < 18) return "Good afternoon";
  return "Good evening";
}

function formattedDate() {
  return new Intl.DateTimeFormat("en-ZA", { weekday: "long", day: "numeric", month: "long", year: "numeric" }).format(new Date());
}

function slug(value: string) {
  return value.toLowerCase().replace(/[^a-z0-9]+/g, "-");
}

function initials(name: string) {
  return name
    .trim()
    .split(/\s+/)
    .slice(0, 2)
    .map((part) => part[0]?.toUpperCase() || "")
    .join("");
}

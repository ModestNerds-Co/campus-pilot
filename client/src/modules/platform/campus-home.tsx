import React, { useEffect, useMemo, useState } from "react";
import { Link, useNavigate } from "@tanstack/react-router";
import {
  ArrowRight,
  CalendarDays,
  Grid2X2,
  LogOut,
  Search,
  School,
  ShieldCheck,
} from "lucide-react";
import toast from "react-hot-toast";

import { Button } from "@/components/ui/button";
import { ThemeToggle } from "@/lib/theme";
import { bootstrapService } from "@/modules/configs";
import type { SchoolConfiguration } from "@/modules/configs/types";
import { useAuthStore } from "@/stores/auth-store";

import { accessService } from "./access-service";
import { defaultModuleVisual, moduleVisuals, stageLabel } from "./module-registry";
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
    return catalog.filter((module) => {
      const enabled = user.modules?.includes(module.key) ?? false;
      const authorized =
        user.permissions?.includes("*") ||
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

  const recentModule =
    accessibleModules.find((module) => module.key === recentModuleKey) ??
    accessibleModules.find((module) => module.key === "administration") ??
    accessibleModules[0];

  const groupedModules = useMemo(() => {
    const modules = searchQuery ? filteredModules : filteredModules.filter((module) => module.key !== recentModule?.key);
    return modules.reduce<Record<string, ModuleDefinition[]>>((groups, module) => {
      (groups[module.group] ||= []).push(module);
      return groups;
    }, {});
  }, [filteredModules, recentModule?.key, searchQuery]);

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

  return (
    <div className="min-h-[100dvh] bg-[var(--canvas)]">
      <a className="cp-skip-link" href="#main-content">Skip to main content</a>
      <header className="relative overflow-hidden bg-[var(--sidebar)] text-[var(--sidebar-foreground)]">
        <div aria-hidden="true" className="campus-grid-pattern absolute inset-0 opacity-45" />
        <div className="relative mx-auto flex max-w-[1480px] items-center gap-4 px-4 py-4 sm:px-6 lg:px-8">
          <Link className="flex min-w-0 items-center gap-3" to="/home">
            <span className="flex size-10 shrink-0 items-center justify-center rounded-[10px] bg-[var(--brand-highlight)] text-[var(--sidebar-active-fg)]">
              <img alt="" aria-hidden="true" className="size-7 rounded-full object-cover mix-blend-multiply" src="/assets/images/campus-pilot-logo.svg" />
            </span>
            <span className="min-w-0">
              <span className="block text-[15px] font-bold tracking-[-0.025em]">Campus Pilot</span>
              <span className="block truncate text-[11px] text-[var(--sidebar-muted)]">{schoolName}</span>
            </span>
          </Link>
          <div className="ml-auto flex items-center gap-2">
            <ThemeToggle variant="sidebar" />
            <button
              aria-label="Sign out"
              className="inline-flex size-10 items-center justify-center rounded-[8px] border border-[var(--sidebar-border)] bg-white/5 text-[var(--sidebar-muted)] hover:bg-[var(--sidebar-hover)] hover:text-[var(--sidebar-foreground)] focus-visible:ring-[var(--brand-highlight)]"
              onClick={() => void handleLogout()}
              type="button"
            >
              <LogOut className="size-[18px]" />
            </button>
          </div>
        </div>
      </header>

      <main className="mx-auto max-w-[1480px] px-4 py-8 sm:px-6 sm:py-10 lg:px-8 lg:py-12" id="main-content" tabIndex={-1}>
        <section className="grid gap-8 lg:grid-cols-[minmax(0,1fr)_minmax(340px,0.48fr)] lg:items-end">
          <div>
            <div className="flex items-center gap-2 text-[11px] font-semibold uppercase tracking-[0.18em] text-[var(--brand-strong)]">
              <Grid2X2 className="size-3.5" />
              Campus modules
            </div>
            <h1 className="mt-4 max-w-[17ch] text-[clamp(2rem,4vw,3.75rem)] font-semibold leading-[1.02] tracking-[-0.055em] text-[var(--text-strong)]">
              {greeting()}, {firstName}.
            </h1>
            <p className="mt-4 max-w-[34em] text-base leading-7 text-[var(--text-muted)]">
              Choose where you need to work. What appears here follows your campus license and assigned roles.
            </p>
          </div>
          <div className="space-y-3">
            <div className="flex items-center gap-2 text-xs font-medium text-[var(--text-muted)]">
              <CalendarDays className="size-4 text-[var(--brand-strong)]" />
              {formattedDate()}
            </div>
            <label className="relative block" htmlFor="module-search">
              <span className="sr-only">Search modules</span>
              <Search className="pointer-events-none absolute left-4 top-1/2 size-[18px] -translate-y-1/2 text-[var(--text-muted)]" />
              <input
                className="h-12 w-full rounded-[var(--radius-lg)] border border-[var(--input-border)] bg-[var(--surface)] pl-11 pr-4 text-base text-[var(--text-strong)] shadow-[var(--shadow-rest)] placeholder:text-[var(--text-subtle)] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--focus-ring)] sm:text-sm"
                id="module-search"
                onChange={(event) => setSearchQuery(event.target.value)}
                placeholder="Find a module"
                type="search"
                value={searchQuery}
              />
            </label>
          </div>
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
          <section className="mt-12 py-14 text-center">
            <ShieldCheck className="mx-auto size-10 text-[var(--brand-strong)]" />
            <h2 className="mt-4 text-lg font-semibold text-[var(--text-strong)]">No modules are assigned yet</h2>
            <p className="mx-auto mt-2 max-w-[34em] text-sm leading-6 text-[var(--text-muted)]">
              Ask your campus administrator to assign a role with access to an enabled module.
            </p>
          </section>
        ) : null}

        {!isLoading && !loadError && recentModule && !searchQuery ? (
          <section className="mt-12" aria-labelledby="continue-heading">
            <div className="mb-3 flex items-baseline justify-between gap-4">
              <div>
                <p className="text-[11px] font-semibold uppercase tracking-[0.16em] text-[var(--text-muted)]">Continue</p>
                <h2 className="mt-1 text-xl font-semibold tracking-[-0.025em] text-[var(--text-strong)]" id="continue-heading">Back to your work</h2>
              </div>
              <span className="hidden text-xs text-[var(--text-subtle)] sm:block">Most recently opened</span>
            </div>
            <FeaturedModule module={recentModule} onOpen={handleModuleOpen} />
          </section>
        ) : null}

        {!isLoading && !loadError && filteredModules.length === 0 && searchQuery ? (
          <section className="mt-12 py-12 text-center">
            <Search className="mx-auto size-9 text-[var(--text-subtle)]" />
            <h2 className="mt-4 text-lg font-semibold text-[var(--text-strong)]">No modules match “{searchQuery}”</h2>
            <p className="mt-2 text-sm text-[var(--text-muted)]">Try a module name or school task.</p>
          </section>
        ) : null}

        <div className="mt-12 space-y-12">
          {Object.entries(groupedModules).map(([group, modules]) => (
            <section aria-labelledby={`module-group-${slug(group)}`} key={group}>
              <div className="mb-4 border-b border-[var(--border)] pb-3">
                <p className="text-[11px] font-semibold uppercase tracking-[0.16em] text-[var(--brand-strong)]">Your modules</p>
                <h2 className="mt-1 text-xl font-semibold tracking-[-0.025em] text-[var(--text-strong)]" id={`module-group-${slug(group)}`}>{group}</h2>
              </div>
              <div className="grid gap-x-8 md:grid-cols-2 xl:grid-cols-3">
                {modules.map((module) => <ModuleRow key={module.key} module={module} onOpen={handleModuleOpen} />)}
              </div>
            </section>
          ))}
        </div>
      </main>
    </div>
  );
};

const FeaturedModule: React.FC<{ module: ModuleDefinition; onOpen: (key: string) => void }> = ({ module, onOpen }) => {
  const visual = moduleVisuals[module.key] ?? defaultModuleVisual;
  const Icon = visual.icon;
  return (
    <ModuleDestination module={module} onOpen={onOpen}>
      <div className="group relative overflow-hidden rounded-[var(--radius-2xl)] bg-[var(--sidebar)] p-6 text-[var(--sidebar-foreground)] sm:p-8">
        <div aria-hidden="true" className="campus-grid-pattern absolute inset-0 opacity-35" />
        <div className="relative grid gap-8 md:grid-cols-[minmax(0,1fr)_minmax(240px,0.55fr)] md:items-end">
          <div className="flex items-start gap-4">
            <span className="flex size-12 shrink-0 items-center justify-center rounded-[11px] bg-[var(--brand-highlight)] text-[var(--sidebar-active-fg)]">
              <Icon className="size-5" />
            </span>
            <div>
              <div className="flex flex-wrap items-center gap-2">
                <span className="text-[11px] font-semibold uppercase tracking-[0.16em] text-[var(--brand-highlight)]">{module.group}</span>
                <span aria-hidden="true" className="size-1 rounded-full bg-[var(--brand-highlight)]" />
                <span className="text-[11px] font-medium text-[var(--on-brand-muted)]">{stageLabel(module.stage)}</span>
              </div>
              <h3 className="mt-3 text-2xl font-semibold tracking-[-0.035em]">{module.label}</h3>
              <p className="mt-2 max-w-xl text-sm leading-6 text-[var(--sidebar-muted)]">{module.description}</p>
            </div>
          </div>
          <div className="flex items-center justify-between gap-4 border-t border-[var(--sidebar-border)] pt-5 md:border-l md:border-t-0 md:pl-8 md:pt-0">
            <span className="text-sm font-semibold">Open workspace</span>
            <span className="flex size-10 items-center justify-center rounded-full bg-white/10 transition-transform duration-200 ease-[var(--motion-ease-default)] group-hover:translate-x-1">
              <ArrowRight className="size-[18px]" />
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
      <div className="group flex min-h-[132px] items-start gap-4 border-b border-[var(--border-subtle)] py-5">
        <span className="flex size-11 shrink-0 items-center justify-center rounded-[10px] bg-[var(--brand-soft)] text-[var(--brand-strong)] transition-colors group-hover:bg-[var(--brand)] group-hover:text-[var(--on-brand)]">
          <Icon className="size-[19px]" />
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
      params={{ moduleKey: module.key }}
      to="/modules/$moduleKey"
    >
      {children}
    </Link>
  );
};

const ModuleLauncherSkeleton = () => (
  <div aria-label="Loading campus modules" className="mt-12 space-y-8" role="status">
    <div className="h-44 animate-pulse rounded-[var(--radius-2xl)] bg-[var(--surface-sunken)]" />
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

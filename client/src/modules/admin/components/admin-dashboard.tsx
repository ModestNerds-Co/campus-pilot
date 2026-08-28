import React from "react";
import { Link } from "@tanstack/react-router";
import { ArrowRight, Bot, Grid2X2, KeyRound, Settings2, ShieldCheck, UsersRound, Waypoints } from "lucide-react";

import { useAuthStore } from "@/stores/auth-store";
import { usePageChrome } from "../layouts/page-chrome";

const administrationAreas = [
  { title: "Users", description: "Create accounts, manage status, and assign roles.", href: "/admin/users" as const, icon: UsersRound, action: "Manage users", permission: "users:view" },
  { title: "Roles and access", description: "Manage role permissions and create custom roles.", href: "/admin/roles" as const, icon: ShieldCheck, action: "Manage roles", permission: "roles:view" },
  { title: "Licensing", description: "Review installation licensing and module access.", href: "/admin/licensing" as const, icon: KeyRound, action: "Review licensing", permission: "licensing:view" },
  { title: "School settings", description: "Manage campus identity, academic defaults, notifications, and integrations.", href: "/admin/settings" as const, icon: Settings2, action: "Open settings", permission: "school_settings:view" },
  { title: "AI providers", description: "Connect and test the model providers available to Agent.", href: "/admin/agent/providers" as const, icon: Bot, action: "Manage providers", permission: "ai_providers:view", module: "agent" },
  { title: "Routing", description: "Set provider and model fallback order for Agent work.", href: "/admin/agent/routing" as const, icon: Waypoints, action: "Manage routes", permission: "ai_routing:view", module: "agent" },
];

export const AdminDashboard: React.FC = () => {
  const user = useAuthStore((state) => state.user);
  const visibleAreas = administrationAreas.filter(
    (area) =>
      (user?.permissions?.includes("*") || user?.permissions?.includes(area.permission)) &&
      (!("module" in area) || !area.module || user?.modules?.includes(area.module)),
  );
  usePageChrome("Overview");

  return (
    <div className="space-y-10">
      <section className="grid gap-8 lg:grid-cols-[minmax(0,1fr)_minmax(320px,0.48fr)] lg:items-end">
        <div>
          <p className="text-[11px] font-semibold uppercase tracking-[0.18em] text-[var(--brand-strong)]">Campus management</p>
          <h1 className="mt-3 max-w-[18ch] text-3xl font-semibold leading-[1.06] tracking-[-0.045em] text-[var(--text-strong)] sm:text-4xl">Administration</h1>
          <p className="mt-4 max-w-[34em] text-base leading-7 text-[var(--text-muted)]">Manage users, roles, module access, licensing, and school settings.</p>
        </div>
        <Link className="group flex items-center justify-between gap-5 bg-[var(--sidebar)] p-5 text-[var(--sidebar-foreground)]" to="/home">
          <span className="flex items-center gap-3">
            <span className="flex size-10 items-center justify-center rounded-[9px] bg-[var(--brand-highlight)] text-[var(--sidebar-active-fg)]"><Grid2X2 className="size-[18px]" /></span>
            <span><span className="block text-sm font-semibold">All modules</span><span className="mt-0.5 block text-xs text-[var(--sidebar-muted)]">Open module launcher</span></span>
          </span>
          <ArrowRight className="size-4 transition-transform group-hover:translate-x-1" />
        </Link>
      </section>

      <section aria-labelledby="admin-areas-heading">
        <div className="border-b border-[var(--border)] pb-3">
          <p className="text-[11px] font-semibold uppercase tracking-[0.16em] text-[var(--text-muted)]">Administration areas</p>
          <h2 className="mt-1 text-xl font-semibold tracking-[-0.025em] text-[var(--text-strong)]" id="admin-areas-heading">Choose what to manage</h2>
        </div>
        <div className="grid gap-x-8 md:grid-cols-2">
          {visibleAreas.map(({ action, description, href, icon: Icon, title }) => (
            <Link className="group flex min-h-[156px] items-start gap-4 border-b border-[var(--border-subtle)] py-6" key={title} to={href}>
              <span className="flex size-11 shrink-0 items-center justify-center rounded-[10px] bg-[var(--brand-soft)] text-[var(--brand-strong)] group-hover:bg-[var(--brand)] group-hover:text-[var(--on-brand)]"><Icon className="size-[19px]" /></span>
              <span className="min-w-0 flex-1">
                <span className="block text-base font-semibold text-[var(--text-strong)]">{title}</span>
                <span className="mt-1.5 block max-w-[34em] text-sm leading-6 text-[var(--text-muted)]">{description}</span>
                <span className="mt-3 inline-flex items-center gap-2 text-sm font-semibold text-[var(--brand-strong)]">{action}<ArrowRight className="size-4 transition-transform group-hover:translate-x-1" /></span>
              </span>
            </Link>
          ))}
        </div>
      </section>

    </div>
  );
};

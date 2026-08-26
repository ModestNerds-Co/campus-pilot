//
//  campus-pilot
//  admin-dashboard.tsx - Campus operations overview
//

import React from "react";
import { Link } from "@tanstack/react-router";
import {
  ArrowRight,
  BookOpen,
  Building2,
  CalendarDays,
  CheckCircle2,
  ClipboardCheck,
  GraduationCap,
  ShieldCheck,
  Truck,
  UserCheck,
  UsersRound,
} from "lucide-react";

import { useAuthStore } from "../../../stores/auth-store";
import { usePageChrome } from "../layouts/page-chrome";

const measures = [
  { label: "Students", value: "0", detail: "No roster imported", icon: GraduationCap },
  { label: "Staff", value: "0", detail: "Directory ready", icon: UserCheck },
  { label: "Departments", value: "0", detail: "Structure not configured", icon: Building2 },
  { label: "Active users", value: "1", detail: "Administrator access", icon: UsersRound },
];

const focusAreas = [
  {
    title: "Build the school structure",
    detail: "Define departments, grades and classes before enrolment begins.",
    href: "/admin/departments",
    icon: Building2,
    meta: "Academic foundation",
  },
  {
    title: "Prepare your people directory",
    detail: "Add staff and users, then assign the right level of access.",
    href: "/admin/users",
    icon: UsersRound,
    meta: "People & permissions",
  },
  {
    title: "Put daily operations in motion",
    detail: "Register vehicles and drivers before recording campus trips.",
    href: "/admin/fleet",
    icon: Truck,
    meta: "Fleet operations",
  },
];

const readiness = [
  { label: "Administrator account", complete: true },
  { label: "School structure", complete: false },
  { label: "Staff and user directory", complete: false },
  { label: "Student roster", complete: false },
];

export const AdminDashboard: React.FC = () => {
  const { user } = useAuthStore();
  const firstName = user?.full_name?.trim().split(/\s+/)[0] || "there";

  usePageChrome("Overview");

  return (
    <div className="space-y-8">
      <section className="flex flex-col gap-5 sm:flex-row sm:items-end sm:justify-between">
        <div>
          <p className="text-[11px] font-semibold uppercase tracking-[0.18em] text-[var(--brand-strong)]">Campus overview</p>
          <h1 className="mt-2 text-3xl font-semibold tracking-[-0.045em] text-[var(--text-strong)] sm:text-4xl">
            {greeting()}, {firstName}.
          </h1>
          <p className="mt-2 text-sm text-[var(--text-muted)]">Here is what needs attention across the school workspace.</p>
        </div>
        <div className="inline-flex w-fit items-center gap-2 rounded-[var(--radius-md)] border border-[var(--border)] bg-[var(--surface)] px-3 py-2 text-sm text-[var(--text-muted)]">
          <CalendarDays className="size-4 text-[var(--brand-strong)]" />
          <span>{formattedDate()}</span>
        </div>
      </section>

      <section className="relative overflow-hidden rounded-[var(--radius-2xl)] bg-[var(--sidebar)] p-6 text-[var(--sidebar-foreground)] sm:p-7">
        <div aria-hidden="true" className="campus-grid-pattern absolute inset-0 opacity-40" />
        <div className="relative flex flex-col gap-6 sm:flex-row sm:items-center sm:justify-between">
          <div className="flex max-w-2xl gap-4">
            <span className="flex size-11 shrink-0 items-center justify-center rounded-[10px] bg-[var(--brand-highlight)] text-[var(--sidebar-active-fg)]">
              <ClipboardCheck className="size-5" />
            </span>
            <div>
              <p className="text-[11px] font-semibold uppercase tracking-[0.16em] text-[var(--brand-highlight)]">Workspace readiness</p>
              <h2 className="mt-2 text-xl font-semibold tracking-[-0.025em]">The foundation is ready. Add the school structure next.</h2>
              <p className="mt-2 text-sm leading-6 text-[var(--sidebar-muted)]">
                Your administrator account is active. Departments, classes and people will turn this into a working campus record.
              </p>
            </div>
          </div>
          <Link
            className="inline-flex min-h-10 shrink-0 items-center justify-center gap-2 rounded-[8px] bg-[var(--brand-highlight)] px-4 text-sm font-semibold text-[var(--sidebar-active-fg)] hover:bg-[var(--brand-highlight-hover)] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-white"
            to="/admin/departments"
          >
            Start school setup
            <ArrowRight className="size-4" />
          </Link>
        </div>
      </section>

      <section aria-labelledby="snapshot-heading">
        <div className="mb-3 flex items-end justify-between gap-4">
          <div>
            <p className="text-[11px] font-semibold uppercase tracking-[0.16em] text-[var(--text-muted)]">Current snapshot</p>
            <h2 className="mt-1 text-lg font-semibold tracking-[-0.02em] text-[var(--text-strong)]" id="snapshot-heading">Campus records</h2>
          </div>
          <span className="text-xs text-[var(--text-subtle)]">Live workspace totals</span>
        </div>
        <div className="grid overflow-hidden rounded-[var(--radius-xl)] border border-[var(--border)] bg-[var(--surface)] sm:grid-cols-2 xl:grid-cols-4">
          {measures.map(({ detail, icon: Icon, label, value }, index) => (
            <div
              className={`p-5 ${index > 0 ? "border-t border-[var(--border-subtle)] sm:border-l" : ""} ${index === 2 ? "sm:border-l-0 xl:border-l" : ""} ${index > 1 ? "xl:border-t-0" : "sm:border-t-0"}`}
              key={label}
            >
              <div className="flex items-center justify-between gap-4">
                <span className="text-[11px] font-semibold uppercase tracking-[0.12em] text-[var(--text-muted)]">{label}</span>
                <Icon className="size-[18px] text-[var(--brand-strong)]" />
              </div>
              <p className="font-tabular mt-4 text-3xl font-semibold tracking-[-0.04em] text-[var(--text-strong)]">{value}</p>
              <p className="mt-2 text-xs text-[var(--text-subtle)]">{detail}</p>
            </div>
          ))}
        </div>
      </section>

      <div className="grid gap-8 xl:grid-cols-[minmax(0,1.35fr)_minmax(310px,0.65fr)]">
        <section aria-labelledby="focus-heading">
          <div className="border-b border-[var(--border)] pb-3">
            <p className="text-[11px] font-semibold uppercase tracking-[0.16em] text-[var(--text-muted)]">Operational focus</p>
            <h2 className="mt-1 text-lg font-semibold tracking-[-0.02em] text-[var(--text-strong)]" id="focus-heading">Set up the work in the right order</h2>
          </div>
          <div className="divide-y divide-[var(--border-subtle)]">
            {focusAreas.map(({ detail, href, icon: Icon, meta, title }, index) => (
              <Link className="group flex items-start gap-4 py-5" key={title} to={href}>
                <span className="font-tabular mt-1 text-xs font-semibold text-[var(--text-subtle)]">{String(index + 1).padStart(2, "0")}</span>
                <span className="flex size-10 shrink-0 items-center justify-center rounded-[9px] bg-[var(--brand-soft)] text-[var(--brand-strong)] transition-transform duration-200 ease-[var(--motion-ease-default)] group-hover:-translate-y-0.5">
                  <Icon className="size-[18px]" />
                </span>
                <span className="min-w-0 flex-1">
                  <span className="text-[11px] font-semibold uppercase tracking-[0.12em] text-[var(--text-subtle)]">{meta}</span>
                  <span className="mt-1 block text-sm font-semibold text-[var(--text-strong)]">{title}</span>
                  <span className="mt-1 block text-sm leading-5 text-[var(--text-muted)]">{detail}</span>
                </span>
                <ArrowRight className="mt-3 size-4 shrink-0 text-[var(--text-subtle)] transition-transform duration-200 group-hover:translate-x-1 group-hover:text-[var(--brand-strong)]" />
              </Link>
            ))}
          </div>
        </section>

        <aside className="rounded-[var(--radius-xl)] border border-[var(--border)] bg-[var(--surface)] p-5" aria-labelledby="readiness-heading">
          <div className="flex items-start justify-between gap-4">
            <div>
              <p className="text-[11px] font-semibold uppercase tracking-[0.16em] text-[var(--brand-strong)]">Getting started</p>
              <h2 className="mt-1 text-base font-semibold text-[var(--text-strong)]" id="readiness-heading">Workspace checklist</h2>
            </div>
            <span className="font-tabular rounded-full bg-[var(--brand-soft)] px-2.5 py-1 text-xs font-semibold text-[var(--brand-strong)]">1 of 4</span>
          </div>

          <div className="mt-5 h-1.5 overflow-hidden rounded-full bg-[var(--surface-sunken)]">
            <div className="h-full w-1/4 rounded-full bg-[var(--brand)]" />
          </div>

          <ul className="mt-5 space-y-3">
            {readiness.map(({ complete, label }) => (
              <li className="flex items-center gap-3" key={label}>
                {complete ? (
                  <CheckCircle2 className="size-[18px] shrink-0 text-[var(--tone-success)]" />
                ) : (
                  <span className="size-[18px] shrink-0 rounded-full border border-[var(--border-strong)] bg-[var(--surface-muted)]" />
                )}
                <span className={`text-sm ${complete ? "text-[var(--text-muted)] line-through" : "font-medium text-[var(--text-body)]"}`}>{label}</span>
              </li>
            ))}
          </ul>

          <div className="mt-6 flex items-center gap-3 border-t border-[var(--border-subtle)] pt-4 text-xs leading-5 text-[var(--text-muted)]">
            <ShieldCheck className="size-4 shrink-0 text-[var(--brand-strong)]" />
            Your account controls which campus records you can view and change.
          </div>
        </aside>
      </div>

      <section className="flex flex-col gap-4 rounded-[var(--radius-xl)] border border-dashed border-[var(--border-strong)] bg-[var(--surface-muted)] p-5 sm:flex-row sm:items-center sm:justify-between">
        <div className="flex items-start gap-3">
          <BookOpen className="mt-0.5 size-5 shrink-0 text-[var(--brand-strong)]" />
          <div>
            <h2 className="text-sm font-semibold text-[var(--text-strong)]">No recent campus activity yet</h2>
            <p className="mt-1 text-sm text-[var(--text-muted)]">Actions will appear here as teams begin working in Campus Pilot.</p>
          </div>
        </div>
        <Link className="inline-flex min-h-10 items-center gap-2 text-sm font-semibold text-[var(--brand-strong)] hover:underline" to="/admin/users">
          Manage access
          <ArrowRight className="size-4" />
        </Link>
      </section>
    </div>
  );
};

function greeting() {
  const hour = new Date().getHours();
  if (hour < 12) return "Good morning";
  if (hour < 18) return "Good afternoon";
  return "Good evening";
}

function formattedDate() {
  return new Intl.DateTimeFormat("en-ZA", {
    weekday: "short",
    day: "numeric",
    month: "short",
    year: "numeric",
  }).format(new Date());
}

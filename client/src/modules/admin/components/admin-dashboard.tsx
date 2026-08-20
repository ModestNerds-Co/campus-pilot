//
//  campus-pilot
//  admin-dashboard.tsx - Admin Dashboard Component (token-driven)
//

import React from "react";
import { useAuthStore } from "../../../stores/auth-store";
import { Card, CardHeader, CardTitle, CardContent } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { usePageChrome } from "@/modules/admin/layouts/page-chrome";
import {
  Users,
  Building2,
  GraduationCap,
  TrendingUp,
  TrendingDown,
  Calendar,
  UserCheck,
  BookOpen,
  Clock,
} from "lucide-react";

type Tone = "brand" | "success" | "accent" | "warn";

const toneMap: Record<Tone, { bg: string; fg: string }> = {
  brand: { bg: "bg-[var(--surface-sunken)]", fg: "text-[var(--brand)]" },
  success: { bg: "bg-[var(--surface-sunken)]", fg: "text-[var(--tone-success)]" },
  accent: { bg: "bg-[var(--surface-sunken)]", fg: "text-[var(--text-muted)]" },
  warn: { bg: "bg-[var(--surface-sunken)]", fg: "text-[var(--tone-warn)]" },
};

interface StatCardProps {
  title: string;
  value: string | number;
  icon: React.ComponentType<{ className?: string }>;
  trend?: { value: string; isPositive: boolean };
  tone?: Tone;
}

const StatCard: React.FC<StatCardProps> = ({ title, value, icon: Icon, trend, tone = "brand" }) => {
  const t = toneMap[tone];
  return (
    <div className="rounded-[12px] border border-[var(--border-subtle)] bg-[var(--surface-sunken)] p-[14px_16px]">
      <div className="flex items-center justify-between mb-3">
        <span className="text-[11px] font-medium uppercase tracking-wide text-[var(--text-muted)]">{title}</span>
        <div className={`flex size-8 items-center justify-center rounded-[8px] bg-[var(--surface)] border border-[var(--border-subtle)] ${t.fg}`}>
          <Icon className="size-4" />
        </div>
      </div>
      <div className="text-[18px] font-bold tracking-tight text-[var(--text-strong)] leading-none">{value}</div>
      {trend && (
        <div className="flex items-center gap-1 mt-2">
          {trend.isPositive ? <TrendingUp className="size-3.5 text-[var(--tone-success)]" /> : <TrendingDown className="size-3.5 text-[var(--tone-danger)]" />}
          <span className={`text-xs font-medium ${trend.isPositive ? "text-[var(--tone-success)]" : "text-[var(--tone-danger)]"}`}>{trend.value}</span>
          <span className="text-xs text-[var(--text-subtle)]">vs last period</span>
        </div>
      )}
    </div>
  );
};

export const AdminDashboard: React.FC = () => {
  const { user } = useAuthStore();

  usePageChrome("Dashboard");

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between">
        <p className="text-sm text-[var(--text-muted)]">Welcome back, {user?.full_name}</p>
        <div className="inline-flex items-center gap-2 rounded-[var(--radius-lg)] border border-[var(--border)] bg-[var(--surface)] px-3 py-2 text-sm text-[var(--text-muted)] shadow-sm">
          <Calendar className="size-4" />
          <span>
            {new Date().toLocaleDateString("en-US", {
              weekday: "short",
              year: "numeric",
              month: "short",
              day: "numeric",
            })}
          </span>
        </div>
      </div>

      {/* Stats Grid */}
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-6">
        <StatCard title="Total students" value="0" icon={GraduationCap} trend={{ value: "0%", isPositive: true }} tone="brand" />
        <StatCard title="Total staff" value="0" icon={UserCheck} trend={{ value: "0%", isPositive: true }} tone="success" />
        <StatCard title="Departments" value="0" icon={Building2} tone="accent" />
        <StatCard title="Active users" value="1" icon={Users} trend={{ value: "0%", isPositive: true }} tone="warn" />
      </div>

      {/* Activity Chart — Huchu outer card pattern */}
      <div className="overflow-hidden rounded-[16px] border border-[var(--border-subtle)] bg-[var(--surface)]" style={{ boxShadow: "0 1px 3px rgba(15,23,42,0.06), 0 4px 12px rgba(15,23,42,0.04)" }}>
        <div className="flex items-center justify-between border-b border-[var(--border-subtle)] bg-[var(--surface-muted)] px-5 py-3">
          <h2 className="text-[13px] font-semibold text-[var(--text-strong)]">Student enrollment trend</h2>
          <div className="flex items-center gap-1 rounded-full border border-[var(--border-subtle)] bg-[var(--surface)] p-1">
            <button className="rounded-full px-3 py-1 text-xs text-[var(--text-muted)] hover:text-[var(--text-strong)]">Week</button>
            <button className="rounded-full bg-[var(--text-strong)] px-3 py-1 text-xs font-medium text-[var(--text-inverse)]">Month</button>
            <button className="rounded-full px-3 py-1 text-xs text-[var(--text-muted)] hover:text-[var(--text-strong)]">Year</button>
          </div>
        </div>
        <div className="p-5">
          <div className="flex h-56 items-center justify-center rounded-[12px] border border-dashed border-[var(--border)] bg-[var(--surface-muted)]">
            <div className="text-center">
              <Clock className="mx-auto mb-3 size-10 text-[var(--text-subtle)]" />
              <p className="text-sm text-[var(--text-muted)]">Chart visualization coming soon</p>
              <p className="mt-1 text-xs text-[var(--text-subtle)]">Activity data will be displayed here</p>
            </div>
          </div>
        </div>
      </div>

      {/* Two Column — Huchu detail grid */}
      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        <div className="overflow-hidden rounded-[16px] border border-[var(--border-subtle)] bg-[var(--surface)]" style={{ boxShadow: "0 1px 3px rgba(15,23,42,0.06), 0 4px 12px rgba(15,23,42,0.04)" }}>
          <div className="border-b border-[var(--border-subtle)] bg-[var(--surface-muted)] px-5 py-3">
            <h2 className="text-[13px] font-semibold text-[var(--text-strong)]">Recent activity</h2>
          </div>
          <div className="flex flex-col items-center justify-center gap-2 p-8 text-center">
            <BookOpen className="mx-auto mb-1 size-10 text-[var(--text-subtle)]" />
            <p className="text-sm text-[var(--text-muted)]">No recent activity</p>
            <p className="mt-1 text-xs text-[var(--text-subtle)]">Activity will appear here as you use the system</p>
          </div>
        </div>

        <div className="overflow-hidden rounded-[16px] border border-[var(--border-subtle)] bg-[var(--surface)]" style={{ boxShadow: "0 1px 3px rgba(15,23,42,0.06), 0 4px 12px rgba(15,23,42,0.04)" }}>
          <div className="border-b border-[var(--border-subtle)] bg-[var(--surface-muted)] px-5 py-3">
            <h2 className="text-[13px] font-semibold text-[var(--text-strong)]">Quick actions</h2>
          </div>
          <div className="space-y-2 p-5">
            <button className="flex w-full items-center gap-3 rounded-[10px] border border-[var(--border-subtle)] bg-[var(--surface)] px-4 py-3 text-left text-[13px] font-medium text-[var(--text-strong)] hover:bg-[var(--surface-muted)]">
              <span className="flex size-8 items-center justify-center rounded-[8px] bg-[var(--brand-soft)] text-[var(--brand)]"><Users className="size-4" /></span>
              Add new user
            </button>
            <button className="flex w-full items-center gap-3 rounded-[10px] border border-[var(--border-subtle)] bg-[var(--surface)] px-4 py-3 text-left text-[13px] font-medium text-[var(--text-strong)] hover:bg-[var(--surface-muted)]">
              <span className="flex size-8 items-center justify-center rounded-[8px] bg-[var(--surface-sunken)] text-[var(--text-muted)]"><Building2 className="size-4" /></span>
              Create department
            </button>
            <button className="flex w-full items-center gap-3 rounded-[10px] border border-[var(--border-subtle)] bg-[var(--surface)] px-4 py-3 text-left text-[13px] font-medium text-[var(--text-strong)] hover:bg-[var(--surface-muted)]">
              <span className="flex size-8 items-center justify-center rounded-[8px] bg-[var(--surface-sunken)] text-[var(--text-muted)]"><GraduationCap className="size-4" /></span>
              Enroll student
            </button>
            <button className="flex w-full items-center gap-3 rounded-[10px] border border-[var(--border-subtle)] bg-[var(--surface)] px-4 py-3 text-left text-[13px] font-medium text-[var(--text-strong)] hover:bg-[var(--surface-muted)]">
              <span className="flex size-8 items-center justify-center rounded-[8px] bg-[var(--surface-sunken)] text-[var(--text-muted)]"><BookOpen className="size-4" /></span>
              Create subject
            </button>
          </div>
        </div>
      </div>

      {/* Getting Started — Huchu banner style */}
      <div className="rounded-[16px] border border-[var(--border-subtle)] bg-[var(--surface-muted)] p-6">
        <h2 className="mb-4 text-[13px] font-semibold uppercase tracking-wide text-[var(--text-muted)]">Getting started</h2>
        <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
          {[
            { n: 1, title: "Set up school structure", desc: "Create departments and classes" },
            { n: 2, title: "Add staff members", desc: "Create employee records" },
            { n: 3, title: "Enroll students", desc: "Start adding students" },
          ].map((s) => (
            <div key={s.n} className="flex gap-3">
              <div className="flex size-8 shrink-0 items-center justify-center rounded-full bg-[var(--brand)] text-sm font-bold text-[var(--on-brand)]">
                {s.n}
              </div>
              <div>
                <p className="text-sm font-medium text-[var(--brand-strong)]">{s.title}</p>
                <p className="mt-1 text-sm text-[var(--brand-strong)]/80">{s.desc}</p>
              </div>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
};

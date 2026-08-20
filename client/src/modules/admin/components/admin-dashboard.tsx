//
//  campus-pilot
//  admin-dashboard.tsx - Admin Dashboard Component (token-driven)
//

import React from "react";
import { useAuthStore } from "../../../stores/auth-store";
import { Card, CardHeader, CardTitle, CardContent } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
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
  brand: { bg: "bg-[var(--brand-soft)]", fg: "text-[var(--brand)]" },
  success: { bg: "bg-[var(--tone-success-bg)]", fg: "text-[var(--tone-success)]" },
  accent: { bg: "bg-[var(--accent-100)]", fg: "text-[var(--accent-700)]" },
  warn: { bg: "bg-[var(--tone-warn-bg)]", fg: "text-[var(--tone-warn)]" },
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
    <Card className="p-6">
      <div className="flex items-center justify-between mb-4">
        <span className="text-sm font-medium text-[var(--text-muted)]">{title}</span>
        <div className={`flex size-9 items-center justify-center rounded-[var(--radius-lg)] ${t.bg}`}>
          <Icon className={`size-5 ${t.fg}`} />
        </div>
      </div>
      <div className="flex items-end justify-between">
        <div>
          <div className="text-2xl font-semibold tracking-tight text-[var(--text-strong)] mb-1">
            {value}
          </div>
          {trend && (
            <div className="flex items-center gap-1">
              {trend.isPositive ? (
                <TrendingUp className="size-4 text-[var(--tone-success)]" />
              ) : (
                <TrendingDown className="size-4 text-[var(--tone-danger)]" />
              )}
              <span
                className={`text-sm font-medium ${trend.isPositive ? "text-[var(--tone-success)]" : "text-[var(--tone-danger)]"}`}
              >
                {trend.value}
              </span>
            </div>
          )}
        </div>
      </div>
    </Card>
  );
};

export const AdminDashboard: React.FC = () => {
  const { user } = useAuthStore();

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between">
        <div>
          <h1 className="text-[22px] font-semibold leading-tight text-[var(--text-strong)]">Dashboard</h1>
          <p className="mt-1 text-sm text-[var(--text-muted)]">Welcome back, {user?.full_name}</p>
        </div>
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

      {/* Activity Chart */}
      <Card className="p-6">
        <div className="flex items-center justify-between mb-6">
          <h2 className="text-base font-semibold text-[var(--text-strong)]">Student Enrollment Trend</h2>
          <div className="flex items-center gap-1 rounded-full border border-[var(--border)] bg-[var(--surface-muted)] p-1">
            <button className="rounded-full px-3 py-1 text-sm text-[var(--text-muted)] hover:text-[var(--text-strong)]">Week</button>
            <button className="rounded-full bg-[var(--surface)] px-3 py-1 text-sm font-medium text-[var(--text-strong)] shadow-sm border border-[var(--border)]">Month</button>
            <button className="rounded-full px-3 py-1 text-sm text-[var(--text-muted)] hover:text-[var(--text-strong)]">Year</button>
          </div>
        </div>
        <div className="flex h-64 items-center justify-center rounded-[var(--radius-lg)] border border-dashed border-[var(--border)] bg-[var(--surface-muted)]">
          <div className="text-center">
            <Clock className="mx-auto mb-3 size-12 text-[var(--text-subtle)]" />
            <p className="text-sm text-[var(--text-muted)]">Chart visualization coming soon</p>
            <p className="mt-1 text-sm text-[var(--text-subtle)]">Activity data will be displayed here</p>
          </div>
        </div>
      </Card>

      {/* Two Column Layout */}
      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        <Card className="p-6">
          <h2 className="mb-4 text-base font-semibold text-[var(--text-strong)]">Recent Activity</h2>
          <div className="flex flex-col items-center justify-center gap-2 py-8 text-center">
            <BookOpen className="mx-auto mb-1 size-12 text-[var(--text-subtle)]" />
            <p className="text-sm text-[var(--text-muted)]">No recent activity</p>
            <p className="mt-1 text-sm text-[var(--text-subtle)]">Activity will appear here as you use the system</p>
          </div>
        </Card>

        <Card className="p-6">
          <h2 className="mb-4 text-base font-semibold text-[var(--text-strong)]">Quick Actions</h2>
          <div className="space-y-3">
            <button className="flex w-full items-center gap-3 rounded-[var(--radius-lg)] border border-[var(--brand-100)] bg-[var(--brand-soft)] px-4 py-3 text-left text-sm font-medium text-[var(--brand-strong)] hover:bg-[var(--brand-100)]">
              <Users className="size-5" />
              Add New User
            </button>
            <button className="flex w-full items-center gap-3 rounded-[var(--radius-lg)] border border-[var(--tone-success-bd)] bg-[var(--tone-success-bg)] px-4 py-3 text-left text-sm font-medium text-[var(--tone-success-strong)] hover:brightness-95">
              <Building2 className="size-5" />
              Create Department
            </button>
            <button className="flex w-full items-center gap-3 rounded-[var(--radius-lg)] border border-[var(--border)] bg-[var(--surface-muted)] px-4 py-3 text-left text-sm font-medium text-[var(--text-strong)] hover:bg-[var(--surface-sunken)]">
              <GraduationCap className="size-5" />
              Enroll Student
            </button>
            <button className="flex w-full items-center gap-3 rounded-[var(--radius-lg)] border border-[var(--border)] bg-[var(--surface)] px-4 py-3 text-left text-sm font-medium text-[var(--text-strong)] hover:bg-[var(--surface-muted)]">
              <BookOpen className="size-5" />
              Create Subject
            </button>
          </div>
        </Card>
      </div>

      {/* Getting Started Section */}
      <Card className="border-[var(--brand-100)] bg-[var(--brand-soft)] p-6">
        <h2 className="mb-4 text-base font-semibold text-[var(--brand-strong)]">Getting Started</h2>
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
      </Card>
    </div>
  );
};

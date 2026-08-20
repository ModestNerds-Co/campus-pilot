//
//  campus-pilot
//  admin-layout.tsx - Admin Layout with Sidebar
//  Slice 1: token-driven shell (design system v1.0)
//
//  Created by Ngonidzashe Mangudya on 02/10/2025.
//  Copyright (c) 2025 Codecraft Solutions
//

import React, { useState } from "react";
import { Link, useNavigate, useLocation } from "@tanstack/react-router";
import { useAuthStore } from "../../../stores/auth-store";
import {
  LayoutDashboard,
  Users,
  Building2,
  GraduationCap,
  BookOpen,
  Settings,
  LogOut,
  Menu,
  X,
  ChevronDown,
  User,
} from "lucide-react";
import { ThemeToggle } from "../../../lib/theme";
import toast from "react-hot-toast";

interface AdminLayoutProps {
  children: React.ReactNode;
}

interface NavItem {
  label: string;
  icon: React.ComponentType<{ className?: string }>;
  href?: string;
  children?: {
    label: string;
    href: string;
  }[];
}

const navigationItems: NavItem[] = [
  {
    label: "Dashboard",
    icon: LayoutDashboard,
    href: "/admin",
  },
  {
    label: "User Management",
    icon: Users,
    children: [
      { label: "Users", href: "/admin/users" },
      { label: "Roles", href: "/admin/roles" },
    ],
  },
  {
    label: "School Structure",
    icon: Building2,
    children: [
      { label: "Departments", href: "/admin/departments" },
      { label: "Grades & Classes", href: "/admin/classes" },
      { label: "Staff", href: "/admin/staff" },
    ],
  },
  {
    label: "Students",
    icon: GraduationCap,
    href: "/admin/students",
  },
  {
    label: "Subjects",
    icon: BookOpen,
    href: "/admin/subjects",
  },
  {
    label: "Settings",
    icon: Settings,
    href: "/admin/settings",
  },
];

export const AdminLayout: React.FC<AdminLayoutProps> = ({ children }) => {
  const navigate = useNavigate();
  const location = useLocation();
  const { user, logout } = useAuthStore();
  const [sidebarOpen, setSidebarOpen] = useState(false);
  const [expandedItems, setExpandedItems] = useState<string[]>([]);

  const handleLogout = async () => {
    await logout();
    toast.success("Logged out successfully");
    navigate({ to: "/login" });
  };

  const toggleExpanded = (label: string) => {
    setExpandedItems((prev) =>
      prev.includes(label)
        ? prev.filter((item) => item !== label)
        : [...prev, label],
    );
  };

  const isActive = (href: string) => {
    return location.pathname === href;
  };

  const isParentActive = (item: NavItem) => {
    if (item.href) {
      return isActive(item.href);
    }
    if (item.children) {
      return item.children.some((child) => isActive(child.href));
    }
    return false;
  };

  return (
    <div className="min-h-screen bg-[var(--canvas)]">
      {/* Top Navigation Bar */}
      <nav
        className="fixed top-0 left-0 right-0 z-[var(--z-nav)] bg-[var(--surface)] border-b border-[var(--border)]"
        style={{
          height: "var(--app-bar-h)",
          boxShadow: "var(--chrome-shadow)",
        }}
      >
        <div className="px-4 sm:px-6 lg:px-8 h-full">
          <div className="flex justify-between h-full items-center">
            <div className="flex items-center gap-3">
              <button
                onClick={() => setSidebarOpen(!sidebarOpen)}
                aria-label={sidebarOpen ? "Close navigation" : "Open navigation"}
                aria-expanded={sidebarOpen}
                className="lg:hidden inline-flex items-center justify-center h-9 w-9 rounded-[var(--radius-md)] border border-transparent text-[var(--text-body)] hover:bg-[var(--surface-muted)] hover:text-[var(--text-strong)] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--focus-ring)] focus-visible:ring-offset-2 transition-colors"
              >
                {sidebarOpen ? (
                  <X className="w-5 h-5" />
                ) : (
                  <Menu className="w-5 h-5" />
                )}
              </button>
              <div className="flex items-center gap-3">
                <img
                  src="/assets/images/campus-pilot-logo.svg"
                  alt="CampusPilot"
                  className="h-7"
                />
                <span className="text-[15px] font-semibold tracking-tight text-[var(--text-strong)] hidden sm:block">
                  Admin
                </span>
              </div>
            </div>

            <div className="flex items-center gap-2 sm:gap-3">
              <ThemeToggle />

              <div className="hidden sm:flex items-center gap-3 pl-3 pr-3 py-1.5 rounded-full bg-[var(--surface-muted)] border border-[var(--border-subtle)]">
                <span className="inline-flex items-center justify-center h-7 w-7 rounded-full bg-[var(--surface)] border border-[var(--border)]">
                  <User className="w-4 h-4 text-[var(--text-muted)]" />
                </span>
                <div className="pr-1">
                  <p className="text-[13px] font-medium leading-none text-[var(--text-strong)]">
                    {user?.full_name}
                  </p>
                  <p className="text-[11px] leading-none text-[var(--text-muted)] mt-0.5">
                    {user?.roles[0]}
                  </p>
                </div>
              </div>

              {/* Mobile user avatar */}
              <div className="sm:hidden inline-flex items-center justify-center h-8 w-8 rounded-full bg-[var(--surface-muted)] border border-[var(--border)]">
                <User className="w-4 h-4 text-[var(--text-muted)]" />
              </div>

              <button
                onClick={handleLogout}
                aria-label="Log out"
                title="Log out"
                className="inline-flex items-center justify-center h-9 w-9 rounded-[var(--radius-md)] border border-transparent text-[var(--text-muted)] hover:bg-[var(--tone-danger-bg)] hover:text-[var(--tone-danger)] hover:border-[var(--tone-danger-bd)] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--focus-ring)] focus-visible:ring-offset-2 transition-colors"
              >
                <LogOut className="w-4 h-4" />
              </button>
            </div>
          </div>
        </div>
      </nav>

      {/* Sidebar */}
      <aside
        aria-label="Admin navigation"
        className={`cp-sidebar-desktop fixed left-0 bottom-0 z-[var(--z-sidebar)] w-64 bg-[var(--surface)] border-r border-[var(--border)] transform transition-transform duration-200 ease-[var(--motion-ease-default)] lg:translate-x-0 ${
          sidebarOpen ? "translate-x-0" : "-translate-x-full"
        }`}
        style={{ top: "var(--app-bar-h)" }}
      >
        <div className="h-full overflow-y-auto px-3 py-5">
          <nav className="space-y-1" aria-label="Primary">
            {navigationItems.map((item) => {
              const Icon = item.icon;
              const isItemActive = isParentActive(item);
              const isExpanded = expandedItems.includes(item.label);

              if (item.children) {
                return (
                  <div key={item.label}>
                    <button
                      onClick={() => toggleExpanded(item.label)}
                      aria-expanded={isExpanded}
                      className={`w-full flex items-center justify-between px-3 py-2 rounded-[var(--radius-md)] text-[13px] font-medium transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--focus-ring)] focus-visible:ring-offset-2 ${
                        isItemActive
                          ? "bg-[var(--surface-muted)] text-[var(--text-strong)]"
                          : "text-[var(--text-body)] hover:bg-[var(--surface-muted)] hover:text-[var(--text-strong)]"
                      }`}
                    >
                      <span className="flex items-center gap-2.5">
                        <Icon className="w-[18px] h-[18px] shrink-0" />
                        <span>{item.label}</span>
                      </span>
                      <ChevronDown
                        className={`w-4 h-4 shrink-0 text-[var(--text-muted)] transition-transform duration-200 ${
                          isExpanded ? "rotate-180" : ""
                        }`}
                      />
                    </button>
                    {isExpanded && (
                      <div className="mt-1 ml-3 pl-6 space-y-1 border-l border-[var(--border-subtle)]">
                        {item.children.map((child) => (
                          <Link
                            key={child.href}
                            to={child.href}
                            onClick={() => setSidebarOpen(false)}
                            className={`block px-3 py-1.5 rounded-[var(--radius-md)] text-[13px] transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--focus-ring)] ${
                              isActive(child.href)
                                ? "bg-[var(--surface-muted)] text-[var(--text-strong)] font-medium"
                                : "text-[var(--text-muted)] hover:bg-[var(--surface-muted)] hover:text-[var(--text-body)]"
                            }`}
                          >
                            {child.label}
                          </Link>
                        ))}
                      </div>
                    )}
                  </div>
                );
              }

              return (
                <Link
                  key={item.href}
                  to={item.href!}
                  onClick={() => setSidebarOpen(false)}
                  className={`flex items-center gap-2.5 px-3 py-2 rounded-[var(--radius-md)] text-[13px] font-medium transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--focus-ring)] focus-visible:ring-offset-2 ${
                    isItemActive
                      ? "bg-[var(--surface-muted)] text-[var(--text-strong)]"
                      : "text-[var(--text-body)] hover:bg-[var(--surface-muted)] hover:text-[var(--text-strong)]"
                  }`}
                >
                  <Icon className="w-[18px] h-[18px] shrink-0" />
                  <span>{item.label}</span>
                </Link>
              );
            })}
          </nav>
        </div>
      </aside>

      {/* Mobile sidebar overlay */}
      {sidebarOpen && (
        <div
          className="fixed inset-0 z-20 bg-[var(--surface-overlay)] backdrop-blur-[2px] lg:hidden"
          style={{ top: "var(--app-bar-h)" }}
          onClick={() => setSidebarOpen(false)}
          aria-hidden="true"
        />
      )}

      {/* Main Content */}
      <main
        className="pt-[var(--app-bar-h)] lg:pl-64"
        style={{ minHeight: "100dvh" }}
      >
        <div className="mx-auto max-w-[1280px] p-4 sm:p-6 lg:p-8">{children}</div>
      </main>
    </div>
  );
};

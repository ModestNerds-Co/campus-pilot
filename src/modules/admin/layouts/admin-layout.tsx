//
//  campus-pilot
//  admin-layout.tsx - Admin Layout with Sidebar
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
  Shield,
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
    <div className="min-h-screen bg-gray-50 dark:bg-gray-900">
      {/* Top Navigation Bar */}
      <nav className="fixed top-0 left-0 right-0 z-40 bg-white dark:bg-gray-800 border-b border-gray-200 dark:border-gray-700">
        <div className="px-4 sm:px-6 lg:px-8">
          <div className="flex justify-between h-16">
            <div className="flex items-center">
              <button
                onClick={() => setSidebarOpen(!sidebarOpen)}
                className="lg:hidden p-2 rounded-lg text-gray-600 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-700"
              >
                {sidebarOpen ? (
                  <X className="w-6 h-6" />
                ) : (
                  <Menu className="w-6 h-6" />
                )}
              </button>
              <div className="ml-4 lg:ml-0 flex items-center gap-3">
                <img
                  src="/assets/images/campus-pilot-logo.svg"
                  alt="CampusPilot"
                  className="h-8"
                />
                <span className="text-xl font-bold text-gray-900 dark:text-white hidden sm:block">
                  Admin
                </span>
              </div>
            </div>

            <div className="flex items-center gap-4">
              <ThemeToggle />

              <div className="flex items-center gap-3 px-3 py-2 rounded-lg bg-gray-100 dark:bg-gray-700">
                <User className="w-5 h-5 text-gray-600 dark:text-gray-300" />
                <div className="hidden sm:block">
                  <p className="text-sm font-medium text-gray-900 dark:text-white">
                    {user?.full_name}
                  </p>
                  <p className="text-xs text-gray-500 dark:text-gray-400">
                    {user?.roles[0]}
                  </p>
                </div>
              </div>

              <button
                onClick={handleLogout}
                className="p-2 rounded-lg text-red-600 hover:bg-red-50 dark:text-red-400 dark:hover:bg-red-900/20 transition-colors"
                title="Logout"
              >
                <LogOut className="w-5 h-5" />
              </button>
            </div>
          </div>
        </div>
      </nav>

      {/* Sidebar */}
      <aside
        className={`fixed top-16 left-0 bottom-0 z-30 w-64 bg-white dark:bg-gray-800 border-r border-gray-200 dark:border-gray-700 transform transition-transform duration-300 lg:translate-x-0 ${
          sidebarOpen ? "translate-x-0" : "-translate-x-full"
        }`}
      >
        <div className="h-full overflow-y-auto px-4 py-6">
          <nav className="space-y-2">
            {navigationItems.map((item) => {
              const Icon = item.icon;
              const isItemActive = isParentActive(item);
              const isExpanded = expandedItems.includes(item.label);

              if (item.children) {
                return (
                  <div key={item.label}>
                    <button
                      onClick={() => toggleExpanded(item.label)}
                      className={`w-full flex items-center justify-between px-3 py-2 rounded-lg transition-colors ${
                        isItemActive
                          ? "bg-blue-50 text-blue-600 dark:bg-blue-900/20 dark:text-blue-400"
                          : "text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-700"
                      }`}
                    >
                      <div className="flex items-center gap-3">
                        <Icon className="w-5 h-5" />
                        <span className="font-medium">{item.label}</span>
                      </div>
                      <ChevronDown
                        className={`w-4 h-4 transition-transform ${
                          isExpanded ? "rotate-180" : ""
                        }`}
                      />
                    </button>
                    {isExpanded && (
                      <div className="mt-1 ml-8 space-y-1">
                        {item.children.map((child) => (
                          <Link
                            key={child.href}
                            to={child.href}
                            className={`block px-3 py-2 rounded-lg text-sm transition-colors ${
                              isActive(child.href)
                                ? "bg-blue-50 text-blue-600 dark:bg-blue-900/20 dark:text-blue-400 font-medium"
                                : "text-gray-600 dark:text-gray-400 hover:bg-gray-100 dark:hover:bg-gray-700"
                            }`}
                            onClick={() => setSidebarOpen(false)}
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
                  className={`flex items-center gap-3 px-3 py-2 rounded-lg transition-colors ${
                    isItemActive
                      ? "bg-blue-50 text-blue-600 dark:bg-blue-900/20 dark:text-blue-400 font-medium"
                      : "text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-700"
                  }`}
                  onClick={() => setSidebarOpen(false)}
                >
                  <Icon className="w-5 h-5" />
                  <span className="font-medium">{item.label}</span>
                </Link>
              );
            })}
          </nav>
        </div>
      </aside>

      {/* Mobile sidebar overlay */}
      {sidebarOpen && (
        <div
          className="fixed inset-0 z-20 bg-black/50 lg:hidden"
          onClick={() => setSidebarOpen(false)}
        />
      )}

      {/* Main Content */}
      <main className="pt-16 lg:pl-64">
        <div className="p-4 sm:p-6 lg:p-8">{children}</div>
      </main>
    </div>
  );
};

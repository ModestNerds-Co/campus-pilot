//
//  campus-pilot
//  dashboard.tsx - Dashboard Route
//
//  Created by Ngonidzashe Mangudya on 02/10/2025.
//  Copyright (c) 2025 Codecraft Solutions
//

import { createFileRoute } from "@tanstack/react-router";
import { ProtectedRoute } from "../components/protected-route";
import { useAuthStore } from "../stores/auth-store";
import { useNavigate } from "@tanstack/react-router";
import { LogOut, User } from "lucide-react";
import toast from "react-hot-toast";

function DashboardComponent() {
  const { user, logout } = useAuthStore();
  const navigate = useNavigate();

  const handleLogout = async () => {
    await logout();
    toast.success("Logged out successfully");
    navigate({ to: "/login" });
  };

  return (
    <div className="min-h-screen bg-gray-50 dark:bg-gray-900">
      <nav className="bg-white dark:bg-gray-800 border-b border-gray-200 dark:border-gray-700">
        <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8">
          <div className="flex justify-between h-16">
            <div className="flex items-center">
              <h1 className="text-xl font-bold text-gray-900 dark:text-white">
                CampusPilot Dashboard
              </h1>
            </div>
            <div className="flex items-center gap-4">
              <div className="flex items-center gap-2 text-sm text-gray-600 dark:text-gray-300">
                <User className="w-4 h-4" />
                <span>{user?.full_name}</span>
              </div>
              <button
                onClick={handleLogout}
                className="flex items-center gap-2 px-4 py-2 text-sm text-red-600 hover:bg-red-50 dark:text-red-400 dark:hover:bg-red-900/20 rounded-lg transition-colors"
              >
                <LogOut className="w-4 h-4" />
                Logout
              </button>
            </div>
          </div>
        </div>
      </nav>

      <main className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-8">
        <div className="bg-white dark:bg-gray-800 rounded-xl border border-gray-200 dark:border-gray-700 p-8">
          <h2 className="text-2xl font-bold text-gray-900 dark:text-white mb-4">
            Welcome to CampusPilot!
          </h2>
          <p className="text-gray-600 dark:text-gray-300 mb-6">
            You are now logged in as{" "}
            <span className="font-semibold">{user?.email}</span>
          </p>

          <div className="space-y-4">
            <div className="bg-blue-50 dark:bg-blue-900/20 border border-blue-200 dark:border-blue-800 rounded-lg p-4">
              <h3 className="text-sm font-medium text-blue-800 dark:text-blue-300 mb-2">
                User Information
              </h3>
              <ul className="text-sm text-blue-700 dark:text-blue-300 space-y-1">
                <li>
                  <strong>Email:</strong> {user?.email}
                </li>
                <li>
                  <strong>Full Name:</strong> {user?.full_name}
                </li>
                <li>
                  <strong>Phone:</strong> {user?.phone || "N/A"}
                </li>
                <li>
                  <strong>Roles:</strong> {user?.roles.join(", ")}
                </li>
                <li>
                  <strong>Active:</strong> {user?.is_active ? "Yes" : "No"}
                </li>
              </ul>
            </div>

            <div className="bg-green-50 dark:bg-green-900/20 border border-green-200 dark:border-green-800 rounded-lg p-4">
              <h3 className="text-sm font-medium text-green-800 dark:text-green-300 mb-2">
                Next Steps
              </h3>
              <p className="text-sm text-green-700 dark:text-green-300">
                The admin UI is ready to be built. Check the ROADMAP.md for the
                next features to implement.
              </p>
            </div>
          </div>
        </div>
      </main>
    </div>
  );
}

export const Route = createFileRoute("/dashboard")({
  component: () => (
    <ProtectedRoute>
      <DashboardComponent />
    </ProtectedRoute>
  ),
});

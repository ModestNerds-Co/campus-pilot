//
//  campus-pilot
//  admin-dashboard.tsx - Admin Dashboard Component
//
//  Created by Ngonidzashe Mangudya on 02/10/2025.
//  Copyright (c) 2025 Codecraft Solutions
//

import React from "react";
import { useAuthStore } from "../../../stores/auth-store";
import {
  Users,
  Shield,
  Building2,
  GraduationCap,
  TrendingUp,
  Activity,
} from "lucide-react";

interface StatCardProps {
  title: string;
  value: string | number;
  icon: React.ComponentType<{ className?: string }>;
  trend?: {
    value: string;
    isPositive: boolean;
  };
  color: "blue" | "green" | "purple" | "orange";
}

const StatCard: React.FC<StatCardProps> = ({
  title,
  value,
  icon: Icon,
  trend,
  color,
}) => {
  const colorClasses = {
    blue: "bg-blue-50 text-blue-600 dark:bg-blue-900/20 dark:text-blue-400",
    green:
      "bg-green-50 text-green-600 dark:bg-green-900/20 dark:text-green-400",
    purple:
      "bg-purple-50 text-purple-600 dark:bg-purple-900/20 dark:text-purple-400",
    orange:
      "bg-orange-50 text-orange-600 dark:bg-orange-900/20 dark:text-orange-400",
  };

  return (
    <div className="bg-white dark:bg-gray-800 rounded-xl border border-gray-200 dark:border-gray-700 p-6">
      <div className="flex items-start justify-between">
        <div>
          <p className="text-sm font-medium text-gray-600 dark:text-gray-400">
            {title}
          </p>
          <p className="mt-2 text-3xl font-bold text-gray-900 dark:text-white">
            {value}
          </p>
          {trend && (
            <div className="mt-2 flex items-center gap-1">
              <TrendingUp
                className={`w-4 h-4 ${trend.isPositive ? "text-green-600" : "text-red-600"}`}
              />
              <span
                className={`text-sm font-medium ${trend.isPositive ? "text-green-600" : "text-red-600"}`}
              >
                {trend.value}
              </span>
              <span className="text-sm text-gray-500 dark:text-gray-400">
                vs last month
              </span>
            </div>
          )}
        </div>
        <div className={`p-3 rounded-lg ${colorClasses[color]}`}>
          <Icon className="w-6 h-6" />
        </div>
      </div>
    </div>
  );
};

export const AdminDashboard: React.FC = () => {
  const { user } = useAuthStore();

  return (
    <div className="space-y-6">
      {/* Welcome Section */}
      <div className="bg-gradient-to-r from-blue-600 to-blue-700 dark:from-blue-700 dark:to-blue-800 rounded-xl p-6 text-white">
        <h1 className="text-2xl font-bold mb-2">
          Welcome back, {user?.full_name}!
        </h1>
        <p className="text-blue-100">
          Here's what's happening with your school today.
        </p>
      </div>

      {/* Stats Grid */}
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-6">
        <StatCard
          title="Total Users"
          value="0"
          icon={Users}
          trend={{ value: "+0%", isPositive: true }}
          color="blue"
        />
        <StatCard
          title="Active Roles"
          value="1"
          icon={Shield}
          color="green"
        />
        <StatCard
          title="Departments"
          value="0"
          icon={Building2}
          color="purple"
        />
        <StatCard
          title="Students"
          value="0"
          icon={GraduationCap}
          trend={{ value: "+0%", isPositive: true }}
          color="orange"
        />
      </div>

      {/* Quick Actions */}
      <div className="bg-white dark:bg-gray-800 rounded-xl border border-gray-200 dark:border-gray-700 p-6">
        <h2 className="text-lg font-bold text-gray-900 dark:text-white mb-4">
          Quick Actions
        </h2>
        <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-4">
          <button className="flex items-center gap-3 px-4 py-3 bg-blue-50 dark:bg-blue-900/20 text-blue-700 dark:text-blue-400 rounded-lg hover:bg-blue-100 dark:hover:bg-blue-900/30 transition-colors">
            <Users className="w-5 h-5" />
            <span className="font-medium">Add New User</span>
          </button>
          <button className="flex items-center gap-3 px-4 py-3 bg-green-50 dark:bg-green-900/20 text-green-700 dark:text-green-400 rounded-lg hover:bg-green-100 dark:hover:bg-green-900/30 transition-colors">
            <Building2 className="w-5 h-5" />
            <span className="font-medium">Create Department</span>
          </button>
          <button className="flex items-center gap-3 px-4 py-3 bg-purple-50 dark:bg-purple-900/20 text-purple-700 dark:text-purple-400 rounded-lg hover:bg-purple-100 dark:hover:bg-purple-900/30 transition-colors">
            <GraduationCap className="w-5 h-5" />
            <span className="font-medium">Enroll Student</span>
          </button>
        </div>
      </div>

      {/* Recent Activity */}
      <div className="bg-white dark:bg-gray-800 rounded-xl border border-gray-200 dark:border-gray-700 p-6">
        <div className="flex items-center gap-3 mb-4">
          <Activity className="w-5 h-5 text-gray-600 dark:text-gray-400" />
          <h2 className="text-lg font-bold text-gray-900 dark:text-white">
            Recent Activity
          </h2>
        </div>
        <div className="text-center py-12">
          <p className="text-gray-500 dark:text-gray-400">
            No recent activity to display
          </p>
          <p className="text-sm text-gray-400 dark:text-gray-500 mt-2">
            Activity will appear here as you use the system
          </p>
        </div>
      </div>

      {/* Getting Started */}
      <div className="bg-blue-50 dark:bg-blue-900/20 border border-blue-200 dark:border-blue-800 rounded-xl p-6">
        <h2 className="text-lg font-bold text-blue-900 dark:text-blue-300 mb-4">
          Getting Started
        </h2>
        <div className="space-y-3">
          <div className="flex items-start gap-3">
            <div className="w-6 h-6 rounded-full bg-blue-600 text-white flex items-center justify-center text-sm font-bold flex-shrink-0">
              1
            </div>
            <div>
              <p className="font-medium text-blue-900 dark:text-blue-300">
                Set up your school structure
              </p>
              <p className="text-sm text-blue-700 dark:text-blue-400 mt-1">
                Create departments, grades, and classes
              </p>
            </div>
          </div>
          <div className="flex items-start gap-3">
            <div className="w-6 h-6 rounded-full bg-blue-600 text-white flex items-center justify-center text-sm font-bold flex-shrink-0">
              2
            </div>
            <div>
              <p className="font-medium text-blue-900 dark:text-blue-300">
                Add staff members
              </p>
              <p className="text-sm text-blue-700 dark:text-blue-400 mt-1">
                Create employee records and assign roles
              </p>
            </div>
          </div>
          <div className="flex items-start gap-3">
            <div className="w-6 h-6 rounded-full bg-blue-600 text-white flex items-center justify-center text-sm font-bold flex-shrink-0">
              3
            </div>
            <div>
              <p className="font-medium text-blue-900 dark:text-blue-300">
                Enroll students
              </p>
              <p className="text-sm text-blue-700 dark:text-blue-400 mt-1">
                Start adding students to your classes
              </p>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
};

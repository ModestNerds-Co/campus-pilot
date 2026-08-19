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
  Building2,
  GraduationCap,
  TrendingUp,
  TrendingDown,
  Calendar,
  DollarSign,
  UserCheck,
  BookOpen,
  Clock,
} from "lucide-react";

interface StatCardProps {
  title: string;
  value: string | number;
  icon: React.ComponentType<{ className?: string }>;
  trend?: {
    value: string;
    isPositive: boolean;
  };
  iconColor: string;
  iconBg: string;
}

const StatCard: React.FC<StatCardProps> = ({
  title,
  value,
  icon: Icon,
  trend,
  iconColor,
  iconBg,
}) => {
  return (
    <div className="bg-white dark:bg-gray-800 rounded-lg border border-gray-200 dark:border-gray-700 p-6 hover:shadow-md transition-shadow">
      <div className="flex items-center justify-between mb-4">
        <span className="text-sm font-medium text-gray-500 dark:text-gray-400">
          {title}
        </span>
        <div className={`p-2 rounded-lg ${iconBg}`}>
          <Icon className={`w-5 h-5 ${iconColor}`} />
        </div>
      </div>
      <div className="flex items-end justify-between">
        <div>
          <div className="text-2xl font-bold text-gray-900 dark:text-white mb-1">
            {value}
          </div>
          {trend && (
            <div className="flex items-center gap-1">
              {trend.isPositive ? (
                <TrendingUp className="w-4 h-4 text-green-600" />
              ) : (
                <TrendingDown className="w-4 h-4 text-red-600" />
              )}
              <span
                className={`text-sm font-medium ${trend.isPositive ? "text-green-600" : "text-red-600"}`}
              >
                {trend.value}
              </span>
            </div>
          )}
        </div>
      </div>
    </div>
  );
};

export const AdminDashboard: React.FC = () => {
  const { user } = useAuthStore();

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold text-gray-900 dark:text-white">
            Dashboard
          </h1>
          <p className="text-sm text-gray-500 dark:text-gray-400 mt-1">
            Welcome back, {user?.full_name}
          </p>
        </div>
        <div className="flex items-center gap-2 px-3 py-2 bg-white dark:bg-gray-800 border border-gray-200 dark:border-gray-700 rounded-lg text-sm text-gray-600 dark:text-gray-400">
          <Calendar className="w-4 h-4" />
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
        <StatCard
          title="Total students"
          value="0"
          icon={GraduationCap}
          trend={{ value: "0%", isPositive: true }}
          iconColor="text-blue-600"
          iconBg="bg-blue-50 dark:bg-blue-900/20"
        />
        <StatCard
          title="Total staff"
          value="0"
          icon={UserCheck}
          trend={{ value: "0%", isPositive: true }}
          iconColor="text-green-600"
          iconBg="bg-green-50 dark:bg-green-900/20"
        />
        <StatCard
          title="Departments"
          value="0"
          icon={Building2}
          iconColor="text-purple-600"
          iconBg="bg-purple-50 dark:bg-purple-900/20"
        />
        <StatCard
          title="Active users"
          value="1"
          icon={Users}
          trend={{ value: "0%", isPositive: true }}
          iconColor="text-orange-600"
          iconBg="bg-orange-50 dark:bg-orange-900/20"
        />
      </div>

      {/* Activity Chart */}
      <div className="bg-white dark:bg-gray-800 rounded-lg border border-gray-200 dark:border-gray-700 p-6">
        <div className="flex items-center justify-between mb-6">
          <h2 className="text-lg font-semibold text-gray-900 dark:text-white">
            Student Enrollment Trend
          </h2>
          <div className="flex items-center gap-2">
            <button className="px-3 py-1.5 text-sm text-gray-600 dark:text-gray-400 hover:bg-gray-100 dark:hover:bg-gray-700 rounded-lg transition-colors">
              Week
            </button>
            <button className="px-3 py-1.5 text-sm bg-blue-50 text-blue-600 dark:bg-blue-900/20 dark:text-blue-400 rounded-lg">
              Month
            </button>
            <button className="px-3 py-1.5 text-sm text-gray-600 dark:text-gray-400 hover:bg-gray-100 dark:hover:bg-gray-700 rounded-lg transition-colors">
              Year
            </button>
          </div>
        </div>
        <div className="h-64 flex items-center justify-center border-2 border-dashed border-gray-200 dark:border-gray-700 rounded-lg">
          <div className="text-center">
            <Clock className="w-12 h-12 text-gray-400 mx-auto mb-3" />
            <p className="text-gray-500 dark:text-gray-400">
              Chart visualization coming soon
            </p>
            <p className="text-sm text-gray-400 dark:text-gray-500 mt-1">
              Activity data will be displayed here
            </p>
          </div>
        </div>
      </div>

      {/* Two Column Layout */}
      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        {/* Recent Activity */}
        <div className="bg-white dark:bg-gray-800 rounded-lg border border-gray-200 dark:border-gray-700 p-6">
          <h2 className="text-lg font-semibold text-gray-900 dark:text-white mb-4">
            Recent Activity
          </h2>
          <div className="space-y-4">
            <div className="text-center py-8">
              <BookOpen className="w-12 h-12 text-gray-400 mx-auto mb-3" />
              <p className="text-gray-500 dark:text-gray-400">
                No recent activity
              </p>
              <p className="text-sm text-gray-400 dark:text-gray-500 mt-1">
                Activity will appear here as you use the system
              </p>
            </div>
          </div>
        </div>

        {/* Quick Actions */}
        <div className="bg-white dark:bg-gray-800 rounded-lg border border-gray-200 dark:border-gray-700 p-6">
          <h2 className="text-lg font-semibold text-gray-900 dark:text-white mb-4">
            Quick Actions
          </h2>
          <div className="space-y-3">
            <button className="w-full flex items-center gap-3 px-4 py-3 bg-blue-50 dark:bg-blue-900/20 text-blue-700 dark:text-blue-400 rounded-lg hover:bg-blue-100 dark:hover:bg-blue-900/30 transition-colors text-left">
              <Users className="w-5 h-5" />
              <span className="font-medium">Add New User</span>
            </button>
            <button className="w-full flex items-center gap-3 px-4 py-3 bg-green-50 dark:bg-green-900/20 text-green-700 dark:text-green-400 rounded-lg hover:bg-green-100 dark:hover:bg-green-900/30 transition-colors text-left">
              <Building2 className="w-5 h-5" />
              <span className="font-medium">Create Department</span>
            </button>
            <button className="w-full flex items-center gap-3 px-4 py-3 bg-purple-50 dark:bg-purple-900/20 text-purple-700 dark:text-purple-400 rounded-lg hover:bg-purple-100 dark:hover:bg-purple-900/30 transition-colors text-left">
              <GraduationCap className="w-5 h-5" />
              <span className="font-medium">Enroll Student</span>
            </button>
            <button className="w-full flex items-center gap-3 px-4 py-3 bg-orange-50 dark:bg-orange-900/20 text-orange-700 dark:text-orange-400 rounded-lg hover:bg-orange-100 dark:hover:bg-orange-900/30 transition-colors text-left">
              <BookOpen className="w-5 h-5" />
              <span className="font-medium">Create Subject</span>
            </button>
          </div>
        </div>
      </div>

      {/* Getting Started Section */}
      <div className="bg-gradient-to-r from-blue-50 to-indigo-50 dark:from-blue-900/20 dark:to-indigo-900/20 border border-blue-200 dark:border-blue-800 rounded-lg p-6">
        <h2 className="text-lg font-semibold text-blue-900 dark:text-blue-300 mb-4">
          Getting Started
        </h2>
        <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
          <div className="flex gap-3">
            <div className="w-8 h-8 rounded-full bg-blue-600 text-white flex items-center justify-center text-sm font-bold flex-shrink-0">
              1
            </div>
            <div>
              <p className="font-medium text-blue-900 dark:text-blue-300">
                Set up school structure
              </p>
              <p className="text-sm text-blue-700 dark:text-blue-400 mt-1">
                Create departments and classes
              </p>
            </div>
          </div>
          <div className="flex gap-3">
            <div className="w-8 h-8 rounded-full bg-blue-600 text-white flex items-center justify-center text-sm font-bold flex-shrink-0">
              2
            </div>
            <div>
              <p className="font-medium text-blue-900 dark:text-blue-300">
                Add staff members
              </p>
              <p className="text-sm text-blue-700 dark:text-blue-400 mt-1">
                Create employee records
              </p>
            </div>
          </div>
          <div className="flex gap-3">
            <div className="w-8 h-8 rounded-full bg-blue-600 text-white flex items-center justify-center text-sm font-bold flex-shrink-0">
              3
            </div>
            <div>
              <p className="font-medium text-blue-900 dark:text-blue-300">
                Enroll students
              </p>
              <p className="text-sm text-blue-700 dark:text-blue-400 mt-1">
                Start adding students
              </p>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
};

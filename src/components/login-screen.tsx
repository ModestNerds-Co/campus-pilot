//
//  campus-pilot
//  LoginScreen.tsx - Login Screen Component
//
//  Created by Ngonidzashe Mangudya on 26/09/2025.
//  Copyright (c) 2025 Codecraft Solutions
//

import React, { useState } from "react";
import {
  Eye,
  EyeOff,
  Loader2,
  AlertCircle,
  School,
  Mail,
  Lock,
} from "lucide-react";
import { bootstrapService } from "../modules/configs";
import { ThemeToggle } from "../lib/theme";
import toast from "react-hot-toast";

interface LoginScreenProps {
  className?: string;
}

export const LoginScreen: React.FC<LoginScreenProps> = ({ className = "" }) => {
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [showPassword, setShowPassword] = useState(false);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Get school data from mock storage for branding
  const schoolData = bootstrapService.getMockSchoolData();

  const getSchoolInitials = (name: string) => {
    return name
      .split(" ")
      .map((word) => word.charAt(0))
      .join("")
      .toUpperCase()
      .substring(0, 2);
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);

    if (!email.trim() || !password) {
      setError("Please enter both email and password");
      return;
    }

    setIsLoading(true);

    try {
      // TODO: Implement actual login API call
      // For now, just simulate login
      await new Promise((resolve) => setTimeout(resolve, 1500));

      // Mock successful login
      toast.success("Login successful!");

      // TODO: Redirect to dashboard
      console.log("Redirecting to dashboard...");
    } catch (err) {
      setError("Invalid email or password");
      toast.error("Login failed");
    } finally {
      setIsLoading(false);
    }
  };

  return (
    <div
      className={`min-h-screen bg-gradient-to-br from-blue-50 via-white to-gray-50 dark:from-gray-900 dark:via-gray-800 dark:to-gray-900 flex items-center justify-center p-4 ${className}`}
    >
      {/* Theme Toggle */}
      <div className="absolute top-6 right-6 z-10">
        <ThemeToggle />
      </div>

      <div className="w-full max-w-md">
        <div className="bg-white dark:bg-gray-800 rounded-2xl shadow-lg border border-gray-100 dark:border-gray-700 p-8">
          {/* School Branding */}
          <div className="text-center mb-8">
            <div className="w-20 h-20 mx-auto mb-4 bg-gradient-to-br from-blue-100 to-blue-200 rounded-full flex items-center justify-center">
              {schoolData?.logo_light_b64 ? (
                <img
                  src={`data:image/png;base64,${schoolData.logo_light_b64}`}
                  alt="School logo"
                  className="w-16 h-16 object-contain rounded-full"
                />
              ) : schoolData?.name ? (
                <span className="text-blue-600 font-bold text-xl">
                  {getSchoolInitials(schoolData.name)}
                </span>
              ) : (
                <School className="w-10 h-10 text-blue-600" />
              )}
            </div>

            <h1 className="text-2xl font-bold text-gray-900 dark:text-white mb-2">
              {schoolData?.name || "CampusPilot"}
            </h1>
            {schoolData?.legal_name &&
              schoolData.legal_name !== schoolData.name && (
                <p className="text-sm text-gray-600 dark:text-gray-300 mb-4">
                  {schoolData.legal_name}
                </p>
              )}
            <p className="text-gray-600 dark:text-gray-300">
              Sign in to your account
            </p>
          </div>

          {/* Login Form */}
          <form onSubmit={handleSubmit} className="space-y-6">
            {error && (
              <div className="bg-red-50 border border-red-200 rounded-lg p-3 flex items-center gap-2 text-red-700">
                <AlertCircle className="w-4 h-4 flex-shrink-0" />
                <span className="text-sm">{error}</span>
              </div>
            )}

            {/* Email Field */}
            <div>
              <label
                htmlFor="email"
                className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2"
              >
                Email Address
              </label>
              <div className="relative">
                <Mail className="absolute left-3 top-1/2 transform -translate-y-1/2 w-5 h-5 text-gray-400" />
                <input
                  id="email"
                  type="email"
                  value={email}
                  onChange={(e) => setEmail(e.target.value)}
                  disabled={isLoading}
                  className="w-full pl-12 pr-4 py-3 border border-gray-300 rounded-xl focus:ring-2 focus:ring-blue-500 focus:border-transparent transition-colors disabled:bg-gray-50 disabled:cursor-not-allowed"
                  placeholder="Enter your email"
                  autoComplete="email"
                />
              </div>
            </div>

            {/* Password Field */}
            <div>
              <label
                htmlFor="password"
                className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2"
              >
                Password
              </label>
              <div className="relative">
                <Lock className="absolute left-3 top-1/2 transform -translate-y-1/2 w-5 h-5 text-gray-400" />
                <input
                  id="password"
                  type={showPassword ? "text" : "password"}
                  value={password}
                  onChange={(e) => setPassword(e.target.value)}
                  disabled={isLoading}
                  className="w-full pl-12 pr-12 py-3 border border-gray-300 rounded-xl focus:ring-2 focus:ring-blue-500 focus:border-transparent transition-colors disabled:bg-gray-50 disabled:cursor-not-allowed"
                  placeholder="Enter your password"
                  autoComplete="current-password"
                />
                <button
                  type="button"
                  onClick={() => setShowPassword(!showPassword)}
                  disabled={isLoading}
                  className="absolute right-3 top-1/2 transform -translate-y-1/2 text-gray-400 hover:text-gray-600 dark:text-gray-300 disabled:cursor-not-allowed"
                >
                  {showPassword ? (
                    <EyeOff className="w-5 h-5" />
                  ) : (
                    <Eye className="w-5 h-5" />
                  )}
                </button>
              </div>
            </div>

            {/* Submit Button */}
            <button
              type="submit"
              disabled={isLoading}
              className="w-full px-6 py-3 bg-blue-600 hover:bg-blue-700 disabled:bg-blue-400 text-white font-semibold rounded-xl transition-colors flex items-center justify-center gap-2 disabled:cursor-not-allowed"
            >
              {isLoading ? (
                <>
                  <Loader2 className="w-5 h-5 animate-spin" />
                  Signing In...
                </>
              ) : (
                "Sign In"
              )}
            </button>
          </form>

          {/* Support Notice */}
          <div className="mt-8 text-center">
            <p className="text-sm text-gray-500 dark:text-gray-400">
              Having trouble?{" "}
              <span className="text-blue-600 hover:text-blue-700 dark:text-blue-400 dark:hover:text-blue-300 cursor-pointer">
                Contact your system admin
              </span>
            </p>
          </div>
        </div>

        {/* School Contact Info */}
        {(schoolData?.email || schoolData?.phone) && (
          <div className="mt-6 text-center">
            <div className="inline-flex items-center gap-4 px-4 py-2 bg-white dark:bg-gray-800 border border-gray-200 rounded-lg text-sm text-gray-600 dark:text-gray-300">
              {schoolData.email && (
                <span className="flex items-center gap-2">
                  <Mail className="w-3 h-3" />
                  {schoolData.email}
                </span>
              )}
              {schoolData.phone && <span>{schoolData.phone}</span>}
            </div>
          </div>
        )}
      </div>
    </div>
  );
};

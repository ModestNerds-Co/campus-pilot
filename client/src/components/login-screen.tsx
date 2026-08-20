//
//  campus-pilot
//  LoginScreen.tsx - Login Screen Component (token-driven, huchu elegance)
//  Canvas-neutral chrome, token surfaces/borders/text/brand. No literal grays/blues.
//

import React, { useState, useEffect } from "react";
import {
  Eye,
  EyeOff,
  Loader2,
  AlertCircle,
  School,
  Mail,
  Lock,
} from "lucide-react";
import { useNavigate } from "@tanstack/react-router";
import { ThemeToggle } from "../lib/theme";
import { useAuthStore } from "../stores/auth-store";
import { bootstrapService } from "../modules/configs";
import type { SchoolConfiguration } from "../modules/configs/types";
import toast from "react-hot-toast";

interface LoginScreenProps {
  className?: string;
}

export const LoginScreen: React.FC<LoginScreenProps> = ({ className = "" }) => {
  const navigate = useNavigate();
  const login = useAuthStore((state) => state.login);
  const authError = useAuthStore((state) => state.error);
  const clearError = useAuthStore((state) => state.clearError);

  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [showPassword, setShowPassword] = useState(false);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [schoolConfig, setSchoolConfig] = useState<SchoolConfiguration | null>(
    null,
  );

  useEffect(() => {
    const fetchSchoolConfig = async () => {
      try {
        const response = await bootstrapService.checkStatus();
        if (response.success && response.data?.school) {
          setSchoolConfig(response.data.school);
        }
      } catch (error) {
        console.error("Failed to fetch school config:", error);
      }
    };

    fetchSchoolConfig();
  }, []);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);
    clearError();

    if (!email.trim() || !password) {
      setError("Please enter both email and password");
      return;
    }

    setIsLoading(true);

    try {
      const success = await login(email.trim(), password);

      if (success) {
        toast.success("Login successful!");
        navigate({ to: "/admin" });
      } else {
        const errorMsg = authError || "Invalid email or password";
        setError(errorMsg);
        toast.error(errorMsg);
      }
    } catch (err) {
      const errorMsg =
        err instanceof Error ? err.message : "Login failed. Please try again.";
      setError(errorMsg);
      toast.error(errorMsg);
    } finally {
      setIsLoading(false);
    }
  };

  return (
    <div
      className={`min-h-screen bg-[var(--canvas)] flex items-center justify-center p-4 ${className}`}
      style={{ backgroundImage: "var(--app-canvas-wash)" }}
    >
      {/* Theme Toggle */}
      <div className="absolute top-6 right-6 z-10">
        <ThemeToggle />
      </div>

      <div className="w-full max-w-md">
        <div className="bg-[var(--surface)] rounded-[var(--radius-2xl)] border border-[var(--border)] p-8 shadow-[var(--shadow-popover)]">
          {/* School Branding */}
          <div className="text-center mb-8">
            <div className="w-20 h-20 mx-auto mb-4 bg-[var(--brand-soft)] border border-[var(--brand-100)] rounded-full flex items-center justify-center overflow-hidden">
              {schoolConfig?.logo_light_url ? (
                <img
                  src={schoolConfig.logo_light_url}
                  alt={`${schoolConfig.name} logo`}
                  className="w-full h-full object-cover"
                />
              ) : (
                <School className="w-10 h-10 text-[var(--brand)]" />
              )}
            </div>

            <h1 className="text-2xl font-bold text-[var(--text-strong)] mb-2">
              {schoolConfig?.name || "CampusPilot"}
            </h1>
            {schoolConfig?.legal_name && (
              <p className="text-sm text-[var(--text-muted)] mb-2">
                {schoolConfig.legal_name}
              </p>
            )}
            <p className="text-[var(--text-body)]">Sign in to your account</p>
          </div>

          {/* Login Form */}
          <form onSubmit={handleSubmit} className="space-y-6">
            {error && (
              <div className="bg-[var(--tone-danger-bg)] border border-[var(--tone-danger-bd)] rounded-[var(--radius-lg)] p-3 flex items-center gap-2 text-[var(--tone-danger-strong)]">
                <AlertCircle className="w-4 h-4 flex-shrink-0" />
                <span className="text-sm">{error}</span>
              </div>
            )}

            {/* Email Field */}
            <div>
              <label
                htmlFor="email"
                className="block text-sm font-medium text-[var(--text-strong)] mb-2"
              >
                Email Address
              </label>
              <div className="relative">
                <Mail className="pointer-events-none absolute left-3 top-1/2 -translate-y-1/2 w-5 h-5 text-[var(--text-subtle)]" />
                <input
                  id="email"
                  type="email"
                  value={email}
                  onChange={(e) => setEmail(e.target.value)}
                  disabled={isLoading}
                  data-slot="input"
                  className="w-full pl-11 pr-4 h-[var(--h-control-md)] rounded-[var(--radius-md)] border border-[var(--input-border)] bg-[var(--input-bg)] text-[var(--text-strong)] placeholder:text-[var(--text-subtle)] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--focus-ring)] focus-visible:ring-offset-0 transition-colors disabled:bg-[var(--surface-muted)] disabled:cursor-not-allowed text-sm"
                  placeholder="Enter your email"
                  autoComplete="email"
                />
              </div>
            </div>

            {/* Password Field */}
            <div>
              <label
                htmlFor="password"
                className="block text-sm font-medium text-[var(--text-strong)] mb-2"
              >
                Password
              </label>
              <div className="relative">
                <Lock className="pointer-events-none absolute left-3 top-1/2 -translate-y-1/2 w-5 h-5 text-[var(--text-subtle)]" />
                <input
                  id="password"
                  type={showPassword ? "text" : "password"}
                  value={password}
                  onChange={(e) => setPassword(e.target.value)}
                  disabled={isLoading}
                  data-slot="input"
                  className="w-full pl-11 pr-11 h-[var(--h-control-md)] rounded-[var(--radius-md)] border border-[var(--input-border)] bg-[var(--input-bg)] text-[var(--text-strong)] placeholder:text-[var(--text-subtle)] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--focus-ring)] focus-visible:ring-offset-0 transition-colors disabled:bg-[var(--surface-muted)] disabled:cursor-not-allowed text-sm"
                  placeholder="Enter your password"
                  autoComplete="current-password"
                />
                <button
                  type="button"
                  onClick={() => setShowPassword(!showPassword)}
                  disabled={isLoading}
                  aria-label={showPassword ? "Hide password" : "Show password"}
                  className="absolute right-2 top-1/2 -translate-y-1/2 inline-flex h-8 w-8 items-center justify-center rounded-[var(--radius-sm)] text-[var(--text-subtle)] hover:text-[var(--text-strong)] hover:bg-[var(--surface-muted)] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--focus-ring)] disabled:cursor-not-allowed"
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
              className="w-full px-6 h-[var(--h-control-md)] min-h-[var(--h-control-md)] bg-[var(--action-primary-bg)] hover:bg-[var(--action-primary-bg-hover)] active:bg-[var(--action-primary-bg-pressed)] disabled:bg-[var(--action-disabled-bg)] disabled:text-[var(--action-disabled-fg)] text-[var(--action-primary-fg)] font-semibold rounded-[var(--radius-md)] transition-colors flex items-center justify-center gap-2 disabled:cursor-not-allowed focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--focus-ring)] focus-visible:ring-offset-2 text-sm"
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
            <p className="text-sm text-[var(--text-muted)]">
              Having trouble?{" "}
              <span className="text-[var(--text-link)] hover:underline cursor-pointer">
                Contact your system admin
              </span>
            </p>
          </div>
        </div>

        {/* School Contact Info */}
        {(schoolConfig?.email || schoolConfig?.phone) && (
          <div className="mt-6 text-center">
            <div className="inline-flex items-center gap-4 px-4 py-2 bg-[var(--surface)] border border-[var(--border)] rounded-[var(--radius-lg)] text-sm text-[var(--text-body)] shadow-[var(--shadow-rest)]">
              {schoolConfig.email && (
                <span className="flex items-center gap-2">
                  <Mail className="w-3 h-3" />
                  {schoolConfig.email}
                </span>
              )}
              {schoolConfig.phone && <span>{schoolConfig.phone}</span>}
            </div>
          </div>
        )}

        {/* Powered by CampusPilot */}
        <div className="mt-8 flex items-center justify-center gap-1">
          <span className="text-xs text-[var(--text-subtle)]">
            Powered by Campus Pilot
          </span>
          <img
            src="/assets/images/campus-pilot-logo.svg"
            alt="CampusPilot"
            className="h-7"
          />
        </div>
      </div>
    </div>
  );
};

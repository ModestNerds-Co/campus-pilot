//
//  campus-pilot
//  login-screen.tsx - CCS-inspired split sign-in experience
//

import React, { useEffect, useState } from "react";
import { useNavigate } from "@tanstack/react-router";
import {
  ArrowRight,
  Eye,
  EyeOff,
  History,
  KeyRound,
  Loader2,
  LockKeyhole,
  School,
  ShieldCheck,
  UsersRound,
} from "lucide-react";
import toast from "react-hot-toast";

import { ThemeToggle } from "../lib/theme";
import { useAuthStore } from "../stores/auth-store";
import { bootstrapService } from "../modules/configs";
import type { SchoolConfiguration } from "../modules/configs/types";

interface LoginScreenProps {
  className?: string;
}

const assurances = [
  { icon: School, label: "One accountable campus workspace" },
  { icon: UsersRound, label: "Role-aware access for every team" },
  { icon: History, label: "Operational records with a clear history" },
];

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
  const [schoolConfig, setSchoolConfig] = useState<SchoolConfiguration | null>(null);

  useEffect(() => {
    let active = true;
    void bootstrapService
      .checkStatus()
      .then((response) => {
        if (active && response.success && response.data?.school) {
          setSchoolConfig(response.data.school);
        }
      })
      .catch(() => undefined);
    return () => {
      active = false;
    };
  }, []);

  const handleSubmit = async (event: React.FormEvent) => {
    event.preventDefault();
    setError(null);
    clearError();

    if (!email.trim() || !password) {
      setError("Enter your email address and password to continue.");
      return;
    }

    setIsLoading(true);
    try {
      const success = await login(email.trim(), password);
      if (success) {
        toast.success("Welcome back");
        navigate({ to: "/admin", replace: true });
      } else {
        const message = authError || "The email address or password does not match an active account.";
        setError(message);
      }
    } catch (reason) {
      const message =
        reason instanceof Error
          ? reason.message
          : "Campus Pilot could not be reached. Check your connection and try again.";
      setError(message);
    } finally {
      setIsLoading(false);
    }
  };

  const schoolName = schoolConfig?.name || "your school";

  return (
    <main className={`grid min-h-[100dvh] bg-[var(--canvas)] lg:grid-cols-[minmax(390px,44%)_1fr] ${className}`}>
      <section className="relative hidden overflow-hidden bg-[var(--sidebar)] px-12 py-10 text-[var(--sidebar-foreground)] lg:flex lg:flex-col xl:px-20">
        <div aria-hidden="true" className="campus-grid-pattern absolute inset-0 opacity-65" />
        <div className="relative z-10 flex items-center gap-3">
          <span className="flex size-11 items-center justify-center rounded-[10px] bg-[var(--brand-highlight)] text-[var(--sidebar-active-fg)]">
            <img
              alt=""
              aria-hidden="true"
              className="size-8 rounded-full object-cover mix-blend-multiply"
              src="/assets/images/campus-pilot-logo.svg"
            />
          </span>
          <div>
            <p className="text-base font-bold tracking-[-0.03em]">Campus Pilot</p>
            <p className="text-[11px] font-medium uppercase tracking-[0.16em] text-[var(--sidebar-muted)]">School operations</p>
          </div>
        </div>

        <div className="relative z-10 my-auto max-w-lg py-12">
          <p className="text-[11px] font-semibold uppercase tracking-[0.2em] text-[var(--brand-highlight)]">WELCOME TO CAMPUS PILOT</p>
          <h1 className="mt-6 text-5xl font-semibold leading-[1.04] tracking-[-0.055em] text-[var(--sidebar-foreground)] xl:text-6xl">
            Your campus,
            <br />
            clearly run.
          </h1>
          <p className="mt-6 max-w-md text-lg leading-8 text-[var(--sidebar-muted)]">
            People, learning and daily operations in one calm, accountable workspace.
          </p>

          <ul aria-label="Platform assurances" className="mt-11 space-y-4">
            {assurances.map(({ icon: Icon, label }) => (
              <li className="flex items-center gap-4" key={label}>
                <span className="flex size-10 items-center justify-center rounded-[9px] border border-[var(--sidebar-border)] bg-white/5 text-[var(--brand-highlight)]">
                  <Icon className="size-[18px]" />
                </span>
                <span className="text-sm font-semibold text-[var(--sidebar-foreground)]">{label}</span>
              </li>
            ))}
          </ul>
        </div>

        <div className="relative z-10 flex items-center gap-3 text-sm text-[var(--sidebar-muted)]">
          <span aria-hidden="true" className="size-2.5 rounded-full bg-[var(--brand-highlight)]" />
          Private campus workspace · Invite only
        </div>
      </section>

      <section className="flex min-h-[100dvh] flex-col bg-[var(--surface)]">
        <div className="flex items-center justify-between border-b border-[var(--border)] bg-[var(--sidebar)] px-5 py-4 lg:border-0 lg:bg-transparent lg:px-8 lg:py-6">
          <div className="flex items-center gap-2.5 text-[var(--sidebar-foreground)] lg:hidden">
            <span className="flex size-9 items-center justify-center rounded-[8px] bg-[var(--brand-highlight)]">
              <img alt="" aria-hidden="true" className="size-7 rounded-full object-cover mix-blend-multiply" src="/assets/images/campus-pilot-logo.svg" />
            </span>
            <span className="text-sm font-bold tracking-[-0.02em]">Campus Pilot</span>
          </div>
          <span className="hidden text-xs font-medium text-[var(--text-muted)] lg:block">Secure school workspace</span>
          <ThemeToggle />
        </div>

        <div className="flex flex-1 items-center justify-center px-5 py-10 sm:px-10 lg:py-12">
          <div className="w-full max-w-[430px]">
            <p className="text-[11px] font-semibold uppercase tracking-[0.18em] text-[var(--brand-strong)]">WELCOME BACK</p>
            <h2 className="mt-4 text-3xl font-semibold tracking-[-0.045em] text-[var(--text-strong)] sm:text-4xl">
              Sign in to {schoolName}
            </h2>
            <p className="mt-3 text-sm leading-6 text-[var(--text-muted)]">
              Use the account issued by your school administrator.
            </p>

            {error ? (
              <div aria-live="polite" className="mt-6 flex gap-3 rounded-[var(--radius-lg)] border border-[var(--tone-danger-bd)] bg-[var(--tone-danger-bg)] p-4 text-[var(--tone-danger-strong)]" id="login-error" role="alert">
                <LockKeyhole className="mt-0.5 size-4 shrink-0" />
                <div>
                  <p className="text-sm font-semibold">Unable to sign in</p>
                  <p className="mt-1 text-sm leading-5">{error}</p>
                </div>
              </div>
            ) : null}

            <form className="mt-8 space-y-6" onSubmit={handleSubmit}>
              <div className="space-y-2">
                <label className="block text-sm font-medium text-[var(--text-strong)]" htmlFor="email">
                  Email address
                </label>
                <input
                  aria-describedby={error ? "login-error" : undefined}
                  autoComplete="email"
                  autoFocus
                  className="h-11 w-full rounded-[var(--radius-md)] border border-[var(--input-border)] bg-[var(--input-bg)] px-3.5 text-base text-[var(--text-strong)] placeholder:text-[var(--text-subtle)] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--focus-ring)] disabled:cursor-not-allowed disabled:opacity-60 sm:text-sm"
                  disabled={isLoading}
                  id="email"
                  inputMode="email"
                  onChange={(event) => setEmail(event.target.value)}
                  placeholder="you@school.edu"
                  type="email"
                  value={email}
                />
              </div>

              <div className="space-y-2">
                <div className="flex items-baseline justify-between gap-4">
                  <label className="block text-sm font-medium text-[var(--text-strong)]" htmlFor="password">
                    Password
                  </label>
                  <span className="text-xs text-[var(--text-muted)]">Managed by your administrator</span>
                </div>
                <div className="relative">
                  <input
                    aria-describedby={error ? "login-error" : undefined}
                    autoComplete="current-password"
                    className="h-11 w-full rounded-[var(--radius-md)] border border-[var(--input-border)] bg-[var(--input-bg)] px-3.5 pr-12 text-base text-[var(--text-strong)] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--focus-ring)] disabled:cursor-not-allowed disabled:opacity-60 sm:text-sm"
                    disabled={isLoading}
                    id="password"
                    onChange={(event) => setPassword(event.target.value)}
                    type={showPassword ? "text" : "password"}
                    value={password}
                  />
                  <button
                    aria-label={showPassword ? "Hide password" : "Show password"}
                    className="absolute inset-y-0 right-0 flex w-11 items-center justify-center rounded-r-[var(--radius-md)] text-[var(--text-muted)] hover:bg-[var(--surface-muted)] hover:text-[var(--text-strong)] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--focus-ring)]"
                    disabled={isLoading}
                    onClick={() => setShowPassword((visible) => !visible)}
                    type="button"
                  >
                    {showPassword ? <EyeOff className="size-4" /> : <Eye className="size-4" />}
                  </button>
                </div>
              </div>

              <button
                className="flex h-11 w-full items-center justify-center gap-2 rounded-[var(--radius-md)] bg-[var(--action-primary-bg)] px-5 text-sm font-semibold text-[var(--action-primary-fg)] hover:bg-[var(--action-primary-bg-hover)] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--focus-ring)] focus-visible:ring-offset-2 disabled:cursor-not-allowed disabled:bg-[var(--action-disabled-bg)] disabled:text-[var(--action-disabled-fg)]"
                disabled={isLoading}
                type="submit"
              >
                {isLoading ? (
                  <>
                    <Loader2 className="size-4 animate-spin" />
                    Signing in…
                  </>
                ) : (
                  <>
                    Sign in
                    <ArrowRight className="size-4" />
                  </>
                )}
              </button>
            </form>

            <div className="mt-8 border-t border-[var(--border)] pt-6 text-center text-sm text-[var(--text-muted)]">
              Need access? Contact your school administrator.
            </div>
            <div className="mt-9 flex items-center justify-center gap-2 text-xs text-[var(--text-subtle)]">
              <KeyRound className="size-3.5" />
              <span>Protected workspace</span>
              <span aria-hidden="true">·</span>
              <ShieldCheck className="size-3.5" />
              <span>Role-aware access</span>
            </div>
          </div>
        </div>
      </section>
    </main>
  );
};

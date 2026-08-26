//
//  campus-pilot
//  BootScreen.tsx - Bootstrap workspace check.
//  Canvas-neutral chrome, token surfaces/borders/text/tones. No literal grays/blues.
//  Created by Ngonidzashe Mangudya on 26/09/2025.
//  Copyright (c) 2025 Codecraft Solutions
//

import React, { useEffect, useState } from "react";
import { useNavigate } from "@tanstack/react-router";
import { Loader2, WifiOff, AlertTriangle, RefreshCw } from "lucide-react";
import { bootstrapService } from "../../services/bootstrap-service";
import type { BootstrapState } from "../../types";
import { ThemeToggle } from "../../../../lib/theme";
import toast from "react-hot-toast";

interface BootScreenProps {
  className?: string;
}

export const BootScreen: React.FC<BootScreenProps> = ({ className = "" }) => {
  const [status, setStatus] = useState<
    "loading" | "offline" | "error" | "success"
  >("loading");
  const [error, setError] = useState<string | null>(null);
  const [isRetrying, setIsRetrying] = useState(false);
  const navigate = useNavigate();

  const checkBootstrapStatus = async (isRetry = false) => {
    if (isRetry) {
      setIsRetrying(true);
    }

    try {
      setStatus("loading");
      setError(null);

      const response = await bootstrapService.checkStatus();

      if (response.success && response.data) {
        setStatus("success");
        const state = response.data.state;

        // Route based on bootstrap state
        setTimeout(() => {
          switch (state) {
            case "Uninitialized":
              navigate({ to: "/setup/school" });
              break;
            case "SchoolConfigured":
              navigate({ to: "/setup/admin" });
              break;
            case "Ready":
              navigate({ to: "/login" });
              break;
            default:
              console.error("Unknown bootstrap state:", state);
              setError("Unknown system state. Please contact support.");
              setStatus("error");
          }
        }, 500);
      } else {
        setError(response.message || "Failed to check system status");
        setStatus("error");
      }
    } catch (err) {
      console.error("Bootstrap status check failed:", err);

      // Check if it's a network error
      if (
        err instanceof Error &&
        (err.message.includes("network") || err.message.includes("Network"))
      ) {
        setStatus("offline");
      } else {
        setError(err instanceof Error ? err.message : "System check failed");
        setStatus("error");
      }
    } finally {
      if (isRetry) {
        setIsRetrying(false);
      }
    }
  };

  useEffect(() => {
    checkBootstrapStatus();
  }, []);

  const handleRetry = () => {
    checkBootstrapStatus(true);
  };

  const renderContent = () => {
    switch (status) {
      case "loading":
        return (
          <div className="space-y-6">
            <div className="w-14 h-14">
              <Loader2 className="w-full h-full text-[var(--brand)] animate-spin" />
            </div>
            <div className="space-y-2">
              <h1 className="text-[length:var(--type-page-title-size)] font-bold text-[var(--text-strong)]">
                Preparing Campus Pilot
              </h1>
              <p className="text-[var(--text-muted)]">
                Checking this school's workspace configuration…
              </p>
            </div>
          </div>
        );

      case "offline":
        return (
          <div className="space-y-6">
            <div className="w-14 h-14 bg-[var(--tone-warn-bg)] border border-[var(--tone-warn-bd)] rounded-[var(--radius-xl)] flex items-center justify-center">
              <WifiOff className="w-8 h-8 text-[var(--tone-warn)]" />
            </div>
            <div className="space-y-4">
              <h1 className="text-[length:var(--type-page-title-size)] font-bold text-[var(--text-strong)]">
                You're offline
              </h1>
              <p className="text-[var(--text-muted)] max-w-md leading-relaxed">
                Campus Pilot cannot reach the school service. Reconnect, then try the workspace check again.
              </p>
              <div className="pt-2">
                <button
                  onClick={handleRetry}
                  disabled={isRetrying}
                  className="px-6 h-[var(--h-control-md)] min-h-[var(--h-control-md)] bg-[var(--tone-warn)] hover:bg-[var(--tone-warn-strong)] disabled:bg-[var(--action-disabled-bg)] disabled:text-[var(--action-disabled-fg)] text-[var(--on-brand)] font-semibold rounded-[var(--radius-md)] transition-colors flex items-center gap-2 disabled:cursor-not-allowed focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--focus-ring)] focus-visible:ring-offset-2 text-sm"
                >
                  {isRetrying ? (
                    <>
                      <Loader2 className="w-4 h-4 animate-spin" />
                      Retrying…
                    </>
                  ) : (
                    <>
                      <RefreshCw className="w-4 h-4" />
                      Retry connection
                    </>
                  )}
                </button>
              </div>
            </div>
          </div>
        );

      case "error":
        return (
          <div className="space-y-6">
            <div className="w-14 h-14 bg-[var(--tone-danger-bg)] border border-[var(--tone-danger-bd)] rounded-[var(--radius-xl)] flex items-center justify-center">
              <AlertTriangle className="w-8 h-8 text-[var(--tone-danger)]" />
            </div>
            <div className="space-y-4">
              <h1 className="text-[length:var(--type-page-title-size)] font-bold text-[var(--text-strong)]">
                Configuration error
              </h1>
              <p className="text-[var(--text-muted)] max-w-md leading-relaxed">
                {error || "Unable to determine system configuration status."}
              </p>
              <div className="pt-2">
                <button
                  onClick={handleRetry}
                  disabled={isRetrying}
                  className="px-6 h-[var(--h-control-md)] min-h-[var(--h-control-md)] bg-[var(--tone-danger)] hover:bg-[var(--tone-danger-strong)] disabled:bg-[var(--action-disabled-bg)] disabled:text-[var(--action-disabled-fg)] text-[var(--on-brand)] font-semibold rounded-[var(--radius-md)] transition-colors flex items-center gap-2 disabled:cursor-not-allowed focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--focus-ring)] focus-visible:ring-offset-2 text-sm"
                >
                  {isRetrying ? (
                    <>
                      <Loader2 className="w-4 h-4 animate-spin" />
                      Retrying…
                    </>
                  ) : (
                    <>
                      <RefreshCw className="w-4 h-4" />
                      Try again
                    </>
                  )}
                </button>
              </div>
            </div>
          </div>
        );

      case "success":
        return (
          <div className="space-y-6">
            <div className="w-14 h-14">
              <Loader2 className="w-full h-full text-[var(--tone-success)] animate-spin" />
            </div>
            <div className="space-y-2">
              <h1 className="text-[length:var(--type-page-title-size)] font-bold text-[var(--text-strong)]">
                Opening sign in
              </h1>
              <p className="text-[var(--tone-success)] font-medium">Configuration loaded</p>
            </div>
          </div>
        );

      default:
        return null;
    }
  };

  return (
    <main className={`grid min-h-[100dvh] bg-[var(--canvas)] lg:grid-cols-[minmax(360px,38%)_1fr] ${className}`}>
      <section className="relative hidden overflow-hidden bg-[var(--sidebar)] px-12 py-10 text-[var(--sidebar-foreground)] lg:flex lg:flex-col xl:px-16">
        <div aria-hidden="true" className="campus-grid-pattern absolute inset-0 opacity-60" />
        <div className="relative z-10 flex items-center gap-3">
          <span className="flex size-11 items-center justify-center rounded-[10px] bg-[var(--brand-highlight)]">
            <img alt="" aria-hidden="true" className="size-8 rounded-full object-cover mix-blend-multiply" src="/assets/images/campus-pilot-logo.svg" />
          </span>
          <div>
            <p className="text-base font-bold tracking-[-0.03em]">Campus Pilot</p>
            <p className="text-[11px] font-medium uppercase tracking-[0.16em] text-[var(--sidebar-muted)]">School operations</p>
          </div>
        </div>
        <div className="relative z-10 my-auto max-w-md py-12">
          <p className="text-[11px] font-semibold uppercase tracking-[0.2em] text-[var(--brand-highlight)]">Workspace status</p>
          <h2 className="mt-5 text-4xl font-semibold leading-tight tracking-[-0.05em]">Checking school configuration</h2>
          <p className="mt-5 text-base leading-7 text-[var(--sidebar-muted)]">The sign-in page will open when the check completes.</p>
        </div>
      </section>

      <section className="flex min-h-[100dvh] flex-col bg-[var(--surface)]">
        <div className="flex items-center justify-between border-b border-[var(--border)] bg-[var(--sidebar)] px-5 py-4 text-[var(--sidebar-foreground)] lg:border-0 lg:bg-transparent lg:px-8 lg:py-6">
          <div className="flex items-center gap-2.5 lg:hidden">
            <span className="flex size-9 items-center justify-center rounded-[8px] bg-[var(--brand-highlight)]">
              <img alt="" aria-hidden="true" className="size-7 rounded-full object-cover mix-blend-multiply" src="/assets/images/campus-pilot-logo.svg" />
            </span>
            <span className="text-sm font-bold">Campus Pilot</span>
          </div>
          <span className="hidden text-xs font-medium text-[var(--text-muted)] lg:block">System status</span>
          <ThemeToggle />
        </div>

        <div className="flex flex-1 items-center px-5 py-12 sm:px-10 lg:px-16">
          <div className="w-full max-w-[480px]">
            {renderContent()}

            {status === "offline" && (
              <div className="mt-8 inline-flex items-center gap-2 rounded-[var(--radius-lg)] border border-[var(--tone-warn-bd)] bg-[var(--tone-warn-bg)] px-4 py-2 text-sm text-[var(--tone-warn-strong)]">
                <div className="size-2 rounded-full bg-[var(--tone-warn)]" />
                Connection required
              </div>
            )}
          </div>
        </div>
      </section>
    </main>
  );
};

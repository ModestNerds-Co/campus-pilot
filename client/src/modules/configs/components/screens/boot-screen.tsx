//
//  campus-pilot
//  BootScreen.tsx - Bootstrap Loading Screen (token-driven, huchu elegance)
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
          <div className="text-center space-y-6">
            <div className="w-16 h-16 mx-auto">
              <Loader2 className="w-full h-full text-[var(--brand)] animate-spin" />
            </div>
            <div className="space-y-2">
              <h1 className="text-[length:var(--type-page-title-size)] font-bold text-[var(--text-strong)]">
                CampusPilot
              </h1>
              <p className="text-[var(--text-muted)]">
                Checking configuration...
              </p>
            </div>
          </div>
        );

      case "offline":
        return (
          <div className="text-center space-y-6">
            <div className="w-16 h-16 mx-auto bg-[var(--tone-warn-bg)] border border-[var(--tone-warn-bd)] rounded-full flex items-center justify-center">
              <WifiOff className="w-8 h-8 text-[var(--tone-warn)]" />
            </div>
            <div className="space-y-4">
              <h1 className="text-[length:var(--type-page-title-size)] font-bold text-[var(--text-strong)]">
                You're offline
              </h1>
              <p className="text-[var(--text-muted)] max-w-md mx-auto leading-relaxed">
                Setup can proceed offline. Changes will sync when internet
                becomes available.
              </p>
              <div className="pt-2">
                <button
                  onClick={handleRetry}
                  disabled={isRetrying}
                  className="px-6 h-[var(--h-control-md)] min-h-[var(--h-control-md)] bg-[var(--tone-warn)] hover:bg-[var(--tone-warn-strong)] disabled:bg-[var(--action-disabled-bg)] disabled:text-[var(--action-disabled-fg)] text-[var(--on-brand)] font-semibold rounded-[var(--radius-md)] transition-colors flex items-center gap-2 mx-auto disabled:cursor-not-allowed focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--focus-ring)] focus-visible:ring-offset-2 text-sm"
                >
                  {isRetrying ? (
                    <>
                      <Loader2 className="w-4 h-4 animate-spin" />
                      Retrying...
                    </>
                  ) : (
                    <>
                      <RefreshCw className="w-4 h-4" />
                      Retry Connection
                    </>
                  )}
                </button>
              </div>
            </div>
          </div>
        );

      case "error":
        return (
          <div className="text-center space-y-6">
            <div className="w-16 h-16 mx-auto bg-[var(--tone-danger-bg)] border border-[var(--tone-danger-bd)] rounded-full flex items-center justify-center">
              <AlertTriangle className="w-8 h-8 text-[var(--tone-danger)]" />
            </div>
            <div className="space-y-4">
              <h1 className="text-[length:var(--type-page-title-size)] font-bold text-[var(--text-strong)]">
                Configuration Error
              </h1>
              <p className="text-[var(--text-muted)] max-w-md mx-auto leading-relaxed">
                {error || "Unable to determine system configuration status."}
              </p>
              <div className="pt-2">
                <button
                  onClick={handleRetry}
                  disabled={isRetrying}
                  className="px-6 h-[var(--h-control-md)] min-h-[var(--h-control-md)] bg-[var(--tone-danger)] hover:bg-[var(--tone-danger-strong)] disabled:bg-[var(--action-disabled-bg)] disabled:text-[var(--action-disabled-fg)] text-[var(--on-brand)] font-semibold rounded-[var(--radius-md)] transition-colors flex items-center gap-2 mx-auto disabled:cursor-not-allowed focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--focus-ring)] focus-visible:ring-offset-2 text-sm"
                >
                  {isRetrying ? (
                    <>
                      <Loader2 className="w-4 h-4 animate-spin" />
                      Retrying...
                    </>
                  ) : (
                    <>
                      <RefreshCw className="w-4 h-4" />
                      Try Again
                    </>
                  )}
                </button>
              </div>
            </div>
          </div>
        );

      case "success":
        return (
          <div className="text-center space-y-6">
            <div className="w-16 h-16 mx-auto">
              <Loader2 className="w-full h-full text-[var(--tone-success)] animate-spin" />
            </div>
            <div className="space-y-2">
              <h1 className="text-[length:var(--type-page-title-size)] font-bold text-[var(--text-strong)]">
                CampusPilot
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
          {renderContent()}
        </div>

        {/* Offline indicator */}
        {status === "offline" && (
          <div className="mt-6 text-center">
            <div className="inline-flex items-center gap-2 px-4 py-2 bg-[var(--tone-warn-bg)] border border-[var(--tone-warn-bd)] rounded-[var(--radius-lg)] text-sm text-[var(--tone-warn-strong)]">
              <div className="w-2 h-2 bg-[var(--tone-warn)] rounded-full"></div>
              Works offline
            </div>
          </div>
        )}
      </div>
    </div>
  );
};

//
//  campus-pilot
//  BootScreen.tsx - Bootstrap Loading Screen
//
//  Created by Ngonidzashe Mangudya on 26/09/2025.
//  Copyright (c) 2025 Codecraft Solutions
//

import React, { useEffect, useState } from "react";
import { useNavigate } from "@tanstack/react-router";
import { Loader2, WifiOff, AlertTriangle, RefreshCw } from "lucide-react";
import { bootstrapService } from "../../services/bootstrapService";
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
              <Loader2 className="w-full h-full text-blue-600 animate-spin" />
            </div>
            <div className="space-y-2">
              <h1 className="text-2xl font-bold text-gray-900 dark:text-white">
                CampusPilot
              </h1>
              <p className="text-gray-600 dark:text-gray-300">
                Checking configuration...
              </p>
            </div>
          </div>
        );

      case "offline":
        return (
          <div className="text-center space-y-6">
            <div className="w-16 h-16 mx-auto bg-orange-100 rounded-full flex items-center justify-center">
              <WifiOff className="w-8 h-8 text-orange-600" />
            </div>
            <div className="space-y-4">
              <h1 className="text-2xl font-bold text-gray-900 dark:text-white">
                You're offline
              </h1>
              <p className="text-gray-600 dark:text-gray-300 max-w-md mx-auto leading-relaxed">
                Setup can proceed offline. Changes will sync when internet
                becomes available.
              </p>
              <div className="pt-2">
                <button
                  onClick={handleRetry}
                  disabled={isRetrying}
                  className="px-6 py-3 bg-orange-600 hover:bg-orange-700 disabled:bg-orange-400 text-white font-semibold rounded-xl transition-colors flex items-center gap-2 mx-auto disabled:cursor-not-allowed"
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
            <div className="w-16 h-16 mx-auto bg-red-100 rounded-full flex items-center justify-center">
              <AlertTriangle className="w-8 h-8 text-red-600" />
            </div>
            <div className="space-y-4">
              <h1 className="text-2xl font-bold text-gray-900 dark:text-white">
                Configuration Error
              </h1>
              <p className="text-gray-600 dark:text-gray-300 max-w-md mx-auto leading-relaxed">
                {error || "Unable to determine system configuration status."}
              </p>
              <div className="pt-2">
                <button
                  onClick={handleRetry}
                  disabled={isRetrying}
                  className="px-6 py-3 bg-red-600 hover:bg-red-700 disabled:bg-red-400 text-white font-semibold rounded-xl transition-colors flex items-center gap-2 mx-auto disabled:cursor-not-allowed"
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
              <Loader2 className="w-full h-full text-green-600 animate-spin" />
            </div>
            <div className="space-y-2">
              <h1 className="text-2xl font-bold text-gray-900 dark:text-white">
                CampusPilot
              </h1>
              <p className="text-green-600 font-medium">Configuration loaded</p>
            </div>
          </div>
        );

      default:
        return null;
    }
  };

  return (
    <div
      className={`min-h-screen bg-gradient-to-br from-blue-50 via-white to-gray-50 dark:from-gray-900 dark:via-gray-800 dark:to-gray-900 flex items-center justify-center p-4 ${className}`}
    >
      {/* Theme Toggle */}
      <div className="absolute top-6 right-6">
        <ThemeToggle />
      </div>

      <div className="w-full max-w-md">
        <div className="bg-white dark:bg-gray-800 rounded-2xl shadow-lg border border-gray-100 dark:border-gray-700 p-8">
          {renderContent()}
        </div>

        {/* Offline indicator */}
        {status === "offline" && (
          <div className="mt-6 text-center">
            <div className="inline-flex items-center gap-2 px-4 py-2 bg-orange-50 dark:bg-orange-900/20 border border-orange-200 dark:border-orange-800 rounded-lg text-sm text-orange-700 dark:text-orange-300">
              <div className="w-2 h-2 bg-orange-500 rounded-full"></div>
              Works offline
            </div>
          </div>
        )}
      </div>
    </div>
  );
};

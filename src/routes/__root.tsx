//
//  campus-pilot
//  __root.tsx
//
//  Created by Ngonidzashe Mangudya on 21/08/2025.
//  Copyright (c) 2025 Codecraft Solutions
//

import { createRootRoute, Outlet, useNavigate } from "@tanstack/react-router";
import { TanStackRouterDevtools } from "@tanstack/router-devtools";
import { ConnectionStatus } from "../components/ConnectionStatus";
import { GlobalKeyboardHandler } from "../components/GlobalKeyboardHandler";
import { TabBar } from "../components/TabBar";
import { LogViewer } from "../components/LogViewer";
import { useConnectionTest } from "../hooks/useConnection";
import { useVersionCheck } from "../hooks/useVersionCheck";
import { ChangelogModal } from "../components/ChangelogModal";
import { Loader2, Info, Database, BarChart3, Calendar } from "lucide-react";
import { APP_VERSION } from "../lib/version";

export const Route = createRootRoute({
  component: RootComponent,
});

function RootComponent() {
  const { data: connection, isLoading, error } = useConnectionTest();
  const {
    showChangelogManually,
    showChangelog: showChangelogModal,
    newChanges,
    closeChangelog,
  } = useVersionCheck();
  const navigate = useNavigate();

  // Show connecting splash screen
  if (isLoading) {
    return (
      <div className="fixed inset-0 bg-background flex items-center justify-center">
        <div className="text-center space-y-4">
          <Loader2 className="w-12 h-12 animate-spin mx-auto text-primary" />
          <h1 className="text-2xl font-semibold">Connecting to Database...</h1>
          <p className="text-sm text-muted-foreground">
            Establishing secure connection
          </p>
        </div>
      </div>
    );
  }

  // Show connection error with retry
  if (error || (connection && !connection.success)) {
    return (
      <div className="fixed inset-0 bg-background flex items-center justify-center">
        <div className="max-w-md w-full mx-4">
          <div className="bg-card border border-destructive/50 rounded-lg p-6 space-y-4">
            <div className="space-y-2">
              <h1 className="text-xl font-semibold text-destructive">
                Connection Failed
              </h1>
              <p className="text-sm text-muted-foreground">
                Unable to connect to the database. Please check your
                configuration.
              </p>
            </div>

            <details className="text-xs">
              <summary className="cursor-pointer font-medium">
                Error Details
              </summary>
              <pre className="mt-2 p-2 bg-muted rounded text-[10px] overflow-auto">
                {error?.message || connection?.error || "Unknown error"}
              </pre>
            </details>

            <div className="flex gap-2">
              <button
                onClick={() => window.location.reload()}
                className="flex-1 compact-button bg-primary text-white hover:bg-primary/90"
              >
                Retry Connection
              </button>
              <button
                onClick={() => {
                  // In a real app, this would open settings
                  alert("Edit .env file and restart the application");
                }}
                className="flex-1 compact-button bg-secondary text-secondary-foreground hover:bg-secondary/90"
              >
                Configure
              </button>
            </div>
          </div>
        </div>
      </div>
    );
  }

  // Main application layout
  return (
    <>
      <div className="flex flex-col h-screen bg-white">
        {/* Header */}
        <header className="sticky-header px-4 py-2 flex items-center justify-between">
          <div className="flex items-center gap-4">
            <h1
              className="text-sm font-bold text-primary"
              onClick={() => navigate({ to: "/" } as any)}
            >
              TGPatcher
            </h1>
            <ConnectionStatus />
          </div>

          <div className="flex items-center gap-2">
            <button
              onClick={() => navigate({ to: "/stats" } as any)}
              className="flex items-center gap-1 px-2 py-1 text-xs text-gray-500 hover:text-green-600 hover:bg-green-50 rounded transition-colors"
              title="System statistics and analytics"
            >
              <BarChart3 className="w-3 h-3" />
              Stats
            </button>
            <button
              onClick={() => navigate({ to: "/appointments" } as any)}
              className="flex items-center gap-1 px-2 py-1 text-xs text-gray-500 hover:text-blue-600 hover:bg-blue-50 rounded transition-colors"
              title="Appointment management"
            >
              <Calendar className="w-3 h-3" />
              Appointments
            </button>
            <button
              onClick={() => navigate({ to: "/admin" } as any)}
              className="flex items-center gap-1 px-2 py-1 text-xs text-gray-500 hover:text-slate-600 hover:bg-slate-50 rounded transition-colors"
              title="Database administration tools"
            >
              <Database className="w-3 h-3" />
              Admin
            </button>
            <button
              onClick={showChangelogManually}
              className="flex items-center gap-1 px-2 py-1 text-xs text-gray-500 hover:text-blue-600 hover:bg-blue-50 rounded transition-colors"
              title="View changelog"
            >
              <Info className="w-3 h-3" />v{APP_VERSION}
            </button>
          </div>
        </header>

        {/* Tab Bar */}
        <TabBar />

        {/* Main Content */}
        <main className="flex-1 overflow-auto">
          <GlobalKeyboardHandler />
          <Outlet />
        </main>
      </div>

      {/* Changelog Modal */}
      <ChangelogModal
        isOpen={showChangelogModal}
        onClose={closeChangelog}
        entries={newChanges}
        currentVersion={APP_VERSION}
      />

      {/* Dev tools in development */}
      {process.env.NODE_ENV === "development" && (
        <>
          <TanStackRouterDevtools position="bottom-left" />
          <LogViewer />
        </>
      )}
    </>
  );
}

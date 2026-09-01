//
//  campus-pilot
//  protected-route.tsx - Protected Route Component (token-driven)
//

import React, { useEffect, useState } from "react";
import { useNavigate } from "@tanstack/react-router";
import { useAuthStore } from "../stores/auth-store";
import { Loader2 } from "lucide-react";

interface ProtectedRouteProps {
  children: React.ReactNode;
  requiredPermission?: string;
  requiredAnyPermissions?: readonly string[];
  requiredModule?: string;
  requiredRecordScope?: string;
  requiredRecordScopeKind?: "self" | "assigned" | "self_and_assigned" | "campus";
}

export const ProtectedRoute: React.FC<ProtectedRouteProps> = ({
  children,
  requiredPermission,
  requiredAnyPermissions,
  requiredModule,
  requiredRecordScope,
  requiredRecordScopeKind,
}) => {
  const navigate = useNavigate();
  const { isAuthenticated, accessToken, checkAuth } = useAuthStore();
  const [isChecking, setIsChecking] = useState(true);

  useEffect(() => {
    let active = true;

    const verifyAuth = async () => {
      if (!isAuthenticated || !accessToken) {
        navigate({ to: "/login" });
        return;
      }

      const isCurrent = await checkAuth();
      if (!active) return;

      if (!isCurrent) {
        navigate({ to: "/login", replace: true });
        return;
      }

      const currentUser = useAuthStore.getState().user;
      const hasPermission =
        !requiredPermission ||
        currentUser?.permissions?.includes("*") ||
        currentUser?.permissions?.includes(requiredPermission);
      const hasAnyPermission =
        !requiredAnyPermissions?.length ||
        currentUser?.permissions?.includes("*") ||
        requiredAnyPermissions.some((permission) => currentUser?.permissions?.includes(permission));
      const hasModule = !requiredModule || currentUser?.modules?.includes(requiredModule);
      const hasRecordScope =
        !requiredRecordScope ||
        (requiredRecordScopeKind
          ? currentUser?.record_scopes?.[requiredRecordScope] === requiredRecordScopeKind
          : Boolean(currentUser?.record_scopes?.[requiredRecordScope]));

      if (!hasPermission || !hasAnyPermission || !hasModule || !hasRecordScope) {
        navigate({ to: "/home", replace: true });
        return;
      }

      setIsChecking(false);
    };

    void verifyAuth();
    return () => {
      active = false;
    };
  }, [isAuthenticated, accessToken, checkAuth, navigate, requiredAnyPermissions, requiredPermission, requiredModule, requiredRecordScope, requiredRecordScopeKind]);

  if (isChecking) {
    return (
      <div className="flex min-h-screen items-center justify-center bg-[var(--canvas)]">
        <div className="text-center">
          <Loader2 className="mx-auto mb-4 size-8 animate-spin text-[var(--brand)]" />
          <p className="text-sm text-[var(--text-muted)]">Checking your access…</p>
        </div>
      </div>
    );
  }

  return <>{children}</>;
};

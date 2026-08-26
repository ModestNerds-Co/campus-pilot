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
  requiredModule?: string;
}

export const ProtectedRoute: React.FC<ProtectedRouteProps> = ({
  children,
  requiredPermission,
  requiredModule,
}) => {
  const navigate = useNavigate();
  const { isAuthenticated, user, accessToken } = useAuthStore();
  const [isChecking, setIsChecking] = useState(true);

  useEffect(() => {
    const verifyAuth = async () => {
      if (!isAuthenticated || !accessToken) {
        navigate({ to: "/login" });
        return;
      }
      if (user) {
        const hasPermission =
          !requiredPermission ||
          user.permissions?.includes("*") ||
          user.permissions?.includes(requiredPermission);
        const hasModule = !requiredModule || user.modules?.includes(requiredModule);
        if (!hasPermission || !hasModule) {
          navigate({ to: "/home", replace: true });
          return;
        }
      }
      setIsChecking(false);
    };
    verifyAuth();
  }, [isAuthenticated, user, accessToken, navigate, requiredPermission, requiredModule]);

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

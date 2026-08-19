//
//  campus-pilot
//  protected-route.tsx - Protected Route Component
//
//  Created by Ngonidzashe Mangudya on 02/10/2025.
//  Copyright (c) 2025 Codecraft Solutions
//

import React, { useEffect, useState } from "react";
import { useNavigate } from "@tanstack/react-router";
import { useAuthStore } from "../stores/auth-store";
import { Loader2 } from "lucide-react";

interface ProtectedRouteProps {
  children: React.ReactNode;
  requiredRoles?: string[];
}

export const ProtectedRoute: React.FC<ProtectedRouteProps> = ({
  children,
  requiredRoles = [],
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

      if (requiredRoles.length > 0 && user) {
        const hasRequiredRole = requiredRoles.some((role) =>
          user.roles.includes(role),
        );
        if (!hasRequiredRole) {
          navigate({ to: "/" }); // was /forbidden
          return;
        }
      }

      setIsChecking(false);
    };

    verifyAuth();
  }, [isAuthenticated, user, accessToken, navigate, requiredRoles]);

  if (isChecking) {
    return (
      <div className="min-h-screen flex items-center justify-center bg-gray-50 dark:bg-gray-900">
        <div className="text-center">
          <Loader2 className="w-8 h-8 animate-spin text-blue-600 mx-auto mb-4" />
          <p className="text-gray-600 dark:text-gray-300">
            Verifying authentication...
          </p>
        </div>
      </div>
    );
  }

  return <>{children}</>;
};

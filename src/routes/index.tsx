//
//  campus-pilot
//  index.tsx
//
//  Created by Ngonidzashe Mangudya on 26/09/2025.
//  Copyright (c) 2025 Codecraft Solutions
//

import { createFileRoute, redirect } from "@tanstack/react-router";
import { bootstrapService } from "../modules/configs";

const getAuthStatus = (): boolean => {
  try {
    const authData = localStorage.getItem("campuspilot_auth");
    if (authData) {
      const parsed = JSON.parse(authData);
      return parsed.state?.isAuthenticated || false;
    }
  } catch {
    return false;
  }
  return false;
};

export const Route = createFileRoute("/")({
  beforeLoad: async () => {
    try {
      const response = await bootstrapService.checkStatus();
      if (response.success && response.data) {
        const state = response.data.state;

        switch (state) {
          case "Uninitialized":
            throw redirect({ to: "/setup/school" });
          case "SchoolConfigured":
            throw redirect({ to: "/setup/admin" });
          case "Ready":
            const isAuthenticated = getAuthStatus();
            if (isAuthenticated) {
              throw redirect({ to: "/admin" });
            } else {
              throw redirect({ to: "/login" });
            }
          default:
            throw redirect({ to: "/boot" });
        }
      } else {
        throw redirect({ to: "/boot" });
      }
    } catch (error) {
      if (error && typeof error === "object" && "redirect" in error) {
        throw error;
      }
      throw redirect({ to: "/boot" });
    }
  },
  component: () => null,
});

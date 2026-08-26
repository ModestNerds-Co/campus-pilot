//
//  campus-pilot
//  index.tsx
//
//  Created by Ngonidzashe Mangudya on 26/09/2025.
//  Copyright (c) 2025 Codecraft Solutions
//

import { createFileRoute, redirect } from "@tanstack/react-router";
import { bootstrapService, type BootstrapState } from "../modules/configs";

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
    let state: BootstrapState;
    try {
      const response = await bootstrapService.checkStatus();
      if (!response.success || !response.data) throw new Error("Bootstrap status unavailable");
      state = response.data.state;
    } catch {
      throw redirect({ to: "/boot" });
    }

    if (state === "Uninitialized") throw redirect({ to: "/setup/school" });
    if (state === "SchoolConfigured") throw redirect({ to: "/setup/admin" });
    throw redirect({ to: getAuthStatus() ? "/home" : "/login" });
  },
  component: () => null,
});

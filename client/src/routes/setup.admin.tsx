//
//  campus-pilot
//  setup.admin.tsx - Admin Setup Route
//
//  Created by Ngonidzashe Mangudya on 26/09/2025.
//  Copyright (c) 2025 Codecraft Solutions
//

import { createFileRoute, redirect } from "@tanstack/react-router";

import { AdminSetupScreen, bootstrapService, type BootstrapState } from "../modules/configs";

export const Route = createFileRoute("/setup/admin")({
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
    if (state === "Ready") throw redirect({ to: "/" });
  },
  component: AdminSetupScreen,
});

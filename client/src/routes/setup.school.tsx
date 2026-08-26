//
//  campus-pilot
//  setup.school.tsx - School Setup Route
//
//  Created by Ngonidzashe Mangudya on 26/09/2025.
//  Copyright (c) 2025 Codecraft Solutions
//

import { createFileRoute, redirect } from "@tanstack/react-router";

import { bootstrapService, SchoolSetupScreen, type BootstrapState } from "../modules/configs";

export const Route = createFileRoute("/setup/school")({
  beforeLoad: async () => {
    let state: BootstrapState;
    try {
      const response = await bootstrapService.checkStatus();
      if (!response.success || !response.data) throw new Error("Bootstrap status unavailable");
      state = response.data.state;
    } catch {
      throw redirect({ to: "/boot" });
    }
    if (state === "SchoolConfigured") throw redirect({ to: "/setup/admin" });
    if (state === "Ready") throw redirect({ to: "/" });
  },
  component: SchoolSetupScreen,
});

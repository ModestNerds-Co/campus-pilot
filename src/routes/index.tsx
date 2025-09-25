//
//  campus-pilot
//  index.tsx
//
//  Created by Ngonidzashe Mangudya on 26/09/2025.
//  Copyright (c) 2025 Codecraft Solutions
//

import { createFileRoute, redirect } from "@tanstack/react-router";
import { bootstrapService } from "../modules/configs";

export const Route = createFileRoute("/")({
  beforeLoad: async () => {
    try {
      const response = await bootstrapService.checkStatus();
      if (response.success && response.data) {
        const state = response.data.state;

        // Redirect based on bootstrap state
        switch (state) {
          case "Uninitialized":
            throw redirect({ to: "/setup/school" });
          case "SchoolConfigured":
            throw redirect({ to: "/setup/admin" });
          case "Ready":
            throw redirect({ to: "/login" });
          default:
            throw redirect({ to: "/boot" });
        }
      } else {
        throw redirect({ to: "/boot" });
      }
    } catch (error) {
      // If it's already a redirect, re-throw it
      if (error && typeof error === "object" && "redirect" in error) {
        throw error;
      }
      // For other errors, go to boot screen
      throw redirect({ to: "/boot" });
    }
  },
  component: () => null, // This should never render
});

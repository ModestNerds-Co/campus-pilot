//
//  campus-pilot
//  dashboard.tsx - Legacy Dashboard Route (Redirects to module launcher)
//
//  Created by Ngonidzashe Mangudya on 02/10/2025.
//  Copyright (c) 2025 Codecraft Solutions
//

import { createFileRoute, redirect } from "@tanstack/react-router";

export const Route = createFileRoute("/dashboard")({
  beforeLoad: () => {
    throw redirect({ to: "/home" });
  },
  component: () => null,
});

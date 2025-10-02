//
//  campus-pilot
//  admin/index.tsx - Admin Dashboard Route
//
//  Created by Ngonidzashe Mangudya on 02/10/2025.
//  Copyright (c) 2025 Codecraft Solutions
//

import { createFileRoute } from "@tanstack/react-router";
import { AdminDashboard } from "../../modules/admin";

export const Route = createFileRoute("/admin/")({
  component: AdminDashboard,
});

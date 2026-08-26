//
//  campus-pilot
//  admin.tsx - Admin Layout Route
//
//  Created by Ngonidzashe Mangudya on 02/10/2025.
//  Copyright (c) 2025 Codecraft Solutions
//

import { createFileRoute, Outlet } from "@tanstack/react-router";
import { ProtectedRoute } from "../components/protected-route";
import { AdminLayout } from "../modules/admin";

export const Route = createFileRoute("/admin")({
  component: () => (
    <ProtectedRoute requiredModule="administration" requiredPermission="administration:view">
      <AdminLayout>
        <Outlet />
      </AdminLayout>
    </ProtectedRoute>
  ),
});

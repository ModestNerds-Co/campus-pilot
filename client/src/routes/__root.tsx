//
//  campus-pilot
//  __root.tsx
//
//  Created by Ngonidzashe Mangudya on 26/09/2025.
//  Copyright (c) 2025 Codecraft Solutions
//

import { createRootRoute, Outlet } from "@tanstack/react-router";
import { TanStackRouterDevtools } from "@tanstack/router-devtools";

export const Route = createRootRoute({
  component: RootComponent,
});

function RootComponent() {
  return (
    <>
      <Outlet />

      {/* Dev tools in development */}
      {process.env.NODE_ENV === "development" && (
        <TanStackRouterDevtools position="bottom-left" />
      )}
    </>
  );
}

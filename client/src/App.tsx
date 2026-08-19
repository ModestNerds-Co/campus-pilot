//
//  campus-pilot
//  App.tsx
//
//  Created by Ngonidzashe Mangudya on 21/08/2025.
//  Copyright (c) 2025 Codecraft Solutions
//

import React from "react";
import { RouterProvider, createRouter } from "@tanstack/react-router";
import { ChangelogModal } from "./components/changelog-modal";
import { useVersionCheck } from "./hooks/use-version-check";

// Import the generated route tree
import { routeTree } from "./routeTree.gen";

// Create a new router instance
const router = createRouter({ routeTree });

// Register the router instance for type safety
declare module "@tanstack/react-router" {
  interface Register {
    router: typeof router;
  }
}

export const App: React.FC = () => {
  const {
    showChangelog,
    newChanges,
    currentVersion,
    markVersionAsSeen,
    closeChangelog,
  } = useVersionCheck();

  const handleCloseChangelog = () => {
    markVersionAsSeen();
    closeChangelog();
  };

  return (
    <>
      <RouterProvider router={router} />
      <ChangelogModal
        isOpen={showChangelog}
        onClose={handleCloseChangelog}
        entries={newChanges}
        currentVersion={currentVersion}
      />
    </>
  );
};

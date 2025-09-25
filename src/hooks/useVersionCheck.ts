//
//  campus-pilot
//  useVersionCheck.tsx
//
//  Created by Ngonidzashe Mangudya on 28/08/2025.
//  Copyright (c) 2025 Codecraft Solutions
//

import { useState, useEffect } from "react";
import {
  APP_VERSION,
  hasNewVersion,
  getChangesSince,
  ChangelogEntry,
} from "../lib/version";

const LAST_SEEN_VERSION_KEY = "campus_pilot_last_seen_version";

export const useVersionCheck = () => {
  const [showChangelog, setShowChangelog] = useState(false);
  const [newChanges, setNewChanges] = useState<ChangelogEntry[]>([]);
  const [isFirstTime, setIsFirstTime] = useState(false);

  useEffect(() => {
    const checkVersion = () => {
      const lastSeenVersion = localStorage.getItem(LAST_SEEN_VERSION_KEY);

      // First time user
      if (!lastSeenVersion) {
        setIsFirstTime(true);
        // Don't show changelog on first visit, just set the current version
        localStorage.setItem(LAST_SEEN_VERSION_KEY, APP_VERSION);
        return;
      }

      // Check if there's a new version
      if (hasNewVersion(lastSeenVersion)) {
        const changes = getChangesSince(lastSeenVersion);
        setNewChanges(changes);
        setShowChangelog(true);
      }
    };

    // Small delay to ensure app is fully loaded
    const timer = setTimeout(checkVersion, 1000);
    return () => clearTimeout(timer);
  }, []);

  const markVersionAsSeen = () => {
    localStorage.setItem(LAST_SEEN_VERSION_KEY, APP_VERSION);
    setShowChangelog(false);
  };

  const showChangelogManually = () => {
    // Show all recent changes when manually opened
    const changes = getChangesSince("1.0.0");
    setNewChanges(changes);
    setShowChangelog(true);
  };

  return {
    showChangelog,
    newChanges,
    isFirstTime,
    currentVersion: APP_VERSION,
    markVersionAsSeen,
    showChangelogManually,
    closeChangelog: () => setShowChangelog(false),
  };
};

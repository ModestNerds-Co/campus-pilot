//
//  campus-pilot
//  version.ts
//
//  Created by Ngonidzashe Mangudya on 28/08/2025.
//  Copyright (c) 2025 Codecraft Solutions
//

export const APP_VERSION = "1.0.0";

export interface ChangelogEntry {
  version: string;
  date: string;
  changes: {
    new: string[];
    fixed: string[];
    improved: string[];
    breaking?: string[];
  };
}

export const CHANGELOG: ChangelogEntry[] = [];

export const getChangesSince = (lastVersion: string): ChangelogEntry[] => {
  const lastVersionIndex = CHANGELOG.findIndex(
    (entry) => entry.version === lastVersion,
  );

  // If version not found, return all entries
  if (lastVersionIndex === -1) {
    return CHANGELOG;
  }

  // Return all entries newer than the last seen version
  return CHANGELOG.slice(0, lastVersionIndex);
};

export const getLatestVersion = (): string => {
  return CHANGELOG[0]?.version || APP_VERSION;
};

export const hasNewVersion = (lastSeenVersion: string | null): boolean => {
  if (!lastSeenVersion) return true;
  return lastSeenVersion !== APP_VERSION;
};

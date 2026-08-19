//
//  campus-pilot
//  useKeyboardShortcuts.ts
//
//  Created by Ngonidzashe Mangudya on 28/08/2025.
//  Copyright (c) 2025 Codecraft Solutions
//

import { useEffect } from "react";
import { useNavigate } from "@tanstack/react-router";

export interface KeyboardShortcut {
  key: string;
  ctrlKey?: boolean;
  metaKey?: boolean;
  shiftKey?: boolean;
  altKey?: boolean;
  action: () => void;
  description: string;
}

export function useKeyboardShortcuts(shortcuts: KeyboardShortcut[]) {
  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      const matchingShortcut = shortcuts.find((shortcut) => {
        return (
          shortcut.key.toLowerCase() === event.key.toLowerCase() &&
          (shortcut.ctrlKey || false) === event.ctrlKey &&
          (shortcut.metaKey || false) === event.metaKey &&
          (shortcut.shiftKey || false) === event.shiftKey &&
          (shortcut.altKey || false) === event.altKey
        );
      });

      if (matchingShortcut) {
        event.preventDefault();
        matchingShortcut.action();
      }
    };

    document.addEventListener("keydown", handleKeyDown);
    return () => document.removeEventListener("keydown", handleKeyDown);
  }, [shortcuts]);
}

export function useGlobalKeyboardShortcuts() {
  const navigate = useNavigate();

  const shortcuts: KeyboardShortcut[] = [
    // Tab navigation
    {
      key: "t",
      metaKey: true,
      action: () => navigate({ to: "/" } as any),
      description: "New Tab",
    },
  ];

  useKeyboardShortcuts(shortcuts);

  return shortcuts;
}

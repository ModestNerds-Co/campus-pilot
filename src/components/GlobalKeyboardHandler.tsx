//
//  campus-pilot
//  GlobalKeyboardHandler.tsx
//
//  Created by Ngonidzashe Mangudya on 28/08/2025.
//  Copyright (c) 2025 Codecraft Solutions
//

import { useEffect } from "react";
import { useGlobalKeyboardShortcuts } from "../hooks/useKeyboardShortcuts";
import { useCommandPalette } from "../hooks/useCommandPalette";
import { CommandPalette } from "./CommandPalette";

export function GlobalKeyboardHandler() {
  // Initialize global keyboard shortcuts inside router context
  useGlobalKeyboardShortcuts();

  const { isOpen, closePalette } = useCommandPalette();

  return <CommandPalette isOpen={isOpen} onClose={closePalette} />;
}

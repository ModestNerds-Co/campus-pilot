//
//  campus-pilot
//  useCommandPalette.ts
//
//  Created by Ngonidzashe Mangudya on 28/08/2025.
//  Copyright (c) 2025 Codecraft Solutions
//

import { useState, useEffect } from "react";

export function useCommandPalette() {
  const [isOpen, setIsOpen] = useState(false);

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      // Cmd+K or Ctrl+K to open command palette
      if ((e.metaKey || e.ctrlKey) && e.key === "k") {
        e.preventDefault();
        setIsOpen((prev) => !prev);
      }
    };

    document.addEventListener("keydown", handleKeyDown);
    return () => document.removeEventListener("keydown", handleKeyDown);
  }, []);

  const openPalette = () => setIsOpen(true);
  const closePalette = () => setIsOpen(false);

  return {
    isOpen,
    openPalette,
    closePalette,
  };
}

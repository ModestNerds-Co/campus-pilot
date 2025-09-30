//
//  campus-pilot
//  ThemeToggle.tsx
//
//  Created by Ngonidzashe Mangudya on 21/08/2025.
//  Copyright (c) 2025 Codecraft Solutions
//

import { Moon, Sun } from "lucide-react";
import { useEffect, useState } from "react";

export function ThemeToggle() {
  const [theme, setTheme] = useState<"light" | "dark">("light");

  useEffect(() => {
    // Get initial theme from localStorage or system preference
    const savedTheme = localStorage.getItem("tgpatcher-theme") as
      | "light"
      | "dark"
      | null;
    const systemTheme = window.matchMedia("(prefers-color-scheme: dark)")
      .matches
      ? "dark"
      : "light";
    const initialTheme = savedTheme || systemTheme;

    setTheme(initialTheme);
    document.documentElement.classList.remove("light", "dark");
    document.documentElement.classList.add(initialTheme);
  }, []);

  const toggleTheme = () => {
    const newTheme = theme === "light" ? "dark" : "light";
    setTheme(newTheme);

    // Update DOM
    document.documentElement.classList.remove("light", "dark");
    document.documentElement.classList.add(newTheme);

    // Save to localStorage
    localStorage.setItem("tgpatcher-theme", newTheme);
  };

  return (
    <button
      onClick={toggleTheme}
      className="p-1.5 rounded-md hover:bg-muted transition-colors"
      aria-label="Toggle theme"
    >
      {theme === "light" ? (
        <Moon className="w-4 h-4" />
      ) : (
        <Sun className="w-4 h-4" />
      )}
    </button>
  );
}

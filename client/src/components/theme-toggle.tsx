//
//  campus-pilot
//  ThemeToggle — shim (hotspot fix)
//  Legacy 2-way toggle on `tgpatcher-theme` is retired. This file now
//  re-exports the canonical ThemeToggle from lib/theme.tsx which owns
//  the 3-way light/dark/system contract on `campuspilot-theme`.
//  Keeping this file as a re-export avoids breaking existing imports
//  while unifying behaviour + storage key.
//

export { ThemeToggle } from "@/lib/theme";
export { ThemeToggle as default } from "@/lib/theme";

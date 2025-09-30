//
//  campus-pilot
//  CommandPalette.tsx
//
//  Created by Ngonidzashe Mangudya on 28/08/2025.
//  Copyright (c) 2025 Codecraft Solutions
//

import { useState, useEffect, useRef } from "react";
import {
  Search,
  X,
  Hash,
  FileText,
  User,
  Fingerprint,
  Target,
  Users,
  CreditCard,
  History,
  Server,
  Building,
  Plus,
} from "lucide-react";
import { useNavigate } from "@tanstack/react-router";
import { useUIStore } from "../stores/uiStore";
import { cn } from "../lib/utils";

interface Command {
  id: string;
  label: string;
  description?: string;
  icon?: React.ComponentType<any>;
  action: () => void;
  group: string;
  keywords?: string[];
}

interface CommandPaletteProps {
  isOpen: boolean;
  onClose: () => void;
}

export function CommandPalette({ isOpen, onClose }: CommandPaletteProps) {
  const [query, setQuery] = useState("");
  const [selectedIndex, setSelectedIndex] = useState(0);
  const navigate = useNavigate();
  const { tabs, activeTabId, setActiveTab, removeTab } = useUIStore();
  const inputRef = useRef<HTMLInputElement>(null);

  // Generate commands based on current state
  const commands: Command[] = [
    // Navigation
    {
      id: "new-tab",
      label: "New Tab",
      description: "Open search page",
      icon: Plus,
      action: () => {
        console.log("Opening new tab");
        navigate({ to: "/" }).catch(console.error);
      },
      group: "Navigation",
      keywords: ["new", "search", "open"],
    },
    {
      id: "close-tab",
      label: "Close Current Tab",
      description: "Close the currently active tab",
      icon: X,
      action: () => {
        console.log("Closing tab:", activeTabId);
        if (activeTabId) {
          const tab = tabs.find((t) => t.id === activeTabId);
          if (tab?.isDirty) {
            const confirmed = window.confirm(
              "You have unsaved changes. Are you sure you want to close this tab?",
            );
            if (!confirmed) return;
          }
          removeTab(activeTabId);
        }
      },
      group: "Navigation",
      keywords: ["close", "remove"],
    },
    // Tab switching
    ...tabs.map((tab, index) => ({
      id: `switch-tab-${tab.id}`,
      label: `Switch to ${tab.reference}`,
      description: `Application ID: ${tab.tgapplicationid}`,
      icon: FileText,
      action: () => {
        console.log("Switching to tab:", tab.reference, tab.tgapplicationid);
        console.log("Current active tab before:", activeTabId);
        setActiveTab(tab.id);
        console.log("Set active tab to:", tab.id);
        navigate({
          to: "/case/$applicationId",
          params: { applicationId: tab.tgapplicationid.toString() },
        })
          .then(() => {
            console.log("Navigation completed");
          })
          .catch((error) => {
            console.error("Navigation failed:", error);
          });
      },
      group: "Open Tabs",
      keywords: [
        "tab",
        "switch",
        tab.reference,
        tab.tgapplicationid.toString(),
      ],
    })),
    // Sub-tab navigation (if we're on a case page)
    ...getSubTabCommands(),
  ];

  function getSubTabCommands(): Command[] {
    if (!activeTabId) return [];

    const activeTab = tabs.find((t) => t.id === activeTabId);
    if (!activeTab) return [];

    // Common sub-tabs for both person and organization
    const commonTabs = [
      { id: "application", label: "Application Details", icon: FileText },
      { id: "workflow", label: "Workflow History", icon: History },
      { id: "payment-details", label: "Payment Details", icon: CreditCard },
    ];

    // Person-specific tabs (would need entitytype from somewhere)
    const personTabs = [
      { id: "person", label: "Person Details", icon: User },
      { id: "biometrics", label: "Biometrics", icon: Fingerprint },
      { id: "hitlist-matches", label: "Hitlist Matches", icon: Target },
      { id: "identities", label: "Identities", icon: CreditCard },
      { id: "relations", label: "Relations", icon: Users },
      { id: "documents", label: "Documents", icon: FileText },
      { id: "perso-logs", label: "Perso Interface Logs", icon: Server },
    ];

    // Organization-specific tabs
    const organizationTabs = [
      { id: "organization", label: "Organization Details", icon: Building },
      {
        id: "organization-identities",
        label: "Organization Identities",
        icon: CreditCard,
      },
      {
        id: "organization-contacts",
        label: "Organization Contacts",
        icon: Users,
      },
      {
        id: "organization-documents",
        label: "Organization Documents",
        icon: FileText,
      },
    ];

    // For now, include all possible tabs (we could enhance this by detecting entity type)
    const allSubTabs = [...commonTabs, ...personTabs, ...organizationTabs];

    return allSubTabs.map((subTab) => ({
      id: `subtab-${subTab.id}`,
      label: `Go to ${subTab.label}`,
      description: `Navigate to ${subTab.label} sub-tab`,
      icon: subTab.icon,
      action: () => {
        console.log(
          "Navigating to sub-tab:",
          subTab.id,
          "for app:",
          activeTab.tgapplicationid,
        );
        navigate({
          to: "/case/$applicationId",
          params: { applicationId: activeTab.tgapplicationid.toString() },
          search: { subtab: subTab.id },
        } as any);
      },
      group: "Sub-tabs",
      keywords: [subTab.label.toLowerCase(), "tab", "navigate"],
    }));
  }

  // Filter commands based on query
  const filteredCommands = commands.filter((command) => {
    const searchText = query.toLowerCase();
    return (
      command.label.toLowerCase().includes(searchText) ||
      command.description?.toLowerCase().includes(searchText) ||
      command.keywords?.some((keyword) =>
        keyword.toLowerCase().includes(searchText),
      )
    );
  });

  // Group filtered commands
  const groupedCommands = filteredCommands.reduce(
    (groups, command) => {
      if (!groups[command.group]) {
        groups[command.group] = [];
      }
      groups[command.group].push(command);
      return groups;
    },
    {} as Record<string, Command[]>,
  );

  // Handle keyboard navigation
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (!isOpen) return;

      switch (e.key) {
        case "ArrowDown":
          e.preventDefault();
          setSelectedIndex((prev) => {
            const newIndex = (prev + 1) % filteredCommands.length;
            // Scroll to selected item
            setTimeout(() => {
              const element = document.querySelector(
                `[data-command-index="${newIndex}"]`,
              );
              element?.scrollIntoView({ behavior: "smooth", block: "nearest" });
            }, 0);
            return newIndex;
          });
          break;
        case "ArrowUp":
          e.preventDefault();
          setSelectedIndex((prev) => {
            const newIndex =
              prev === 0 ? filteredCommands.length - 1 : prev - 1;
            // Scroll to selected item
            setTimeout(() => {
              const element = document.querySelector(
                `[data-command-index="${newIndex}"]`,
              );
              element?.scrollIntoView({ behavior: "smooth", block: "nearest" });
            }, 0);
            return newIndex;
          });
          break;
        case "Enter":
          e.preventDefault();
          console.log(
            "Enter pressed, executing command:",
            filteredCommands[selectedIndex]?.label,
          );
          if (filteredCommands[selectedIndex]) {
            try {
              filteredCommands[selectedIndex].action();
              onClose();
            } catch (error) {
              console.error("Command execution failed:", error);
            }
          }
          break;
        case "Escape":
          e.preventDefault();
          onClose();
          break;
      }
    };

    document.addEventListener("keydown", handleKeyDown);
    return () => document.removeEventListener("keydown", handleKeyDown);
  }, [isOpen, selectedIndex, filteredCommands, onClose]);

  // Reset state when opening
  useEffect(() => {
    if (isOpen) {
      setQuery("");
      setSelectedIndex(0);
      setTimeout(() => inputRef.current?.focus(), 100);
    }
  }, [isOpen]);

  // Update selected index when filtered commands change
  useEffect(() => {
    setSelectedIndex(0);
  }, [query]);

  if (!isOpen) return null;

  return (
    <div className="fixed inset-0 z-50 bg-black/50 flex items-start justify-center pt-32">
      <div className="bg-white rounded-lg shadow-xl border border-gray-200 w-full max-w-2xl max-h-96 overflow-hidden">
        {/* Search Input */}
        <div className="flex items-center gap-3 p-4 border-b border-gray-200">
          <Search className="w-5 h-5 text-gray-400" />
          <input
            ref={inputRef}
            type="text"
            placeholder="Type a command or search..."
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            className="flex-1 bg-transparent outline-none border-none focus:ring-0 focus:border-none focus:outline-none text-gray-900"
            style={{ boxShadow: "none", border: "none" }}
          />
          <button onClick={onClose} className="p-1 hover:bg-gray-100 rounded">
            <X className="w-4 h-4 text-gray-400" />
          </button>
        </div>

        {/* Commands List */}
        <div className="overflow-y-auto max-h-80">
          {Object.keys(groupedCommands).length === 0 ? (
            <div className="p-8 text-center text-gray-500">
              <Search className="w-8 h-8 mx-auto mb-2 text-gray-300" />
              <p>No commands found</p>
            </div>
          ) : (
            Object.entries(groupedCommands).map(([group, groupCommands]) => (
              <div key={group}>
                <div className="px-4 py-2 text-xs font-semibold text-gray-500 bg-gray-50 border-b border-gray-100">
                  {group}
                </div>
                {groupCommands.map((command, index) => {
                  const globalIndex = filteredCommands.indexOf(command);
                  const Icon = command.icon;

                  return (
                    <button
                      key={command.id}
                      data-command-index={globalIndex}
                      onClick={() => {
                        console.log("Clicked command:", command.label);
                        try {
                          command.action();
                          onClose();
                        } catch (error) {
                          console.error("Command click failed:", error);
                        }
                      }}
                      className={cn(
                        "w-full flex items-center gap-3 p-3 text-left hover:bg-gray-50 transition-colors border-b border-gray-50 last:border-b-0",
                        globalIndex === selectedIndex &&
                          "bg-blue-50 text-blue-600",
                      )}
                    >
                      {Icon && <Icon className="w-4 h-4 text-gray-400" />}
                      <div className="flex-1 min-w-0">
                        <div className="font-medium text-sm">
                          {command.label}
                        </div>
                        {command.description && (
                          <div className="text-xs text-gray-500 truncate">
                            {command.description}
                          </div>
                        )}
                      </div>
                    </button>
                  );
                })}
              </div>
            ))
          )}
        </div>

        {/* Footer */}
        <div className="p-3 border-t border-gray-200 bg-gray-50 text-xs text-gray-500">
          <div className="flex items-center justify-between">
            <span>Use ↑↓ to navigate, Enter to select</span>
            <span>ESC to close</span>
          </div>
        </div>
      </div>
    </div>
  );
}

import { create } from 'zustand';
import { devtools, persist } from 'zustand/middleware';
import type { TabState, UIState, ConnectionStatus, StagedChanges } from '../types/database';

interface UIStore extends UIState {
  // Tab management
  addTab: (tab: TabState) => void;
  removeTab: (tabId: string) => void;
  setActiveTab: (tabId: string | null) => void;
  updateTab: (tabId: string, updates: Partial<TabState>) => void;
  getTab: (tabId: string) => TabState | undefined;

  // Staged changes management
  setStagedChanges: (tabId: string, changes: StagedChanges) => void;
  clearStagedChanges: (tabId: string) => void;
  updateStagedChanges: (tabId: string, section: keyof StagedChanges, data: any) => void;

  // Connection management
  setConnectionStatus: (status: ConnectionStatus) => void;

  // UI preferences
  toggleTheme: () => void;
  toggleRightDrawer: () => void;
  setRightDrawer: (open: boolean) => void;

  // Utility functions
  createTabId: () => string;
  markTabDirty: (tabId: string, isDirty: boolean) => void;
  hasUnsavedChanges: () => boolean;
}

export const useUIStore = create<UIStore>()(
  devtools(
    persist(
      (set, get) => ({
        // Initial state
        activeTabId: null,
        tabs: [],
        rightDrawerOpen: false,
        theme: 'light',
        connectionStatus: {
          status: 'connecting',
        },

        // Tab management
        addTab: (tab) =>
          set((state) => {
            // Check if tab for this application already exists
            const existingTab = state.tabs.find(t => t.tgapplicationid === tab.tgapplicationid);
            if (existingTab) {
              // Switch to existing tab instead of creating new one
              return { ...state, activeTabId: existingTab.id };
            }

            // Add new tab and make it active
            return {
              tabs: [...state.tabs, tab],
              activeTabId: tab.id,
            };
          }),

        removeTab: (tabId) =>
          set((state) => {
            const filteredTabs = state.tabs.filter((t) => t.id !== tabId);
            let newActiveTabId = state.activeTabId;

            // If removing active tab, switch to another tab
            if (state.activeTabId === tabId) {
              const currentIndex = state.tabs.findIndex((t) => t.id === tabId);
              if (filteredTabs.length > 0) {
                // Try to activate the tab at the same position, or the previous one
                const newIndex = Math.min(currentIndex, filteredTabs.length - 1);
                newActiveTabId = filteredTabs[newIndex].id;
              } else {
                newActiveTabId = null;
              }
            }

            return {
              tabs: filteredTabs,
              activeTabId: newActiveTabId,
            };
          }),

        setActiveTab: (tabId) =>
          set(() => ({
            activeTabId: tabId,
          })),

        updateTab: (tabId, updates) =>
          set((state) => ({
            tabs: state.tabs.map((tab) =>
              tab.id === tabId ? { ...tab, ...updates } : tab
            ),
          })),

        getTab: (tabId) => {
          const state = get();
          return state.tabs.find((t) => t.id === tabId);
        },

        // Staged changes management
        setStagedChanges: (tabId, changes) =>
          set((state) => ({
            tabs: state.tabs.map((tab) =>
              tab.id === tabId
                ? {
                    ...tab,
                    stagedChanges: changes,
                    isDirty: true,
                  }
                : tab
            ),
          })),

        clearStagedChanges: (tabId) =>
          set((state) => ({
            tabs: state.tabs.map((tab) =>
              tab.id === tabId
                ? {
                    ...tab,
                    stagedChanges: {},
                    isDirty: false,
                  }
                : tab
            ),
          })),

        updateStagedChanges: (tabId, section, data) =>
          set((state) => ({
            tabs: state.tabs.map((tab) => {
              if (tab.id !== tabId) return tab;

              const updatedChanges = { ...tab.stagedChanges };

              if (section === 'application' || section === 'person' || section === 'workflowHistory') {
                updatedChanges[section] = { ...updatedChanges[section], ...data };
              } else if (section === 'biometrics' || section === 'identities') {
                // Handle collections with updates, creates, deletes
                updatedChanges[section] = {
                  updates: new Map(updatedChanges[section]?.updates || new Map()),
                  creates: [...(updatedChanges[section]?.creates || [])],
                  deletes: [...(updatedChanges[section]?.deletes || [])],
                  ...data,
                };
              }

              // Check if there are any actual changes
              const hasChanges = Object.keys(updatedChanges).some((key) => {
                const value = updatedChanges[key as keyof StagedChanges];
                if (!value) return false;

                if (typeof value === 'object' && 'updates' in value) {
                  // Check collections
                  return (
                    value.updates.size > 0 ||
                    value.creates.length > 0 ||
                    value.deletes.length > 0
                  );
                }

                // Check objects
                return Object.keys(value).length > 0;
              });

              return {
                ...tab,
                stagedChanges: updatedChanges,
                isDirty: hasChanges,
              };
            }),
          })),

        // Connection management
        setConnectionStatus: (status) =>
          set(() => ({
            connectionStatus: status,
          })),

        // UI preferences
        toggleTheme: () =>
          set((state) => {
            const newTheme = state.theme === 'light' ? 'dark' : 'light';
            // Apply theme to document
            if (typeof document !== 'undefined') {
              document.documentElement.classList.remove('light', 'dark');
              document.documentElement.classList.add(newTheme);
            }
            return { theme: newTheme };
          }),

        toggleRightDrawer: () =>
          set((state) => ({
            rightDrawerOpen: !state.rightDrawerOpen,
          })),

        setRightDrawer: (open) =>
          set(() => ({
            rightDrawerOpen: open,
          })),

        // Utility functions
        createTabId: () => {
          return `tab-${Date.now()}-${Math.random().toString(36).substr(2, 9)}`;
        },

        markTabDirty: (tabId, isDirty) =>
          set((state) => ({
            tabs: state.tabs.map((tab) =>
              tab.id === tabId ? { ...tab, isDirty } : tab
            ),
          })),

        hasUnsavedChanges: () => {
          const state = get();
          return state.tabs.some((tab) => tab.isDirty);
        },
      }),
      {
        name: 'tgpatcher-ui-store',
        partialize: (state) => ({
          theme: state.theme,
          rightDrawerOpen: state.rightDrawerOpen,
        }),
      }
    )
  )
);

// Helper hook to get current active tab
export const useActiveTab = () => {
  const activeTabId = useUIStore((state) => state.activeTabId);
  const tabs = useUIStore((state) => state.tabs);
  return tabs.find((tab) => tab.id === activeTabId);
};

// Helper hook to check for unsaved changes before navigation
export const useUnsavedChangesWarning = () => {
  const hasUnsavedChanges = useUIStore((state) => state.hasUnsavedChanges);

  if (typeof window !== 'undefined') {
    window.addEventListener('beforeunload', (e) => {
      if (hasUnsavedChanges()) {
        e.preventDefault();
        e.returnValue = 'You have unsaved changes. Are you sure you want to leave?';
      }
    });
  }
};

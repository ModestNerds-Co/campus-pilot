//
//  campus-pilot
//  auth-store.ts - Authentication State Management
//
//  Created by Ngonidzashe Mangudya on 02/10/2025.
//  Copyright (c) 2025 Codecraft Solutions
//

import { create } from "zustand";
import { persist } from "zustand/middleware";
import { authService, type User } from "../lib/auth-service";

interface AuthState {
  user: User | null;
  accessToken: string | null;
  refreshToken: string | null;
  expiresAt: number | null;
  isAuthenticated: boolean;
  isLoading: boolean;
  error: string | null;
}

interface AuthActions {
  login: (email: string, password: string) => Promise<boolean>;
  logout: () => Promise<void>;
  refreshAccessToken: () => Promise<boolean>;
  clearError: () => void;
  setUser: (user: User) => void;
  checkAuth: () => Promise<boolean>;
}

type AuthStore = AuthState & AuthActions;

const STORAGE_KEY = "campuspilot_auth";

export const useAuthStore = create<AuthStore>()(
  persist(
    (set, get) => ({
      user: null,
      accessToken: null,
      refreshToken: null,
      expiresAt: null,
      isAuthenticated: false,
      isLoading: false,
      error: null,

      login: async (email: string, password: string) => {
        set({ isLoading: true, error: null });

        try {
          const response = await authService.login({ email, password });

          if (response.success && response.data) {
            const { access_token, refresh_token, expires_in, user } =
              response.data;
            const expiresAt = Date.now() + expires_in * 1000;

            set({
              user,
              accessToken: access_token,
              refreshToken: refresh_token,
              expiresAt,
              isAuthenticated: true,
              isLoading: false,
              error: null,
            });

            return true;
          } else {
            set({
              error: response.message || "Login failed",
              isLoading: false,
            });
            return false;
          }
        } catch (error) {
          const errorMessage =
            error instanceof Error ? error.message : "Login failed";
          set({ error: errorMessage, isLoading: false });
          return false;
        }
      },

      logout: async () => {
        const { refreshToken } = get();

        try {
          if (refreshToken) {
            await authService.logout(refreshToken);
          }
        } catch (error) {
          console.error("Logout error:", error);
        } finally {
          set({
            user: null,
            accessToken: null,
            refreshToken: null,
            expiresAt: null,
            isAuthenticated: false,
            error: null,
          });
        }
      },

      refreshAccessToken: async () => {
        const { refreshToken, expiresAt } = get();

        if (!refreshToken) {
          return false;
        }

        if (expiresAt && Date.now() < expiresAt - 60000) {
          return true;
        }

        try {
          const response = await authService.refresh(refreshToken);

          if (response.success && response.data) {
            const { access_token, refresh_token, expires_in } = response.data;
            const newExpiresAt = Date.now() + expires_in * 1000;

            set({
              accessToken: access_token,
              refreshToken: refresh_token,
              expiresAt: newExpiresAt,
            });

            return true;
          } else {
            await get().logout();
            return false;
          }
        } catch (error) {
          console.error("Token refresh error:", error);
          await get().logout();
          return false;
        }
      },

      checkAuth: async () => {
        const { accessToken, refreshToken, expiresAt } = get();

        if (!accessToken || !refreshToken) {
          return false;
        }

        if (expiresAt && Date.now() >= expiresAt) {
          const refreshed = await get().refreshAccessToken();
          if (!refreshed) {
            return false;
          }
        }

        try {
          const response = await authService.getMe();
          if (response.success && response.data) {
            set({ user: response.data, isAuthenticated: true });
            return true;
          } else {
            await get().logout();
            return false;
          }
        } catch (error) {
          console.error("Auth check error:", error);
          await get().logout();
          return false;
        }
      },

      clearError: () => set({ error: null }),

      setUser: (user: User) => set({ user }),
    }),
    {
      name: STORAGE_KEY,
      partialize: (state) => ({
        user: state.user,
        accessToken: state.accessToken,
        refreshToken: state.refreshToken,
        expiresAt: state.expiresAt,
        isAuthenticated: state.isAuthenticated,
      }),
    },
  ),
);

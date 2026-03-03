import { create } from "zustand";
import {
  saveTokens,
  getAccessToken,
  getRefreshToken,
  clearTokens,
} from "@/utils/secureStorage";

interface AuthState {
  accessToken: string | null;
  refreshToken: string | null;
  userId: number | null;
  isAuthenticated: boolean;
  isLoading: boolean;
  needsTotp: boolean;
  pendingUserId: number | null;

  setTokens: (accessToken: string, refreshToken: string) => Promise<void>;
  setUserId: (userId: number) => void;
  setNeedsTotp: (userId: number) => void;
  clearTotp: () => void;
  logout: () => Promise<void>;
  hydrate: () => Promise<void>;
}

export const useAuthStore = create<AuthState>((set) => ({
  accessToken: null,
  refreshToken: null,
  userId: null,
  isAuthenticated: false,
  isLoading: true,
  needsTotp: false,
  pendingUserId: null,

  setTokens: async (accessToken, refreshToken) => {
    await saveTokens(accessToken, refreshToken);
    set({ accessToken, refreshToken, isAuthenticated: true, needsTotp: false, pendingUserId: null });
  },

  setUserId: (userId) => {
    set({ userId });
  },

  setNeedsTotp: (userId) => {
    set({ needsTotp: true, pendingUserId: userId });
  },

  clearTotp: () => {
    set({ needsTotp: false, pendingUserId: null });
  },

  logout: async () => {
    await clearTokens();
    set({
      accessToken: null,
      refreshToken: null,
      userId: null,
      isAuthenticated: false,
      needsTotp: false,
      pendingUserId: null,
    });
  },

  hydrate: async () => {
    try {
      const [accessToken, refreshToken] = await Promise.all([
        getAccessToken(),
        getRefreshToken(),
      ]);
      if (accessToken && refreshToken) {
        set({ accessToken, refreshToken, isAuthenticated: true, isLoading: false });
      } else {
        set({ isLoading: false });
      }
    } catch {
      set({ isLoading: false });
    }
  },
}));

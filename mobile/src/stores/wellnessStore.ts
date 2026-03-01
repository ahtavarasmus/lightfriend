import { create } from "zustand";

interface WellnessState {
  processedNotificationIds: Set<string>;
  markProcessed: (id: string) => void;
  markAllProcessed: (ids: string[]) => void;
  isProcessed: (id: string) => boolean;
  clearProcessed: () => void;
}

export const useWellnessStore = create<WellnessState>((set, get) => ({
  processedNotificationIds: new Set<string>(),

  markProcessed: (id) =>
    set((state) => ({
      processedNotificationIds: new Set([...state.processedNotificationIds, id]),
    })),

  markAllProcessed: (ids) =>
    set((state) => ({
      processedNotificationIds: new Set([...state.processedNotificationIds, ...ids]),
    })),

  isProcessed: (id) => get().processedNotificationIds.has(id),

  clearProcessed: () => set({ processedNotificationIds: new Set() }),
}));

import { create } from "zustand";

interface DumbphoneState {
  isDumbphone: boolean;
  toggleDumbphone: () => void;
  setDumbphone: (value: boolean) => void;
}

export const useDumbphoneStore = create<DumbphoneState>((set) => ({
  isDumbphone: false,
  toggleDumbphone: () => set((state) => ({ isDumbphone: !state.isDumbphone })),
  setDumbphone: (value) => set({ isDumbphone: value }),
}));

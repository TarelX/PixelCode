import { create } from "zustand";

export type Screen = "title" | "camp";

export interface Camp {
  id: string;
  name: string | null;
  root_path: string;
  created_at: number | null;
  last_opened_at: number | null;
}

interface AppState {
  screen: Screen;
  camp: Camp | null;
  openCamp: (camp: Camp) => void;
}

export const useAppStore = create<AppState>((set) => ({
  screen: "title",
  camp: null,
  openCamp: (camp) => set({ camp, screen: "camp" }),
}));

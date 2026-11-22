import { create } from "zustand";

export type Screen = "title" | "camp";
export type WorkspaceTab = "agents" | "conversation" | "tasks" | "council";

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
  workspaceTab: WorkspaceTab;
  setWorkspaceTab: (tab: WorkspaceTab) => void;
  openCamp: (camp: Camp) => void;
  showTitle: () => void;
}

export const useAppStore = create<AppState>((set) => ({
  screen: "title",
  camp: null,
  workspaceTab: "agents",
  setWorkspaceTab: (workspaceTab) => set({ workspaceTab }),
  openCamp: (camp) => set({ camp, screen: "camp" }),
  showTitle: () => set({ screen: "title" }),
}));

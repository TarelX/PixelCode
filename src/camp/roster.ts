import { invoke } from "@tauri-apps/api/core";
import { create } from "zustand";

export interface RoleConfig {
  id: string;
  role_type: "captain" | "architect" | "designer" | "frontend" | "backend" | "debugger" | "reviewer";
  name: string;
  name_en: string | null;
  title: string | null;
  avatar: string | null;
  personality: string;
  skills: string[];
  loadouts: { cli: "claude-code" | "codex"; model: string }[];
  permissions: { allow: string[]; deny: string[] };
  task_types: string[];
  source: "preset" | "modified_preset" | "custom";
}

interface RosterState {
  roles: RoleConfig[];
  error: string | null;
  load: (campId: string | null) => Promise<RoleConfig[]>;
}

let request = 0;
export const useRoster = create<RosterState>((set) => ({
  roles: [],
  error: null,
  load: async (campId) => {
    const current = ++request;
    set({ roles: [], error: null });
    try {
      const roles = await invoke<RoleConfig[]>("list_roles", { campId });
      if (current === request) set({ roles });
      return roles;
    } catch (reason) {
      if (current === request) set({ error: `无法加载角色：${String(reason)}` });
      throw reason;
    }
  },
}));

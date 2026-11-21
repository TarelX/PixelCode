import { invoke } from "@tauri-apps/api/core";
import { create } from "zustand";

export type RoleType = "captain" | "architect" | "designer" | "frontend" | "backend" | "debugger" | "reviewer";
export type CliId = "claude-code" | "codex";

export interface RoleConfig {
  id: string;
  role_type: RoleType;
  name: string;
  name_en: string | null;
  title: string | null;
  avatar: string | null;
  personality: string;
  skills: string[];
  loadouts: { cli: CliId; model: string }[];
  permissions: { allow: string[]; deny: string[] };
  task_types: string[];
  source: "preset" | "modified_preset" | "custom";
}

export interface SkillInfo {
  id: string;
  summary: string;
}

interface RosterState {
  roles: RoleConfig[];
  skills: SkillInfo[];
  selectedId: string | null;
  error: string | null;
  load: (campId: string | null) => Promise<RoleConfig[]>;
  loadSkills: () => Promise<void>;
  select: (id: string | null) => void;
  save: (role: RoleConfig, campId: string | null) => Promise<void>;
  remove: (id: string, campId: string | null) => Promise<void>;
}

let request = 0;
export const useRoster = create<RosterState>((set, get) => ({
  roles: [],
  skills: [],
  selectedId: null,
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
  loadSkills: async () => {
    if (get().skills.length) return;
    set({ skills: await invoke<SkillInfo[]>("list_skills") });
  },
  select: (selectedId) => set({ selectedId }),
  save: async (role, campId) => {
    await invoke("save_role", { role, campId: null });
    const current = ++request;
    const roles = await invoke<RoleConfig[]>("list_roles", { campId });
    if (current === request) set({ roles, selectedId: role.id });
  },
  remove: async (id, campId) => {
    await invoke("delete_role", { id });
    const current = ++request;
    const roles = await invoke<RoleConfig[]>("list_roles", { campId });
    if (current === request) set({ roles, selectedId: roles.some((role) => role.id === id) ? id : null });
  },
}));

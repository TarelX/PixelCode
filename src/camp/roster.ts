import { parse } from "yaml";

export interface RoleConfig {
  id: string;
  name: string;
  name_en: string;
  title: string;
  avatar: string;
}

const roleFiles = import.meta.glob<string>("/roles/*.yaml", { query: "?raw", import: "default", eager: true });

export const roster: RoleConfig[] = Object.entries(roleFiles)
  .sort(([a], [b]) => a.localeCompare(b))
  .map(([, text]) => parse(text) as RoleConfig);

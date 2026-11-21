import { invoke } from "@tauri-apps/api/core";
import { parse } from "yaml";
import { create } from "zustand";
import type { RoleConfig } from "../camp/roster";
import zh from "../../themes/three-kingdoms/ui.json";
import en from "../../themes/three-kingdoms/ui.en.json";
import names from "../../themes/three-kingdoms/names.yaml?raw";

export type Locale = "zh-CN" | "en";
export type ThemeCopy = typeof zh;
const copy: Record<Locale, ThemeCopy> = { "zh-CN": zh, en };

export const useLocale = create<{
  locale: Locale;
  saving: boolean;
  error: string | null;
  load: () => Promise<void>;
  change: (locale: Locale) => Promise<void>;
}>((set, get) => ({
  locale: "zh-CN",
  saving: false,
  error: null,
  load: async () => {
    try {
      set({ locale: await invoke<Locale>("get_locale") });
    } catch (reason) {
      set({ error: String(reason) });
    }
  },
  change: async (locale) => {
    if (get().saving || get().locale === locale) return;
    set({ saving: true, error: null });
    try {
      await invoke("set_locale", { locale });
      set({ locale });
    } catch (reason) {
      set({ error: String(reason) });
    } finally {
      set({ saving: false });
    }
  },
}));

export function useI18n() {
  const locale = useLocale((s) => s.locale);
  return { locale, theme: copy[locale] };
}

const { generals } = parse(names) as { generals: { id: string; title: string; title_en: string }[] };

export function roleName(role: Pick<RoleConfig, "name" | "name_en">, locale: Locale) {
  return (locale === "en" && role.name_en?.trim()) || role.name.trim();
}

export function roleTitle(role: RoleConfig, locale: Locale) {
  const general = locale === "en" && role.source !== "custom"
    ? generals.find((item) => item.id === role.id && item.title === role.title) : undefined;
  return general?.title_en ?? (role.title?.trim() || copy[locale].roleTypes[role.role_type]);
}

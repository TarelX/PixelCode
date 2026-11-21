import { invoke } from "@tauri-apps/api/core";
import { useEffect } from "react";
import { create } from "zustand";

export interface DetectResult {
  id: "claude-code" | "codex";
  status: "not_found" | "found" | "found_needs_login";
  version: string | null;
  error: string | null;
}

interface CliDetectionState {
  results: DetectResult[];
  loading: boolean;
  error: string | null;
  refresh: () => Promise<void>;
}

const useCliDetectionStore = create<CliDetectionState>((set, get) => ({
  results: [],
  loading: false,
  error: null,
  refresh: async () => {
    if (get().loading) return;
    set({ loading: true, error: null });
    try {
      set({ results: await invoke<DetectResult[]>("detect_clis") });
    } catch (error) {
      set({ results: [], error: `无法探测 CLI：${String(error)}` });
    } finally {
      set({ loading: false });
    }
  },
}));

export function useCliDetection(scanOnMount = false) {
  const state = useCliDetectionStore();
  useEffect(() => {
    if (scanOnMount) void state.refresh();
  }, [scanOnMount, state.refresh]);
  return state;
}

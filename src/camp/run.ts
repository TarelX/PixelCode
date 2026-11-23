import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { create } from "zustand";
import type { Artifact } from "../orchestrator/artifact-manager";

export interface RoleRun {
  prompt?: string;
  run_id: number;
  task_id: string;
  artifact?: Artifact;
  status: "running" | "waiting" | "completed" | "failed" | "blocked" | "stopped";
  text: string;
  tool: string;
  error: string;
  usage_limited?: { resets_at: number | null };
  turn_metrics?: { input_tokens: number; output_tokens: number; cache_tokens: number | null };
}
interface RunEvent {
  run_id: number;
  task_id: string;
  artifact?: Artifact;
  camp_id: string;
  role_id: string;
  type: "started" | "text" | "tool" | "waiting" | "usage_limited" | "turn_metrics" | "completed" | "failed" | "blocked" | "stopped";
  text?: string;
  name?: string;
  message?: string;
  resets_at?: number | null;
  input_tokens?: number;
  output_tokens?: number;
  cache_tokens?: number | null;
}
export interface Clarification {
  questions: string[];
  answer: string | null;
}

export const useRuns = create<{
  runs: Record<string, RoleRun>;
  prompts: Record<string, string>;
  busy: boolean;
  error: string;
  receive: (event: RunEvent) => void;
  dismiss: (id: string) => void;
  start: (campId: string, roleId: string, loadoutIndex: number, prompt: string, clarification?: Clarification) => Promise<string[] | null>;
  stop: () => Promise<void>;
}>((set) => ({
  runs: {}, prompts: {}, busy: false, error: "",
  receive: (event) => set((state) => {
    const previous = state.runs[event.role_id];
    if (event.type !== "started" && previous?.run_id !== event.run_id) return state;
    const run: RoleRun = event.type === "started"
      ? { run_id: event.run_id, task_id: event.task_id, prompt: state.prompts[event.role_id], status: "running", text: "", tool: "", error: "" }
      : { ...previous };
    if (event.type === "text") run.text = (run.text + (event.text ?? "")).slice(-32000);
    if (event.type === "tool") run.tool = event.name ?? "";
    if (event.type === "usage_limited") run.usage_limited = { resets_at: event.resets_at ?? null };
    if (event.type === "turn_metrics" && typeof event.input_tokens === "number" && typeof event.output_tokens === "number") {
      run.turn_metrics = { input_tokens: event.input_tokens, output_tokens: event.output_tokens, cache_tokens: event.cache_tokens ?? null };
    }
    if (event.type === "completed" || event.type === "failed" || event.type === "blocked" || event.type === "stopped" || event.type === "waiting") {
      run.status = event.type;
      run.error = event.message ?? "";
      run.artifact = event.artifact;
    }
    return { runs: { ...state.runs, [event.role_id]: run }, prompts: event.type === "started"
      ? Object.fromEntries(Object.entries(state.prompts).filter(([id]) => id !== event.role_id)) : state.prompts };
  }),
  dismiss: (id) => set((state) => ({ runs: Object.fromEntries(Object.entries(state.runs).filter(([key]) => key !== id)) })),
  start: async (campId, roleId, loadoutIndex, prompt, clarification) => {
    set({ busy: true, error: "" });
    set((state) => ({ prompts: { ...state.prompts, [roleId]: prompt } }));
    try { return await invoke<string[] | null>("start_role_run", { campId, roleId, loadoutIndex, prompt, clarification }); }
    catch (error) { set({ error: String(error) }); return null; }
    finally { set((state) => ({ busy: false, prompts: Object.fromEntries(Object.entries(state.prompts).filter(([id]) => id !== roleId)) })); }
  },
  stop: async () => {
    try { await invoke("stop_role_run"); }
    catch (error) { set({ error: String(error) }); }
  },
}));

export function listenToRuns(campId: string) {
  useRuns.setState({ runs: {}, prompts: {}, error: "" });
  return listen<RunEvent>("role-run", ({ payload }) => {
    if (payload.camp_id === campId) useRuns.getState().receive(payload);
  });
}

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { create } from "zustand";
import type { Artifact } from "../orchestrator/artifact-manager";

export interface RoleRun {
  prompt?: string;
  run_id: string;
  task_id: string;
  artifact?: Artifact;
  status: "running" | "waiting" | "completed" | "failed" | "blocked" | "stopped";
  text: string;
  tool: string;
  error: string;
}
interface RunEvent {
  run_id: string;
  task_id: string;
  artifact?: Artifact;
  camp_id: string;
  role_id: string;
  type: "started" | "text" | "tool" | "waiting" | "completed" | "failed" | "blocked" | "stopped";
  text?: string;
  name?: string;
  message?: string;
}
export interface Clarification {
  questions: string[];
  answer: string | null;
}
export interface CaptainQuestions {
  questions: string[];
  pending_id: string;
}
export interface PendingQuestion {
  id: string;
  camp_id: string;
  role_id: string;
  loadout_index: number;
  request: string;
  questions: string[];
  answer: string | null;
  created_at: number;
}

export const useRuns = create<{
  runs: Record<string, RoleRun>;
  prompts: Record<string, string>;
  busy: boolean;
  error: string;
  receive: (event: RunEvent) => void;
  dismiss: (id: string) => void;
  start: (campId: string, roleId: string, loadoutIndex: number, prompt: string, clarification?: Clarification, pendingQuestionId?: string | null, answerKey?: string | null) => Promise<CaptainQuestions | null>;
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
    if (event.type === "completed" || event.type === "failed" || event.type === "blocked" || event.type === "stopped" || event.type === "waiting") {
      run.status = event.type;
      run.error = event.message ?? "";
      run.artifact = event.artifact;
    }
    return { runs: { ...state.runs, [event.role_id]: run }, prompts: event.type === "started"
      ? Object.fromEntries(Object.entries(state.prompts).filter(([id]) => id !== event.role_id)) : state.prompts };
  }),
  dismiss: (id) => set((state) => ({ runs: Object.fromEntries(Object.entries(state.runs).filter(([key]) => key !== id)) })),
  start: async (campId, roleId, loadoutIndex, prompt, clarification, pendingQuestionId = null, answerKey = null) => {
    set({ busy: true, error: "" });
    set((state) => ({ prompts: { ...state.prompts, [roleId]: prompt } }));
    try {
      const pendingAnswer = pendingQuestionId ? { id: pendingQuestionId, key: answerKey ?? "" } : null;
      return await invoke<CaptainQuestions | null>("start_role_run", { campId, roleId, loadoutIndex, prompt, clarification, pendingAnswer });
    } catch (error) { set({ error: String(error) }); return null; }
    finally { set((state) => ({ busy: false, prompts: Object.fromEntries(Object.entries(state.prompts).filter(([id]) => id !== roleId)) })); }
  },
  stop: async () => {
    try { await invoke("stop_role_run"); }
    catch (error) { set({ error: String(error) }); }
  },
}));

export async function listenToRuns(campId: string) {
  useRuns.setState({ runs: {}, prompts: {}, error: "" });
  const events = await invoke<RunEvent[]>("run_events", { campId }).catch(() => [] as RunEvent[]);
  for (const event of events ?? []) {
    if (event.camp_id === campId) useRuns.getState().receive(event);
  }
  return listen<RunEvent>("role-run", ({ payload }) => {
    if (payload.camp_id === campId) useRuns.getState().receive(payload);
  });
}

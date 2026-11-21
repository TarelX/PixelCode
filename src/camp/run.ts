import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { create } from "zustand";
import type { Artifact } from "../orchestrator/artifact-manager";

export interface RoleRun {
  run_id: number;
  task_id: string;
  artifact?: Artifact;
  status: "running" | "completed" | "failed" | "stopped";
  text: string;
  tool: string;
  error: string;
}
interface RunEvent {
  run_id: number;
  task_id: string;
  artifact?: Artifact;
  camp_id: string;
  role_id: string;
  type: "started" | "text" | "tool" | "completed" | "failed" | "stopped";
  text?: string;
  name?: string;
  message?: string;
}
export const useRuns = create<{
  runs: Record<string, RoleRun>;
  busy: boolean;
  error: string;
  receive: (event: RunEvent) => void;
  dismiss: (id: string) => void;
  start: (campId: string, roleId: string, loadoutIndex: number, prompt: string) => Promise<void>;
  stop: () => Promise<void>;
}>((set) => ({
  runs: {}, busy: false, error: "",
  receive: (event) => set((state) => {
    const previous = state.runs[event.role_id];
    if (event.type !== "started" && previous?.run_id !== event.run_id) return state;
    const run: RoleRun = event.type === "started"
      ? { run_id: event.run_id, task_id: event.task_id, status: "running", text: "", tool: "", error: "" }
      : { ...previous };
    if (event.type === "text") run.text = (run.text + (event.text ?? "")).slice(-32000);
    if (event.type === "tool") run.tool = event.name ?? "";
    if (["completed", "failed", "stopped"].includes(event.type)) {
      run.status = event.type as RoleRun["status"];
      run.error = event.message ?? "";
      run.artifact = event.artifact;
    }
    return { runs: { ...state.runs, [event.role_id]: run } };
  }),
  dismiss: (id) => set((state) => ({ runs: Object.fromEntries(Object.entries(state.runs).filter(([key]) => key !== id)) })),
  start: async (campId, roleId, loadoutIndex, prompt) => {
    set({ busy: true, error: "" });
    try { await invoke("start_role_run", { campId, roleId, loadoutIndex, prompt }); }
    catch (error) { set({ error: String(error) }); }
    finally { set({ busy: false }); }
  },
  stop: async () => {
    try { await invoke("stop_role_run"); }
    catch (error) { set({ error: String(error) }); }
  },
}));

export function listenToRuns(campId: string) {
  useRuns.setState({ runs: {}, error: "" });
  return listen<RunEvent>("role-run", ({ payload }) => {
    if (payload.camp_id === campId) useRuns.getState().receive(payload);
  });
}

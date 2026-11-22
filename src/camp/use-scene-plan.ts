import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { StoredPlan } from "../ui/plan-layout";
import { useRuns } from "./run";

export function useScenePlan(campId: string | undefined) {
  const [plan, setPlan] = useState<StoredPlan | null>(null);
  const [error, setError] = useState("");
  const refreshRef = useRef(() => {});
  const revision = useRuns((s) => Object.values(s.runs).map((run) => `${run.run_id}:${run.status}`).join(","));
  useEffect(() => {
    setPlan(null); setError("");
    if (!campId) return;
    let active = true, request = 0;
    const refresh = async () => {
      const id = ++request;
      try {
        const next = await invoke<StoredPlan | null>("latest_plan", { campId });
        if (active && id === request) { setPlan(next); setError(""); }
      } catch (reason) { if (active && id === request) setError(String(reason)); }
    };
    refreshRef.current = () => { void refresh(); };
    const subscriptions = [
      listen<{ camp_id: string }>("plan-task", ({ payload }) => { if (payload.camp_id === campId) void refresh(); }),
      listen<string>("plan-state", ({ payload }) => { if (payload === campId) void refresh(); }),
    ];
    void Promise.all(subscriptions).then(refresh).catch((reason) => { if (active) setError(String(reason)); });
    return () => {
      active = false; refreshRef.current = () => {};
      for (const subscription of subscriptions) void subscription.then((stop) => stop(), () => {});
    };
  }, [campId]);
  useEffect(() => { refreshRef.current(); }, [revision]);
  return { plan, error };
}

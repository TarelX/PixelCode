import { useCallback, useEffect, useState, type ReactNode } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useRoster, type RoleConfig } from "../camp/roster";
import { useRuns } from "../camp/run";
import { roleName, roleTitle, useI18n } from "../i18n";
import type { StoredPlan } from "./plan-layout";
import { RoleRoster } from "./RoleRoster";
import { RoleAvatar } from "./components/RoleAvatar";
import { PixelIcon } from "./components/PixelIcon";

export function AgentPanel({ campId, onOpen, onPlan, children }: { campId: string; onOpen: (role: RoleConfig | null) => void; onPlan: () => void; children: ReactNode }) {
  const { locale, theme } = useI18n();
  const { roles, selectedId } = useRoster();
  const runs = useRuns((s) => s.runs);
  const selected = roles.find((role) => role.id === selectedId);
  const [plan, setPlan] = useState<StoredPlan | null>(null);
  const [error, setError] = useState("");
  const revision = Object.values(runs).map((run) => `${run.run_id}:${run.status}`).join(",");
  const refresh = useCallback(async () => {
    try { setPlan(await invoke<StoredPlan | null>("latest_plan", { campId })); setError(""); }
    catch (reason) { setError(String(reason)); }
  }, [campId]);
  useEffect(() => {
    let active = true;
    const refreshActive = () => { if (active) void refresh(); };
    const subscriptions = [
      listen<{ camp_id: string }>("plan-task", ({ payload }) => { if (payload.camp_id === campId) refreshActive(); }),
      listen<string>("plan-state", ({ payload }) => { if (payload === campId) refreshActive(); }),
    ];
    void Promise.all(subscriptions).then(refreshActive).catch((reason) => { if (active) setError(String(reason)); });
    return () => { active = false; for (const subscription of subscriptions) void subscription.then((stop) => stop(), () => {}); };
  }, [campId, refresh]);
  useEffect(() => { void refresh(); }, [refresh, revision]);
  const tasks = plan?.plan.tasks ?? [];
  const done = tasks.filter((task) => task.status === "done").length;
  const artifacts = Object.values(runs).flatMap((run) => run.artifact ? [run.artifact] : []);
  return <aside className="agent-panel" aria-label={theme.studio.team}>
    <header className="agent-panel__header"><PixelIcon name="team" /><strong>{theme.studio.team}</strong><span>{roles.length}</span></header>
    <section className="agent-focus"><h3>{theme.studio.lead}</h3>{selected && <button onClick={() => onOpen(selected)} title={theme.studio.editAgent}>
      <RoleAvatar avatar={selected.avatar} /><span><strong>{roleName(selected, locale)}</strong><small>{roleTitle(selected, locale)}</small></span><PixelIcon name="edit" />
    </button>}<p>{selected?.loadouts[0]?.cli ?? theme.taskSelect}</p></section>
    <section className="team-progress"><div className="panel-section-heading"><h3>{theme.studio.progress}</h3><button onClick={onPlan}><PixelIcon name="arrow" /></button></div>
      {plan ? <><div className="progress-track" role="progressbar" aria-label={theme.studio.progress} aria-valuemin={0} aria-valuemax={tasks.length || 1} aria-valuenow={done}><span style={{ width: `${tasks.length ? done / tasks.length * 100 : 0}%` }} /></div><p><strong>{done} / {tasks.length}</strong><span>{theme.studio.done}</span></p><button className="plan-summary" onClick={onPlan}>{plan.user_request}</button><small>{(theme.wall.planStatuses as Record<string, string>)[plan.status] ?? plan.status}</small></> : <><div className="progress-track"><span /></div><p>{theme.studio.noPlan}</p><small>{theme.studio.planHint}</small></>}
      {error && <p role="alert">{error}</p>}
    </section>
    <RoleRoster onOpen={onOpen} />
    <section className="team-artifacts"><h3>{theme.studio.outputs}<span>{artifacts.length}</span></h3>{artifacts.length ? artifacts.map((artifact) => <button key={artifact.id} onClick={onPlan} title={artifact.absolute_path}><PixelIcon name="folder" /><span>{artifact.file_path.split(/[/\\]/).pop()}</span><i className="status-dot" /></button>) : <p>{theme.studio.noOutputs}</p>}</section>
    <div className="agent-panel__footer">{children}</div>
  </aside>;
}

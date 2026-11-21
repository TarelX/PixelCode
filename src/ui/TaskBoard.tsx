import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useRoster } from "../camp/roster";
import { useRuns } from "../camp/run";
import { layoutPlan, type StoredPlan, type PlanTask } from "./plan-layout";
import { PixelDialog } from "./PixelDialog";
import { TaskDetail } from "./TaskDetail";
import theme from "../../themes/three-kingdoms/ui.json";
import "../styles/task-board.css";

export function TaskBoard({ campId }: { campId: string }) {
  const [plan, setPlan] = useState<StoredPlan | null>(null);
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(true);
  const [selected, setSelected] = useState<PlanTask | null>(null);
  const roles = useRoster((s) => s.roles);
  const revision = useRuns((s) => Object.values(s.runs).filter((run) => run.status === "completed" && run.artifact?.type === "plan").map((run) => run.artifact!.id).join(","));
  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setError("");
    setSelected(null);
    invoke<StoredPlan | null>("latest_plan", { campId }).then((value) => {
      if (!cancelled) setPlan(value);
    }).catch((reason) => { if (!cancelled) setError(`${theme.wall.loadError}${String(reason)}`); })
      .finally(() => { if (!cancelled) setLoading(false); });
    return () => { cancelled = true; };
  }, [campId, revision]);
  const roleName = (id: string) => roles.find((role) => role.id === id)?.name ?? id;
  if (loading) return <p role="status">{theme.wall.loading}</p>;
  if (error) return <p role="alert">{error}</p>;
  if (!plan) return <p className="wall-empty">{theme.wall.empty}</p>;
  let nodes;
  try { nodes = layoutPlan(plan.plan.tasks); }
  catch { return <p role="alert">{theme.wall.invalid}</p>; }
  const width = Math.max(480, ...nodes.map((node) => node.x + 256));
  const height = Math.max(176, ...nodes.map((node) => node.y + 144));
  const statuses: Record<string, string> = theme.wall.statuses;
  const planStatuses: Record<string, string> = theme.wall.planStatuses;
  return <section className="strategy-wall">
    <header><span className="section-label">{plan.scenario}</span><strong>{planStatuses[plan.status] ?? plan.status}</strong><p>{plan.user_request}</p></header>
    <div className="wall-scroll"><div className="wall-graph" style={{ width, height }}>
      <svg width={width} height={height} shapeRendering="crispEdges" aria-hidden="true">
        {nodes.flatMap((node) => node.task.depends_on.map((id) => {
          const parent = nodes.find((item) => item.task.id === id)!;
          const x = parent.x + 240, y = parent.y + 64, end = node.x, bottom = node.y + 64, mid = end - 24;
          return <g key={`${id}-${node.task.id}`}><path d={`M${x} ${y}H${mid}V${bottom}H${end - 6}`} /><path d={`M${end - 8} ${bottom - 4}L${end - 4} ${bottom}L${end - 8} ${bottom + 4}`} /></g>;
        }))}
      </svg>
      {nodes.map(({ task, x, y }) => <button className="strategy-card" key={task.id} style={{ left: x, top: y }} onClick={() => setSelected(task)}>
        <strong>{task.title}</strong><span>{roleName(task.role_id)}</span>
        <small className={`task-status task-status--${Object.prototype.hasOwnProperty.call(statuses, task.status ?? "pending") ? task.status ?? "pending" : "pending"}`}>{statuses[task.status ?? "pending"] ?? task.status}</small>
      </button>)}
    </div></div>
    <PixelDialog open={selected !== null} title={selected?.title ?? theme.wall.title} closeLabel={theme.wall.close} onClose={() => setSelected(null)}>
      {selected && <TaskDetail task={selected} role={roleName(selected.role_id)} tasks={plan.plan.tasks} />}
    </PixelDialog>
  </section>;
}

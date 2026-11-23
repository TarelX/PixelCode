import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useRoster } from "../camp/roster";
import { useRuns } from "../camp/run";
import { layoutPlan, type StoredPlan } from "./plan-layout";
import { PixelDialog } from "./PixelDialog";
import { TaskDetail } from "./TaskDetail";
import { PixelButton } from "./components/PixelButton";
import { useI18n, roleName } from "../i18n";
import "../styles/task-board.css";

export function TaskBoard({ campId }: { campId: string }) {
  const { locale, theme } = useI18n();
  const gate = theme.wall.gate;
  const [plan, setPlan] = useState<StoredPlan | null>(null);
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(true);
  const [selected, setSelected] = useState<{ planId: string; taskId: string } | null>(null);
  const [edits, setEdits] = useState<{ planId: string; assignments: Record<string, string> } | null>(null);
  const [busy, setBusy] = useState(false);
  const [actionError, setActionError] = useState("");
  const [cleanup, setCleanup] = useState<string[]>([]);
  const request = useRef(0);
  const roles = useRoster((s) => s.roles);
  const revision = useRuns((s) => Object.values(s.runs).filter((run) => run.status === "completed" && run.artifact?.type === "plan").map((run) => run.artifact!.id).join(","));
  const refresh = useCallback(async () => {
    const id = ++request.current;
    try {
      const value = await invoke<StoredPlan | null>("latest_plan", { campId });
      const diagnostics = await invoke<string[]>("list_worktree_diagnostics", { campId }).catch(() => [] as string[]);
      if (id === request.current) { setPlan(value); setCleanup(diagnostics); setError(""); }
    } catch (reason) {
      if (id === request.current) setError(String(reason));
    } finally {
      if (id === request.current) setLoading(false);
    }
  }, [campId]);
  useEffect(() => {
    let active = true;
    const subscriptions = [
      listen<{ camp_id: string }>("plan-task", ({ payload }) => {
        if (active && payload.camp_id === campId) void refresh();
      }),
      listen<string>("plan-state", ({ payload }) => {
        if (active && payload === campId) void refresh();
      }),
    ];
    void Promise.all(subscriptions).then(() => { if (active) void refresh(); })
      .catch((reason) => { if (active) setActionError(String(reason)); });
    return () => {
      active = false;
      request.current++;
      for (const subscription of subscriptions) void subscription.then((unlisten) => unlisten(), () => {});
    };
  }, [campId, refresh]);
  useEffect(() => { void refresh(); }, [refresh, revision]);
  const nameForRole = (id: string) => {
    const role = roles.find((item) => item.id === id);
    return role ? roleName(role, locale) : id;
  };
  if (loading) return <p role="status">{theme.wall.loading}</p>;
  if (error) return <p role="alert">{theme.wall.loadError}{error}</p>;
  if (!plan) return <p className="wall-empty">{theme.wall.empty}</p>;
  const draft = plan.status === "draft";
  const delivery = plan.status === "awaiting_delivery";
  const assignments = draft && edits?.planId === plan.id ? edits.assignments : {};
  const tasks = plan.plan.tasks.map((task) => ({
    ...task, role_id: Object.prototype.hasOwnProperty.call(assignments, task.id) ? assignments[task.id] : task.role_id,
  }));
  const detail = selected?.planId === plan.id ? tasks.find((task) => task.id === selected.taskId) : undefined;
  const checkedTasks = tasks.filter((task) => task.check_command);
  const resume = plan.status === "aborted" && tasks.some((task) => task.status === "interrupted");
  async function decide(command: string, args: Record<string, unknown> = {}) {
    setBusy(true);
    setActionError("");
    try { await invoke(command, { campId, planId: plan!.id, ...args }); }
    catch (reason) { setActionError(String(reason)); }
    finally { await refresh(); setBusy(false); }
  }
  let nodes;
  try { nodes = layoutPlan(tasks); }
  catch { return <p role="alert">{theme.wall.invalid}</p>; }
  const width = Math.max(480, ...nodes.map((node) => node.x + 256));
  const height = Math.max(176, ...nodes.map((node) => node.y + 144));
  const statuses: Record<string, string> = theme.wall.statuses;
  const planStatuses: Record<string, string> = theme.wall.planStatuses;
  const scenarios: Record<string, string> = theme.wall.scenarios;
  return <section className="strategy-wall">
    <header>
      <span className="section-label">{scenarios[plan.scenario] ?? plan.scenario}</span><strong>{planStatuses[plan.status] ?? plan.status}</strong><p>{plan.user_request}</p>
      <div className="wall-recruits"><span>{theme.wall.recruited}</span>{plan.plan.recruited_roles.map((id) => <span key={id}>{nameForRole(id)}</span>)}</div>
    </header>
    {(cleanup.length > 0 || actionError || plan.execution_message) && <p role="alert">{cleanup.length > 0 ? `${theme.wall.cleanup}${cleanup.join("、")}` : ""}{actionError || plan.execution_message}</p>}
    <div className="wall-scroll"><div className="wall-graph" style={{ width, height }}>
      <svg width={width} height={height} shapeRendering="crispEdges" aria-hidden="true">
        {nodes.flatMap((node) => node.task.depends_on.map((id) => {
          const parent = nodes.find((item) => item.task.id === id)!;
          const x = parent.x + 240, y = parent.y + 64, end = node.x, bottom = node.y + 64, mid = end - 24;
          return <g key={`${id}-${node.task.id}`}><path d={`M${x} ${y}H${mid}V${bottom}H${end - 6}`} /><path d={`M${end - 8} ${bottom - 4}L${end - 4} ${bottom}L${end - 8} ${bottom + 4}`} /></g>;
        }))}
      </svg>
      {nodes.map(({ task, x, y }) => <button className="strategy-card" key={task.id} style={{ left: x, top: y }} onClick={() => setSelected({ planId: plan.id, taskId: task.id })}>
        <strong>{task.title}</strong><span>{nameForRole(task.role_id)}</span>
        <small className={`task-status task-status--${Object.prototype.hasOwnProperty.call(statuses, task.status ?? "pending") ? task.status ?? "pending" : "pending"}`}>{statuses[task.status ?? "pending"] ?? task.status}</small>
      </button>)}
    </div></div>
    {(draft || delivery || resume) && <section className="wall-gate">
      <h3>{draft ? gate.approvalTitle : delivery ? gate.deliveryTitle : gate.resumeTitle}</h3>
      <p>{draft ? gate.approvalHint : delivery ? gate.deliveryHint : gate.resumeHint}</p>
      {draft && checkedTasks.length > 0 && <ul>{checkedTasks.map(({ id, title, check_command: check }) =>
        <li key={id}>{title}: <code>{JSON.stringify([check!.program, ...check!.args])}</code></li>
      )}</ul>}
      <div className="wall-gate__actions">
        {resume ? <PixelButton variant="primary" disabled={busy} onClick={() => void decide("continue_plan_execution")}>{gate.resume}</PixelButton> : <>
          <PixelButton variant="primary" disabled={busy} onClick={() => void (draft
            ? decide("approve_plan_execution", { assignments })
            : decide("decide_delivery", { accepted: true }))}>{draft ? gate.approve : gate.deliver}</PixelButton>
          <PixelButton disabled={busy} onClick={() => void (draft
            ? decide("reject_plan")
            : decide("decide_delivery", { accepted: false }))}>{draft ? gate.rejectDraft : gate.rejectDelivery}</PixelButton>
        </>}
      </div>
    </section>}
    <PixelDialog open={Boolean(detail)} title={detail?.title ?? theme.wall.title} closeLabel={gate.backToWall} onClose={() => setSelected(null)}>
      {detail && <TaskDetail
        key={`${plan.id}-${detail.id}`}
        task={detail}
        role={nameForRole(detail.role_id)}
        tasks={tasks}
        resultId={!draft && detail.status === "done" ? `${plan.id}-${detail.id}` : undefined}
        assignment={draft && <fieldset className="wall-assignment">
          <legend>{gate.assign}</legend>
          {roles.map((role) => <PixelButton
            key={role.id}
            disabled={busy}
            aria-pressed={detail.role_id === role.id}
            onClick={() => setEdits({ planId: plan.id, assignments: { ...assignments, [detail.id]: role.id } })}
          >{roleName(role, locale)}</PixelButton>)}
        </fieldset>}
      />}
    </PixelDialog>
  </section>;
}

import type { RoleConfig } from "../camp/roster";
import type { RoleRun } from "../camp/run";
import type { BubbleAnchor } from "../camp/bubble";
import { planHandoffs, roleActivity, type Handoff } from "../camp/collaboration";
import type { StoredPlan } from "./plan-layout";
import { roleName, useI18n } from "../i18n";
import "../styles/scene.css";

export function SceneTasks({ roles, runs, plan, anchors, selectedId, handoff, onSelect }: {
  roles: RoleConfig[]; runs: Record<string, RoleRun>; plan: StoredPlan | null;
  anchors: Record<string, BubbleAnchor>; selectedId: string | null; handoff: Handoff | null;
  onSelect: (id: string) => void;
}) {
  const { locale, theme } = useI18n();
  const copy = theme.scene;
  const statuses: Record<string, string> = { ...theme.wall.statuses, ...theme.runStates, idle: copy.idle, draft: copy.draft };
  return <div className="scene-tasks">{roles.map((role) => {
    const anchor = anchors[role.id];
    if (!anchor) return null;
    const activity = roleActivity(role.id, plan, runs[role.id], role.role_type === "reviewer");
    const passing = handoff?.from.role_id === role.id;
    const name = roleName(role, locale);
    const status = passing ? copy.handing : statuses[activity.status] ?? activity.status;
    const title = passing ? `${copy.to} ${roleName(roles.find((item) => item.id === handoff.to.role_id) ?? role, locale)}` : activity.title || copy.available;
    const width = Math.min(132, Math.max(88, anchor.canvas.width / 5.6));
    const left = anchor.canvas.left + Math.max(4, Math.min(anchor.canvas.width - width - 4, anchor.x - width / 2));
    return <button key={role.id} className={`scene-task ${role.id === selectedId ? "is-selected" : ""}`} data-status={passing ? "handoff" : activity.status}
      style={{ left, top: anchor.canvas.top + Math.max(4, anchor.y - 45), width }} onClick={() => onSelect(role.id)}
      title={`${name} · ${status}\n${title}${activity.detail ? `\n${activity.detail}` : ""}`}>
      <span><strong>{name}</strong><i />{status}</span><small>{title}</small>
    </button>;
  })}</div>;
}

export function SceneHandoffs({ plan, roles, handoff, error, onPlan }: {
  plan: StoredPlan | null; roles: RoleConfig[]; handoff: Handoff | null; error: string; onPlan: () => void;
}) {
  const { locale, theme } = useI18n();
  const copy = theme.scene;
  const transfers = planHandoffs(plan).slice(-3).reverse();
  const name = (id: string) => { const role = roles.find((item) => item.id === id); return role ? roleName(role, locale) : id; };
  return <div className="scene-handoffs">
    <div className="scene-handoffs__heading"><strong>{copy.handoffs}</strong><span>{copy.hint}</span><button onClick={onPlan}>{theme.studio.viewPlan} →</button></div>
    {error ? <p role="alert">{theme.wall.loadError}{error}</p> : transfers.length ? <ol aria-label={copy.handoffs}>{transfers.map((transfer) => <li key={transfer.id}>
      <i className="scene-envelope" aria-hidden="true" /><div><strong>{name(transfer.from.role_id)} <span>→</span> {name(transfer.to.role_id)}</strong>
        <small title={transfer.artifacts.join(", ") || transfer.from.title}>{transfer.artifacts.map((path) => path.split(/[/\\]/).pop()).join(", ") || transfer.from.title}</small></div>
      <span className="scene-transfer-state">{handoff?.id === transfer.id ? copy.handing : transfer.ready ? copy.ready : copy.waiting}</span>
    </li>)}</ol> : <p>{copy.empty}</p>}
    <span className="sr-only" role="status">{handoff ? `${name(handoff.from.role_id)} → ${name(handoff.to.role_id)}: ${copy.handing}` : ""}</span>
  </div>;
}

import type { RoleConfig } from "../camp/roster";
import type { RoleRun } from "../camp/run";
import type { BubbleAnchor } from "../camp/bubble";
import { planHandoffs, roleActivity, type Handoff } from "../camp/collaboration";
import { loadCampLayout } from "../camp/tilemap";
import type { StoredPlan } from "./plan-layout";
import { roleName, useI18n } from "../i18n";
import "../styles/scene.css";

export function SceneTasks({ placement, roles, runs, plan, anchors, selectedId, handoff, onSelect }: {
  placement: "top" | "bottom";
  roles: RoleConfig[]; runs: Record<string, RoleRun>; plan: StoredPlan | null;
  anchors: Record<string, BubbleAnchor>; selectedId: string | null; handoff: Handoff | null;
  onSelect: (id: string) => void;
}) {
  const { locale, theme } = useI18n();
  const copy = theme.scene;
  const statuses: Record<string, string> = { ...theme.wall.statuses, ...theme.runStates, idle: copy.idle, draft: copy.draft };
  const canvas = Object.values(anchors)[0]?.canvas;
  const layout = loadCampLayout();
  const zones = new Map(layout.zones.filter((zone) => zone.role).map((zone) => [zone.role, zone]));
  const row = roles.filter((role) => {
    const zone = zones.get(role.id);
    const top = zone ? (zone.y + zone.h) * layout.tileSize < layout.height / 2 : false;
    return top === (placement === "top");
  }).sort((a, b) => (zones.get(a.id)?.x ?? layout.width) - (zones.get(b.id)?.x ?? layout.width));
  if (!row.length) return null;
  const capacity = Math.max(1, Math.floor(((canvas?.width ?? layout.width) - 10) / 94));
  const columns = Math.ceil(row.length / Math.ceil(row.length / capacity));
  return <div className={`scene-tasks scene-tasks--${placement}`} style={{ width: canvas?.width, gridTemplateColumns: `repeat(${columns}, minmax(0, 1fr))` }}>{row.map((role) => {
    const activity = roleActivity(role.id, plan, runs[role.id], role.role_type === "reviewer");
    const passing = handoff?.from.role_id === role.id;
    const name = roleName(role, locale);
    const status = passing ? copy.handing : statuses[activity.status] ?? activity.status;
    const title = passing ? `${copy.to} ${roleName(roles.find((item) => item.id === handoff.to.role_id) ?? role, locale)}` : activity.title || copy.available;
    return <button key={role.id} className={`scene-task ${role.id === selectedId ? "is-selected" : ""}`} data-status={passing ? "handoff" : activity.status}
      onClick={() => onSelect(role.id)}
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

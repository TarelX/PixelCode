import { useRoster } from "../camp/roster";
import { useRuns } from "../camp/run";
import { roleActivity } from "../camp/collaboration";
import { useI18n } from "../i18n";
import { PixelIcon } from "./components/PixelIcon";
import type { StoredPlan } from "./plan-layout";

export function WorkspaceHeading({ plan }: { plan: StoredPlan | null }) {
  const { theme } = useI18n();
  const roles = useRoster((s) => s.roles);
  const runs = useRuns((s) => s.runs);
  const active = roles.filter((role) => ["running", "waiting", "checking", "reviewing"].includes(
    roleActivity(role.id, plan, runs[role.id], role.role_type === "reviewer").status,
  )).length;
  return <header className="workspace-heading">
    <h1>{theme.workbench.eyebrow}</h1>
    <div className="workspace-metrics"><span><PixelIcon name="team" /><strong>{roles.length}</strong>{theme.workbench.agents}</span><span className={active ? "is-running" : ""}><i className="status-dot" /><strong>{active}</strong>{theme.workbench.active}</span></div>
  </header>;
}

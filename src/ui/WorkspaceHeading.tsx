import { useAppStore } from "../app/store";
import { useRoster } from "../camp/roster";
import { useRuns } from "../camp/run";
import { useI18n } from "../i18n";
import { PixelIcon } from "./components/PixelIcon";

export function WorkspaceHeading() {
  const { theme } = useI18n();
  const camp = useAppStore((s) => s.camp);
  const roles = useRoster((s) => s.roles);
  const runs = useRuns((s) => s.runs);
  const active = roles.filter((role) => ["running", "waiting"].includes(runs[role.id]?.status)).length;
  return <header className="workspace-heading">
    <div><span className="section-label">{theme.workbench.eyebrow}</span><h1>{camp?.name || theme.workspace.title}</h1><p>{theme.workbench.description}</p></div>
    <div className="workspace-metrics"><span><PixelIcon name="team" /><strong>{roles.length}</strong>{theme.workbench.agents}</span><span className={active ? "is-running" : ""}><i className="status-dot" /><strong>{active}</strong>{theme.workbench.active}</span></div>
  </header>;
}

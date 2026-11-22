import { useAppStore } from "../app/store";
import { useRoster } from "../camp/roster";
import { useRuns } from "../camp/run";
import { roleName, useI18n } from "../i18n";
import { PixelIcon } from "./components/PixelIcon";

export function ProjectSidebar() {
  const { locale, theme } = useI18n();
  const { camp, screen, openCamp, showTitle, setWorkspaceTab } = useAppStore();
  const { roles, selectedId, select } = useRoster();
  const runs = useRuns((s) => s.runs);
  function compose() {
    if (!camp) { showTitle(); return; }
    openCamp(camp);
    setWorkspaceTab("conversation");
    select(roles.find((role) => role.role_type === "captain")?.id ?? null);
    requestAnimationFrame(() => document.getElementById("task-prompt")?.focus());
  }
  return <aside className="project-sidebar" aria-label={theme.workspace.label}>
    <header><strong>{theme.appName}</strong><span>⌘</span></header>
    <button className="project-switch" onClick={showTitle} title={theme.studio.openProject}>
      <span className="project-monogram">{(camp?.name || "P").slice(0, 1).toUpperCase()}</span>
      <strong>{camp?.name || theme.workspace.title}</strong><PixelIcon name="chevron" />
    </button>
    <p className="project-path" title={camp?.root_path}><PixelIcon name="folder" />{camp?.root_path || theme.studio.chooseFolder}</p>
    <button className="new-session" onClick={compose}><PixelIcon name="plus" />{theme.studio.newTask}</button>
    <div className="sidebar-section"><span>{theme.studio.conversations}</span><span title={theme.studio.sessionHint}><PixelIcon name="info" /></span></div>
    <div className="session-list">
      {camp && screen === "camp" && <button className={`session-item ${!runs[selectedId ?? ""] ? "is-active" : ""}`} onClick={compose}><PixelIcon name="chat" /><span>{theme.studio.newConversation}</span><span>···</span></button>}
      {screen === "camp" && Object.entries(runs).filter(([id]) => roles.some((role) => role.id === id)).map(([id, run]) => <button key={id} className={`session-item ${id === selectedId ? "is-active" : ""}`} onClick={() => { if (camp) openCamp(camp); setWorkspaceTab("conversation"); select(id); }}>
        <PixelIcon name="chat" /><span>{roleName(roles.find((role) => role.id === id)!, locale)}</span><i className={`status-dot run-dot--${run.status}`} />
      </button>)}
      {Object.keys(runs).length === 0 && <p className="sidebar-empty">{theme.studio.noConversations}</p>}
    </div>
    <div className="sidebar-tip"><PixelIcon name="info" /><p>{theme.studio.sessionHint}</p></div>
    <div className="sidebar-bottom"><span className="status-dot" />{theme.workspace.localTitle}<small>{theme.workspace.localHint}</small></div>
  </aside>;
}

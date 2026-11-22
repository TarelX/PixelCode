import { useState } from "react";
import { useCliDetection } from "./cli-detection";
import { useAppStore } from "./store";
import { TitleScreen } from "../ui/TitleScreen";
import { CampView } from "../ui/CampView";
import { PixelDialog } from "../ui/PixelDialog";
import { SettingsView } from "../ui/SettingsView";
import { CliStatus } from "../ui/components/CliStatus";
import { PixelButton } from "../ui/components/PixelButton";
import { TopBar } from "../ui/components/TopBar";
import { PixelIcon } from "../ui/components/PixelIcon";
import { useI18n } from "../i18n";
import { ProjectSidebar } from "../ui/ProjectSidebar";

export default function App() {
  const { theme } = useI18n();
  const screen = useAppStore((s) => s.screen);
  const camp = useAppStore((s) => s.camp);
  const showTitle = useAppStore((s) => s.showTitle);
  const openCamp = useAppStore((s) => s.openCamp);
  const [settingsOpen, setSettingsOpen] = useState(false);
  useCliDetection(true);
  return (
    <div className="pixel-shell">
      <TopBar title={screen === "title" ? theme.workspace.home : camp?.name || theme.campTitle}>
          <span className="workspace-local"><span className="status-dot" />{theme.workspace.local}</span>
          <PixelButton className="workspace-settings" aria-haspopup="dialog" onClick={() => setSettingsOpen(true)}><PixelIcon name="settings" />{theme.settings}</PixelButton>
      </TopBar>
      <div className="workspace-layout">
        <nav className="workspace-rail" aria-label={theme.workspace.navigation}>
          <span className="rail-brand" title={theme.appName}><PixelIcon name="terminal" /></span>
          <button title={theme.workspace.home} aria-label={theme.workspace.home} aria-current={screen === "title" ? "page" : undefined} onClick={showTitle}><PixelIcon name="home" /></button>
          {camp && <button className="rail-project" title={camp.name || theme.campTitle} aria-label={camp.name || theme.campTitle} aria-current={screen === "camp" ? "page" : undefined} onClick={() => openCamp(camp)}>{(camp.name || "P").slice(0, 1).toUpperCase()}</button>}
          <button title={theme.studio.openProject} aria-label={theme.studio.openProject} onClick={showTitle}><PixelIcon name="plus" /></button>
          <button className="rail-settings" title={theme.settings} aria-label={theme.settings} aria-haspopup="dialog" onClick={() => setSettingsOpen(true)}><PixelIcon name="settings" /></button>
        </nav>
        <ProjectSidebar />
        <div className="workspace-main"><div className="pixel-shell__content">{screen === "title" ? <TitleScreen /> : <div className="pixel-shell__camp" key={camp?.id}><CampView /></div>}</div></div>
      </div>
      <CliStatus compact onOpenSettings={() => setSettingsOpen(true)} />
      <PixelDialog
        open={settingsOpen}
        title={theme.settingsTitle}
        closeLabel={theme.closeSettings}
        onClose={() => setSettingsOpen(false)}
      >
        {settingsOpen && <SettingsView />}
      </PixelDialog>
    </div>
  );
}

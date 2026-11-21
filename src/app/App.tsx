import { useState } from "react";
import { useCliDetection } from "./cli-detection";
import { useAppStore } from "./store";
import { zh } from "../i18n/zh";
import { TitleScreen } from "../ui/TitleScreen";
import { CampView } from "../ui/CampView";
import { PixelDialog } from "../ui/PixelDialog";
import { SettingsView } from "../ui/SettingsView";
import { CliStatus } from "../ui/components/CliStatus";
import { PixelButton } from "../ui/components/PixelButton";
import { TopBar } from "../ui/components/TopBar";
import { PixelIcon } from "../ui/components/PixelIcon";
import theme from "../../themes/three-kingdoms/ui.json";

export default function App() {
  const screen = useAppStore((s) => s.screen);
  const camp = useAppStore((s) => s.camp);
  const showTitle = useAppStore((s) => s.showTitle);
  const openCamp = useAppStore((s) => s.openCamp);
  const [settingsOpen, setSettingsOpen] = useState(false);
  useCliDetection(true);
  return (
    <div className="pixel-shell">
      <aside className="workspace-sidebar" aria-label="主导航">
        <div className="workspace-brand"><span className="workspace-brand__mark"><PixelIcon name="camp" /></span><strong>{zh.appName}<small>{theme.brandTagline}</small></strong></div>
        <span className="section-label">WORKSPACE</span>
        <nav>
          <PixelButton className={`workspace-nav ${screen === "title" ? "is-active" : ""}`} aria-current={screen === "title" ? "page" : undefined} onClick={showTitle}><PixelIcon name="home" />开始</PixelButton>
          {camp && <PixelButton className={`workspace-nav ${screen === "camp" ? "is-active" : ""}`} aria-current={screen === "camp" ? "page" : undefined} onClick={() => openCamp(camp)}><PixelIcon name="camp" />{zh.campTitle}</PixelButton>}
        </nav>
        {camp && <div className="workspace-project"><span className="section-label">当前项目</span><span><PixelIcon name="folder" />{camp.name || camp.root_path}</span><small title={camp.root_path}>{camp.root_path}</small></div>}
        <div className="workspace-sidebar__bottom">
          <div className="workspace-note"><span className="status-dot" />本地工作空间<small>你的代码，留在你的电脑</small></div>
          <PixelButton className="workspace-nav" aria-haspopup="dialog" onClick={() => setSettingsOpen(true)}><PixelIcon name="settings" />{zh.settings}</PixelButton>
          <span className="workspace-version">PIXELCODE <span>0.1.0</span></span>
        </div>
      </aside>
      <div className="workspace-main">
        <TopBar title={screen === "title" ? "开始" : camp?.name || zh.campTitle}>
          <span className="workspace-local"><span className="status-dot" />LOCAL</span>
          <PixelButton className="workspace-settings" aria-haspopup="dialog" onClick={() => setSettingsOpen(true)}><PixelIcon name="settings" />{zh.settings}</PixelButton>
        </TopBar>
        <div className="pixel-shell__content">{screen === "title" ? <TitleScreen /> : <div className="pixel-shell__camp"><CampView /></div>}</div>
        <CliStatus compact onOpenSettings={() => setSettingsOpen(true)} />
      </div>
      <PixelDialog
        open={settingsOpen}
        title={theme.settingsTitle}
        closeLabel={zh.closeSettings}
        onClose={() => setSettingsOpen(false)}
      >
        {settingsOpen && <SettingsView />}
      </PixelDialog>
    </div>
  );
}

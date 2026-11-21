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

export default function App() {
  const screen = useAppStore((s) => s.screen);
  const [settingsOpen, setSettingsOpen] = useState(false);
  useCliDetection(true);
  return (
    <div className="pixel-shell">
      <TopBar title={zh.appName}>
        <PixelButton aria-haspopup="dialog" onClick={() => setSettingsOpen(true)}>
          {zh.settings}
        </PixelButton>
      </TopBar>
      <div className="pixel-shell__content">
        {screen === "title" ? <TitleScreen /> : (
          <div className="pixel-shell__camp">
            <CampView />
          </div>
        )}
        <CliStatus />
      </div>
      <PixelDialog
        open={settingsOpen}
        title={zh.settings}
        closeLabel={zh.closeSettings}
        onClose={() => setSettingsOpen(false)}
      >
        {settingsOpen && <SettingsView />}
      </PixelDialog>
    </div>
  );
}

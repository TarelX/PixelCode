import { useAppStore } from "./store";
import { TitleScreen } from "../ui/TitleScreen";
import { CampView } from "../ui/CampView";
import { useState } from "react";
import { useCliDetection } from "./cli-detection";
import { CliStatus } from "../ui/components/CliStatus";
import { SettingsView } from "../ui/SettingsView";

export default function App() {
  const screen = useAppStore((s) => s.screen);
  const [settingsOpen, setSettingsOpen] = useState(false);
  useCliDetection(true);
  return (
    <>
      <button onClick={() => setSettingsOpen(!settingsOpen)}>{settingsOpen ? "关闭设置" : "设置"}</button>
      {settingsOpen ? <SettingsView /> : <>
        {screen === "title" ? <TitleScreen /> : <CampView />}
        <CliStatus />
      </>}
    </>
  );
}

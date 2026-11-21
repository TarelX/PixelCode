import { useAppStore } from "./store";
import { TitleScreen } from "../ui/TitleScreen";
import { CampView } from "../ui/CampView";

export default function App() {
  const screen = useAppStore((s) => s.screen);
  return screen === "title" ? <TitleScreen /> : <CampView />;
}

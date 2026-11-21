import { useAppStore } from "../app/store";
import { zh } from "../i18n/zh";

export function TitleScreen() {
  const setScreen = useAppStore((s) => s.setScreen);
  return (
    <main className="screen title-screen">
      <h1>{zh.appName}</h1>
      <button onClick={() => setScreen("camp")}>{zh.titleRecruit}</button>
    </main>
  );
}

import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { useAppStore, type Camp } from "../app/store";
import { zh } from "../i18n/zh";

export function TitleScreen() {
  const openCamp = useAppStore((s) => s.openCamp);
  const [recent, setRecent] = useState<Camp[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const opening = useRef(false);

  useEffect(() => {
    let active = true;
    invoke<Camp[]>("list_recent_camps")
      .then((camps) => { if (active) setRecent(camps); })
      .catch((reason: unknown) => { if (active) setError(`${zh.recentError}${String(reason)}`); });
    return () => { active = false; };
  }, []);

  async function enterCamp(path?: string) {
    if (opening.current) return;
    opening.current = true;
    setBusy(true);
    setError(null);
    try {
      const selected = path ?? await open({ directory: true, multiple: false, title: zh.titleRecruit });
      if (typeof selected === "string") {
        openCamp(await invoke<Camp>("open_camp", { path: selected }));
      }
    } catch (reason) {
      setError(`${zh.openError}${String(reason)}`);
    } finally {
      opening.current = false;
      setBusy(false);
    }
  }
  return (
    <main className="screen title-screen">
      <h1>{zh.appName}</h1>
      <button disabled={busy} onClick={() => void enterCamp()}>{zh.titleRecruit}</button>
      <h2>{zh.recentCamps}</h2>
      {recent.length === 0 && <p>{zh.noRecentCamps}</p>}
      <ul>
        {recent.map((camp) => (
          <li key={camp.id}>
            <button disabled={busy} onClick={() => void enterCamp(camp.root_path)}>
              {camp.root_path}
            </button>
          </li>
        ))}
      </ul>
      {busy && <p role="status">{zh.openingCamp}</p>}
      {error && <p role="alert">{error}</p>}
    </main>
  );
}

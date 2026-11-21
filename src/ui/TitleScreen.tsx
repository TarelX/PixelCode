import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { useAppStore, type Camp } from "../app/store";
import { zh } from "../i18n/zh";
import { PixelButton } from "./components/PixelButton";
import { PixelIcon } from "./components/PixelIcon";
import theme from "../../themes/three-kingdoms/ui.json";
import orchard from "../../themes/three-kingdoms/tiles/orchard.png";

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
      <div className="welcome-hero">
        <div className="welcome-copy"><span className="section-label welcome-eyebrow">{theme.welcomeEyebrow}</span>
          <h1>{theme.welcomeTitle[0]}<br /><span>{theme.welcomeTitle[1]}</span></h1>
          <p>{theme.welcomeDescription}</p>
          <PixelButton variant="primary" disabled={busy} onClick={() => void enterCamp()}><PixelIcon name="plus" />{zh.titleRecruit}<PixelIcon name="arrow" /></PixelButton>
          <small>{theme.openHint}</small>
        </div>
        <figure className="welcome-art"><img src={orchard} width="384" height="288" alt={theme.orchardAlt} /><figcaption><span />{theme.orchardCaption}<span /></figcaption></figure>
      </div>
      <section className="recent-projects" aria-labelledby="recent-title">
        <div className="section-heading"><h2 id="recent-title">{zh.recentCamps}</h2><span>{recent.length.toString().padStart(2, "0")} / 05</span></div>
        {recent.length === 0 && <div className="recent-empty"><PixelIcon name="folder" /><div><strong>{zh.noRecentCamps}</strong><p>{theme.emptyHint}</p></div><span aria-hidden="true">—</span></div>}
        <ul className="title-screen__recent">
        {recent.map((camp) => (
          <li key={camp.id}>
            <PixelButton disabled={busy} onClick={() => void enterCamp(camp.root_path)}>
              <span className="recent-project__icon"><PixelIcon name="folder" /></span><span className="recent-project__info"><strong>{camp.name || camp.root_path}</strong><small>{camp.root_path}</small></span><span className="recent-project__open">{theme.enterCamp}<PixelIcon name="arrow" /></span>
            </PixelButton>
          </li>
        ))}
        </ul>
      </section>
      {busy && <p role="status">{zh.openingCamp}</p>}
      {error && <p role="alert">{error}</p>}
      <p className="welcome-footer"><PixelIcon name="terminal" />{theme.localHint}</p>
    </main>
  );
}

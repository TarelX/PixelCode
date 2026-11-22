import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { useAppStore, type Camp } from "../app/store";
import { PixelButton } from "./components/PixelButton";
import { PixelIcon } from "./components/PixelIcon";
import { useI18n } from "../i18n";
import { RoleAvatar } from "./components/RoleAvatar";
import { parse } from "yaml";
import names from "../../themes/three-kingdoms/names.yaml?raw";
import { avatarFiles } from "../camp/sprites";

const { generals } = parse(names) as { generals: { id: string; name: string; name_en: string }[] };

export function TitleScreen() {
  const { locale, theme } = useI18n();
  const openCamp = useAppStore((s) => s.openCamp);
  const [recent, setRecent] = useState<Camp[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<["recentError" | "openError", string] | null>(null);
  const opening = useRef(false);

  useEffect(() => {
    let active = true;
    invoke<Camp[]>("list_recent_camps")
      .then((camps) => { if (active) setRecent(camps); })
      .catch((reason: unknown) => { if (active) setError(["recentError", String(reason)]); });
    return () => { active = false; };
  }, []);

  async function enterCamp(path?: string) {
    if (opening.current) return;
    opening.current = true;
    setBusy(true);
    setError(null);
    try {
      const selected = path ?? await open({ directory: true, multiple: false, title: theme.titleRecruit });
      if (typeof selected === "string") {
        openCamp(await invoke<Camp>("open_camp", { path: selected }));
      }
    } catch (reason) {
      setError(["openError", String(reason)]);
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
          <PixelButton variant="primary" disabled={busy} onClick={() => void enterCamp()}><PixelIcon name="folder" />{theme.studio.openProject}<PixelIcon name="arrow" /></PixelButton>
          <small>{theme.openHint}</small>
        </div>
        <aside className="welcome-ensemble" aria-label={theme.workbench.yourTeam}><header><PixelIcon name="team" /><span>{theme.workbench.yourTeam}</span><span className="tab-tag">{generals.length}</span></header><div className="welcome-team" aria-hidden="true">{generals.map((general, index) => <span key={general.id} title={locale === "en" ? general.name_en : general.name}><RoleAvatar avatar={avatarFiles[index] ?? null} /></span>)}</div><strong>{theme.workbench.teamTitle}</strong><p>{theme.workbench.teamHint}</p><footer><i className="status-dot" />{theme.studio.local}</footer></aside>
      </div>
      <ol className="welcome-workflow">{theme.workbench.steps.map((step, index) => <li key={step.title}><span className="workflow-number">0{index + 1}</span><div><strong>{step.title}</strong><p>{step.hint}</p></div><PixelIcon name={(["folder", "team", "grid"] as const)[index]} /></li>)}</ol>
      <section className="recent-projects" aria-labelledby="recent-title">
        <div className="section-heading"><h2 id="recent-title">{theme.recentCamps}</h2><span>{recent.length.toString().padStart(2, "0")} / 05</span></div>
        {recent.length === 0 && <div className="recent-empty"><PixelIcon name="folder" /><div><strong>{theme.noRecentCamps}</strong><p>{theme.emptyHint}</p></div><span aria-hidden="true">—</span></div>}
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
      {busy && <p role="status">{theme.openingCamp}</p>}
      {error && <p role="alert">{theme[error[0]]}{error[1]}</p>}
      <p className="welcome-footer"><PixelIcon name="terminal" />{theme.localHint}</p>
    </main>
  );
}

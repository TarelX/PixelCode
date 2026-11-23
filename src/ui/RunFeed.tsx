import { useEffect, useRef, useState } from "react";
import { useRoster } from "../camp/roster";
import { useRuns } from "../camp/run";
import { roleName, useI18n } from "../i18n";
import { readArtifact } from "../orchestrator/artifact-manager";
import { PixelDialog } from "./PixelDialog";
import { RoleAvatar } from "./components/RoleAvatar";
import { PixelIcon } from "./components/PixelIcon";
import { usageLimitText, tokenUsageText } from "./run-usage";

export function RunFeed({ onPlan }: { onPlan: () => void }) {
  const { locale, theme } = useI18n();
  const { roles, selectedId } = useRoster();
  const runs = useRuns((s) => s.runs);
  const dismiss = useRuns((s) => s.dismiss);
  const [filter, setFilter] = useState("all");
  const [viewer, setViewer] = useState<{ content: string; path: string } | null>(null);
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(false);
  const host = useRef<HTMLDivElement>(null);
  const pinned = useRef(true);
  const entries = Object.entries(runs).filter(([id]) => filter === "all" || id === selectedId);
  useEffect(() => { if (pinned.current && host.current) host.current.scrollTop = host.current.scrollHeight; }, [runs, filter]);
  async function view(id: string, path: string) {
    setLoading(true); setError("");
    try { setViewer({ ...await readArtifact(id), path }); }
    catch (reason) { setError(String(reason)); }
    finally { setLoading(false); }
  }
  return <section className="activity-feed" aria-label={theme.studio.activity}>
    <header><strong>{theme.studio.activity}</strong><span className="live-label"><i className="status-dot" />{theme.studio.live}</span><div className="feed-filters">
      {(["all", "selected"] as const).map((id) => <button key={id} aria-pressed={filter === id} onClick={() => { pinned.current = true; setFilter(id); }}>{theme.studio[id]}</button>)}
    </div></header>
    <div className="activity-scroll" ref={host} onScroll={() => { const el = host.current!; pinned.current = el.scrollHeight - el.clientHeight - el.scrollTop < 48; }}>
      {entries.length === 0 && <div className="activity-empty"><PixelIcon name="chat" /><div><strong>{theme.studio.emptyTitle}</strong><p>{theme.studio.emptyHint}</p></div></div>}
      {entries.map(([id, run]) => {
        const role = roles.find((item) => item.id === id);
        const limit = usageLimitText(run, theme, locale);
        const metrics = tokenUsageText(run, theme);
        return <article className={`feed-message feed-message--${run.status}`} key={`${id}-${run.run_id}`}>
          <RoleAvatar avatar={role?.avatar ?? null} scale={1} /><div className="feed-message__body">
            <div className="feed-message__heading"><strong>{role ? roleName(role, locale) : id}</strong><span className={`run-label run-label--${run.status}`}>{theme.runStates[run.status]}</span>
              {run.status !== "running" && <button className="feed-dismiss" aria-label={theme.taskDismiss} onClick={() => dismiss(id)}>×</button>}
            </div>
            <pre>{run.text || theme.taskWaiting}</pre>
            {run.tool && <small><PixelIcon name="terminal" />{theme.taskTool}: {run.tool}</small>}
            {limit && <small role="status">{limit}</small>}
            {metrics && <small className="run-metrics">{metrics}</small>}
            {run.error && <p role="alert">{run.error}</p>}
            {run.artifact && <button className="feed-artifact" disabled={loading} title={run.artifact.absolute_path} onClick={() => run.artifact!.type === "plan" ? onPlan() : void view(run.artifact!.id, run.artifact!.absolute_path)}><PixelIcon name="folder" />{run.artifact.file_path.split(/[/\\]/).pop()}<PixelIcon name="arrow" /></button>}
          </div>
        </article>;
      })}
      {error && <p role="alert">{error}</p>}
    </div>
    <PixelDialog open={viewer !== null} title={theme.artifactTitle} closeLabel={theme.artifactClose} onClose={() => setViewer(null)}><p className="artifact-location">{viewer?.path}</p><pre className="artifact-content">{viewer?.content}</pre></PixelDialog>
  </section>;
}

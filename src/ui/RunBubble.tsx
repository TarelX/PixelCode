import { useEffect, useLayoutEffect, useRef, useState, type CSSProperties } from "react";
import { useRuns, type RoleRun } from "../camp/run";
import { bubblePosition, type BubbleAnchor } from "../camp/bubble";
import { useI18n } from "../i18n";
import { readArtifact } from "../orchestrator/artifact-manager";
import { usageLimitText, tokenUsageText } from "./run-usage";
import { PixelDialog } from "./PixelDialog";

export function RunBubble({ id, run, anchor }: { id: string; run: RoleRun; anchor: BubbleAnchor }) {
  const { locale, theme } = useI18n();
  const text = useRef<HTMLPreElement>(null);
  const bubble = useRef<HTMLElement>(null);
  const [height, setHeight] = useState(0);
  const [viewer, setViewer] = useState<{ content: string } | null>(null);
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(false);
  const view = async () => {
    if (!run.artifact) return;
    setLoading(true);
    setError("");
    try { setViewer(await readArtifact(run.artifact.id)); }
    catch (error) { setError(String(error)); }
    finally { setLoading(false); }
  };
  useLayoutEffect(() => {
    const element = bubble.current!;
    const measure = () => setHeight(element.offsetHeight);
    const observer = new ResizeObserver(measure);
    observer.observe(element);
    measure();
    return () => observer.disconnect();
  }, []);
  const { below, tail, ...position } = bubblePosition(anchor, height);
  const draft = run.artifact?.type === "plan";
  const dismiss = useRuns((s) => s.dismiss);
  const limit = usageLimitText(run, theme, locale);
  const metrics = tokenUsageText(run, theme);
  useEffect(() => { if (text.current) text.current.scrollTop = text.current.scrollHeight; }, [run.text]);
  return <aside ref={bubble} className={`run-bubble run-bubble--${run.status}${below ? " run-bubble--below" : ""}`} style={{ ...position, maxHeight: anchor.canvas.height - 16, "--tail-left": `${tail}px` } as CSSProperties} aria-label={theme.taskOutput}>
    <header><strong>{draft ? theme.planDraft : theme.runStates[run.status]}</strong>{run.status !== "running" && <button aria-label={theme.taskDismiss} onClick={() => dismiss(id)}>×</button>}</header>
    {(run.text || run.status === "running") && <pre ref={text}>{run.text || theme.taskWaiting}</pre>}
    {run.tool && <small>{theme.taskTool}: {run.tool}</small>}
    {limit && <small role="status">{limit}</small>}
    {metrics && <small className="run-metrics">{metrics}</small>}
    {run.error && <p role="alert">{run.error}</p>}
    {run.artifact && <div className="run-artifact">
      <strong>{run.artifact.file_path.split("/").pop()}</strong>
      <small>{run.artifact.absolute_path}</small>
      <button disabled={loading} onClick={() => void view()}>{loading ? theme.artifactLoading : theme.artifactView}</button>
    </div>}
    {error && <p role="alert">{error}</p>}
    <PixelDialog open={viewer !== null} title={theme.artifactTitle} closeLabel={theme.artifactClose} onClose={() => setViewer(null)}>
      <p className="artifact-location">{run.artifact?.absolute_path}</p>
      <pre className="artifact-content">{viewer?.content}</pre>
    </PixelDialog>
  </aside>;
}

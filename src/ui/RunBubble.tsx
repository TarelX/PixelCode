import { useEffect, useRef } from "react";
import { useRuns, type RoleRun } from "../camp/run";
import theme from "../../themes/three-kingdoms/ui.json";

export function RunBubble({ id, run, x, y }: { id: string; run: RoleRun; x: number; y: number }) {
  const text = useRef<HTMLPreElement>(null);
  const dismiss = useRuns((s) => s.dismiss);
  useEffect(() => { if (text.current) text.current.scrollTop = text.current.scrollHeight; }, [run.text]);
  return <aside className={`run-bubble run-bubble--${run.status}`} style={{ left: x, top: y }} aria-label={theme.taskOutput}>
    <header><strong>{theme.runStates[run.status]}</strong>{run.status !== "running" && <button aria-label={theme.taskDismiss} onClick={() => dismiss(id)}>×</button>}</header>
    <pre ref={text}>{run.text || theme.taskWaiting}</pre>
    {run.tool && <small>{theme.taskTool}: {run.tool}</small>}
    {run.error && <p role="alert">{run.error}</p>}
  </aside>;
}

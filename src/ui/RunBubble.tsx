import { useEffect, useLayoutEffect, useRef, useState, type CSSProperties } from "react";
import { useRuns, type RoleRun } from "../camp/run";
import { bubblePosition, type BubbleAnchor } from "../camp/bubble";
import theme from "../../themes/three-kingdoms/ui.json";

export function RunBubble({ id, run, anchor }: { id: string; run: RoleRun; anchor: BubbleAnchor }) {
  const text = useRef<HTMLPreElement>(null);
  const bubble = useRef<HTMLElement>(null);
  const [height, setHeight] = useState(0);
  useLayoutEffect(() => {
    const element = bubble.current!;
    const measure = () => setHeight(element.offsetHeight);
    const observer = new ResizeObserver(measure);
    observer.observe(element);
    measure();
    return () => observer.disconnect();
  }, []);
  const { below, tail, ...position } = bubblePosition(anchor, height);
  const dismiss = useRuns((s) => s.dismiss);
  useEffect(() => { if (text.current) text.current.scrollTop = text.current.scrollHeight; }, [run.text]);
  return <aside ref={bubble} className={`run-bubble run-bubble--${run.status}${below ? " run-bubble--below" : ""}`} style={{ ...position, maxHeight: anchor.canvas.height - 16, "--tail-left": `${tail}px` } as CSSProperties} aria-label={theme.taskOutput}>
    <header><strong>{theme.runStates[run.status]}</strong>{run.status !== "running" && <button aria-label={theme.taskDismiss} onClick={() => dismiss(id)}>×</button>}</header>
    <pre ref={text}>{run.text || theme.taskWaiting}</pre>
    {run.tool && <small>{theme.taskTool}: {run.tool}</small>}
    {run.error && <p role="alert">{run.error}</p>}
  </aside>;
}

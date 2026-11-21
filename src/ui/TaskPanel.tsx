import { useState } from "react";
import { useRoster } from "../camp/roster";
import { useRuns } from "../camp/run";
import theme from "../../themes/three-kingdoms/ui.json";

export function TaskPanel({ campId, ready }: { campId: string; ready: boolean }) {
  const role = useRoster((s) => s.roles.find((r) => r.id === s.selectedId));
  const { busy, error, start, stop } = useRuns();
  const [prompt, setPrompt] = useState("");
  const [choice, setChoice] = useState({ roleId: "", index: 0 });
  const index = choice.roleId === role?.id ? choice.index : 0;
  return <section className="task-panel">
    <h2>{theme.taskTitle} · {role?.name ?? theme.taskSelect}</h2>
    {role && role.loadouts.length > 1 && <div className="task-loadouts" role="group" aria-label={theme.taskLoadout}>
      {role.loadouts.map((loadout, i) => <button key={i} type="button" aria-pressed={index === i} disabled={busy} onClick={() => setChoice({ roleId: role.id, index: i })}>{loadout.cli} · {loadout.model}</button>)}
    </div>}
    <label htmlFor="task-prompt">{theme.taskPrompt}</label>
    <textarea id="task-prompt" rows={2} value={prompt} disabled={busy} onChange={(e) => setPrompt(e.target.value)} placeholder={theme.taskPlaceholder} />
    <div className="task-actions"><button disabled={!ready || !role || busy || !prompt.trim()} onClick={() => role && void start(campId, role.id, index, prompt)}>{theme.taskStart}</button>
      {busy && <button onClick={() => void stop()}>{theme.taskStop}</button>}</div>
    {error && <p role="alert">{error}</p>}
  </section>;
}
